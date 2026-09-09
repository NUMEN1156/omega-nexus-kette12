use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rustls::server::AllowAnyAuthenticatedClient;
use rustls::{Certificate, PrivateKey, ServerConfig};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio::time::{self, Instant};
use tokio_rustls::TlsAcceptor;
use tracing::{error, info, warn};
use x509_parser::prelude::*;

use crate::config::RelayConfig;
use crate::protocol::{self, RelayMessage};

/// Authenticated relay with bounded topic fan-out.
///
/// Every authenticated session is currently a subscriber. Payloads are
/// broadcast to all other sessions; the publisher receives an Ack only.
pub struct RelayServer {
    config: RelayConfig,
    tls_acceptor: TlsAcceptor,
}

impl RelayServer {
    pub fn new(config: RelayConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let tls_acceptor = build_tls_acceptor(&config)?;
        Ok(Self {
            config,
            tls_acceptor,
        })
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(&self.config.bind_addr).await?;
        let (routing_tx, _) = broadcast::channel::<(String, RelayMessage)>(256);

        info!(
            "==> kette12-relay (mTLS + protocol + session + routing) listening on {}",
            self.config.bind_addr
        );

        loop {
            let (stream, addr) = listener.accept().await?;
            info!("Incoming TCP connection from: {}", addr);

            let acceptor = self.tls_acceptor.clone();
            let routing_tx = routing_tx.clone();
            let session_timeout = Duration::from_secs(self.config.session_timeout_secs);

            tokio::spawn(async move {
                if let Err(e) = handle_client(
                    acceptor,
                    stream,
                    addr,
                    session_timeout,
                    routing_tx,
                )
                .await
                {
                    error!("Error handling client {}: {}", addr, e);
                }
            });
        }
    }
}

fn build_tls_acceptor(config: &RelayConfig) -> Result<TlsAcceptor, Box<dyn std::error::Error>> {
    let cert_file = File::open(&config.cert_path)?;
    let mut cert_reader = BufReader::new(cert_file);
    let certs = rustls_pemfile::certs(&mut cert_reader)?
        .into_iter()
        .map(Certificate)
        .collect();

    let key_file = File::open(&config.key_path)?;
    let mut key_reader = BufReader::new(key_file);
    let keys = rustls_pemfile::pkcs8_private_keys(&mut key_reader)?;
    let key = PrivateKey(
        keys.into_iter()
            .next()
            .ok_or("No private key found in key file")?,
    );

    let ca_file = File::open(&config.ca_cert_path)?;
    let mut ca_reader = BufReader::new(ca_file);
    let ca_certs = rustls_pemfile::certs(&mut ca_reader)?;
    let mut root_store = rustls::RootCertStore::empty();
    for ca in ca_certs {
        root_store.add(&Certificate(ca))?;
    }

    let client_auth = AllowAnyAuthenticatedClient::new(root_store);
    let server_config = ServerConfig::builder()
        .with_safe_defaults()
        .with_client_cert_verifier(Arc::new(client_auth))
        .with_single_cert(certs, key)?;

    Ok(TlsAcceptor::from(Arc::new(server_config)))
}

