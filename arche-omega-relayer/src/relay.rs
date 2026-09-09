use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls::server::AllowAnyAuthenticatedClient;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;
use tracing::{info, error, warn};
use x509_parser::prelude::*;

use crate::config::RelayConfig;
use crate::protocol::{self, RelayMessage};

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
        info!("==> kette12-relay (mTLS + protocol) listening on {}", self.config.bind_addr);

        loop {
            let (stream, addr) = listener.accept().await?;
            info!("Incoming TCP connection from: {}", addr);

            let acceptor = self.tls_acceptor.clone();
            let heartbeat_interval = self.config.heartbeat_interval_secs;

            tokio::spawn(async move {
                if let Err(e) = handle_client(acceptor, stream, addr, heartbeat_interval).await {
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
    _heartbeat_interval_secs: u64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut tls_stream = acceptor.accept(stream).await?;
    info!("TLS handshake completed for {}", addr);

    // Derive node_id from client certificate (authoritative)
    let cert_node_id = {
        let (_, session) = tls_stream.get_ref();
        let peer_certs = session
            .peer_certificates()
            .ok_or("No peer certificate presented (mTLS required)")?;

        let first = peer_certs
            .first()
            .ok_or("Empty peer certificate chain")?;

        extract_node_id(&first.0).map_err(|e| e.to_string())?
    };

    info!("Authenticated certificate node_id={} from {}", cert_node_id, addr);

    // --- Application protocol: first message MUST be Handshake ---
    let first = protocol::read_message(&mut tls_stream).await?;

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

    // Acknowledge successful handshake
    protocol::write_message(
        &mut tls_stream,
        &RelayMessage::Ack {
            status: "ok".into(),
        },
    )
    .await?;

    // --- Main message loop ---
    loop {
        let msg = match protocol::read_message(&mut tls_stream).await {
            Ok(m) => m,
            Err(e) => {
                // Graceful close or protocol error
                info!("Connection closed or read error for {}: {}", node_id, e);
                break;
            }
        };

        match msg {
            RelayMessage::Heartbeat { timestamp } => {
                info!("Heartbeat from {} (ts={})", node_id, timestamp);
                // Reply with our own heartbeat / ack
                protocol::write_message(
                    &mut tls_stream,
                    &RelayMessage::Heartbeat {
                        timestamp: now_secs(),
                    },
                )
                .await?;
            }
            RelayMessage::Payload { topic, data } => {
                info!(
                    "Payload from {} topic='{}' len={}",
                    node_id,
                    topic,
                    data.len()
                );
                // Stub: acknowledge receipt. Real routing comes later.
                protocol::write_message(
                    &mut tls_stream,
                    &RelayMessage::Ack {
                        status: format!("received:{}", topic),
                    },
                )
                .await?;
            }
            RelayMessage::Handshake { .. } => {
                warn!("Unexpected second Handshake from {}, ignoring", node_id);
                protocol::write_message(
                    &mut tls_stream,
                    &RelayMessage::Ack {
                        status: "error: handshake already completed".into(),
                    },
                )
                .await?;
            }
            RelayMessage::Ack { status } => {
                info!("Ack from {}: {}", node_id, status);
            }
        }
    }

    info!("Session ended for {}", node_id);
    Ok(())
}