fn extract_node_id(cert_der: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let (_, cert) = X509Certificate::from_der(cert_der)?;

    if let Some(cn) = cert.subject().iter_common_name().next() {
        if let Ok(s) = cn.as_str() {
            return Ok(s.to_string());
        }
    }

    if let Some(sans) = cert.subject_alternative_name()? {
        for san in &sans.value.general_names {
            if let GeneralName::DNSName(name) = san {
                return Ok(name.to_string());
            }
        }
    }

    Err("Could not extract node_id from client certificate (no CN or DNS SAN)".into())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

async fn handle_client(
    acceptor: TlsAcceptor,
    stream: TcpStream,
    addr: std::net::SocketAddr,
    session_timeout: Duration,
    routing_tx: broadcast::Sender<(String, RelayMessage)>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut tls_stream = acceptor.accept(stream).await?;
    info!("TLS handshake completed for {}", addr);

    let cert_node_id = {
        let (_, session) = tls_stream.get_ref();
        let peer_certs = session
            .peer_certificates()
            .ok_or("No peer certificate presented (mTLS required)")?;
        let first = peer_certs.first().ok_or("Empty peer certificate chain")?;
        extract_node_id(&first.0).map_err(|e| e.to_string())?
    };

    info!("Authenticated certificate node_id={} from {}", cert_node_id, addr);

    let first = tokio::time::timeout(session_timeout, protocol::read_message(&mut tls_stream))
        .await
        .map_err(|_| "timeout waiting for initial Handshake")??;

    let (node_id, role) = match first {
        RelayMessage::Handshake { node_id, role } => {
            if node_id != cert_node_id {
                let _ = protocol::write_message(
                    &mut tls_stream,
                    &RelayMessage::Ack {
                        status: "error: node_id mismatch with certificate".into(),
                    },
                )
                .await;
                return Err(format!(
                    "node_id mismatch: handshake claimed '{}', certificate says '{}'",
                    node_id, cert_node_id
                )
                .into());
            }
            (node_id, role)
        }
        other => {
            let _ = protocol::write_message(
                &mut tls_stream,
                &RelayMessage::Ack {
                    status: "error: expected Handshake as first message".into(),
                },
            )
            .await;
            return Err(format!("expected Handshake, got {:?}", other).into());
        }
    };

    info!("Handshake accepted: node_id={}, role={}", node_id, role);
    protocol::write_message(
        &mut tls_stream,
        &RelayMessage::Ack {
            status: "ok".into(),
        },
    )
    .await?;

    let mut routing_rx = routing_tx.subscribe();
    let mut last_activity = Instant::now();

    loop {
        let remaining = session_timeout.saturating_sub(last_activity.elapsed());

        tokio::select! {
            _ = time::sleep(remaining) => {
                warn!("Session timeout for {} (no message for {:?})", node_id, session_timeout);
                let _ = protocol::write_message(
                    &mut tls_stream,
                    &RelayMessage::Ack { status: "error: session timeout".into() },
                ).await;
                break;
            }

            routed = routing_rx.recv() => {
                match routed {
                    Ok((source_node_id, RelayMessage::Payload { topic, data }))
                        if source_node_id != node_id =>
                    {
                        info!("Routing topic='{}' from {} to {}", topic, source_node_id, node_id);
                        protocol::write_message(
                            &mut tls_stream,
                            &RelayMessage::Payload { topic, data },
                        ).await?;
                    }
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!("Subscriber {} lagged; skipped {} routed messages", node_id, skipped);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }

            result = protocol::read_message(&mut tls_stream) => {
                let msg = match result {
                    Ok(m) => m,
                    Err(e) => {
                        info!("Connection closed or read error for {}: {}", node_id, e);
                        break;
                    }
                };
                last_activity = Instant::now();

                match msg {
                    RelayMessage::Heartbeat { timestamp } => {
                        info!("Heartbeat from {} (ts={})", node_id, timestamp);
                        protocol::write_message(
                            &mut tls_stream,
                            &RelayMessage::Heartbeat { timestamp: now_secs() },
                        ).await?;
                    }
                    RelayMessage::Payload { topic, data } => {
                        let receivers = routing_tx.send((
                            node_id.clone(),
                            RelayMessage::Payload { topic: topic.clone(), data },
                        )).unwrap_or(0);
                        let subscribers = receivers.saturating_sub(1);
                        info!("Published topic='{}' from {} to {} subscribers", topic, node_id, subscribers);
                        protocol::write_message(
                            &mut tls_stream,
                            &RelayMessage::Ack {
                                status: format!("published:{}:fanout={}", topic, subscribers),
                            },
                        ).await?;
                    }
                    RelayMessage::Handshake { .. } => {
                        warn!("Unexpected second Handshake from {}, rejecting", node_id);
                        protocol::write_message(
                            &mut tls_stream,
                            &RelayMessage::Ack { status: "error: handshake already completed".into() },
                        ).await?;
                    }
                    RelayMessage::Ack { status } => info!("Ack from {}: {}", node_id, status),
                }
            }
        }
    }

    info!("Session ended for {}", node_id);
    Ok(())
}
