use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;
use std::sync::atomic::Ordering;
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

use crate::acl::TopicAcl;
use crate::config::RelayConfig;
use crate::metrics::Metrics;
use crate::outbox::OutboxWriter;
use crate::protocol::{self, RelayMessage};
use crate::skills::{self, SkillEvent, SkillKind, SkillResultKind};

pub struct RelayServer {
    config: RelayConfig,
    tls_acceptor: TlsAcceptor,
    metrics: Arc<Metrics>,
    outbox: OutboxWriter,
}

impl RelayServer {
    pub fn new(
        config: RelayConfig,
        metrics: Arc<Metrics>,
        outbox: OutboxWriter,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            tls_acceptor: build_tls_acceptor(&config)?,
            config,
            metrics,
            outbox,
        })
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(&self.config.bind_addr).await?;
        let (routing_tx, _) = broadcast::channel::<(String, RelayMessage)>(256);
        info!(
            "==> kette12-relay (mTLS + protocol + session + subscriptions + routing + metrics) listening on {}",
            self.config.bind_addr
        );

        loop {
            let (stream, addr) = listener.accept().await?;
            let acceptor = self.tls_acceptor.clone();
            let routing_tx = routing_tx.clone();
            let timeout = Duration::from_secs(self.config.session_timeout_secs);
            let metrics = self.metrics.clone();
            let outbox = self.outbox.clone();
            let outbox_filters = self.config.outbox_topic_filters.clone();
            let acl = self.config.acl.clone();

            tokio::spawn(async move {
                if let Err(e) = handle_client(
                    acceptor,
                    stream,
                    addr,
                    timeout,
                    routing_tx,
                    metrics,
                    outbox,
                    outbox_filters,
                    acl,
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
    let mut cert_reader = BufReader::new(File::open(&config.cert_path)?);
    let certs = rustls_pemfile::certs(&mut cert_reader)?
        .into_iter()
        .map(Certificate)
        .collect();
    let mut key_reader = BufReader::new(File::open(&config.key_path)?);
    let key = PrivateKey(
        rustls_pemfile::pkcs8_private_keys(&mut key_reader)?
            .into_iter()
            .next()
            .ok_or("No private key found")?,
    );
    let mut ca_reader = BufReader::new(File::open(&config.ca_cert_path)?);
    let mut roots = rustls::RootCertStore::empty();
    for ca in rustls_pemfile::certs(&mut ca_reader)? {
        roots.add(&Certificate(ca))?;
    }
    let verifier = AllowAnyAuthenticatedClient::new(roots);
    let cfg = ServerConfig::builder()
        .with_safe_defaults()
        .with_client_cert_verifier(Arc::new(verifier))
        .with_single_cert(certs, key)?;
    Ok(TlsAcceptor::from(Arc::new(cfg)))
}

fn extract_node_id(cert_der: &[u8]) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let (_, cert) = X509Certificate::from_der(cert_der)
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.to_string().into() })?;

    if let Some(cn) = cert.subject().iter_common_name().next() {
        if let Ok(s) = cn.as_str() {
            return Ok(s.to_string());
        }
    }

    if let Ok(Some(sans)) = cert.subject_alternative_name() {
        for san in &sans.value.general_names {
            if let GeneralName::DNSName(name) = san {
                return Ok(name.to_string());
            }
        }
    }

    Err("client certificate has no CN or DNS SAN".into())
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
    metrics: Arc<Metrics>,
    outbox: OutboxWriter,
    outbox_filters: Vec<String>,
    acl: TopicAcl,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut tls_stream = acceptor.accept(stream).await?;

    let cert_node_id = {
        let (_, session) = tls_stream.get_ref();
        let certs = session.peer_certificates().ok_or("No peer certificate")?;
        extract_node_id(&certs.first().ok_or("Empty peer certificate chain")?.0)?
    };

    let first = time::timeout(session_timeout, protocol::read_message(&mut tls_stream))
        .await
        .map_err(|_| "timeout waiting for Handshake")??;

    let (node_id, role) = match first {
        RelayMessage::Handshake { node_id, role } if node_id == cert_node_id => (node_id, role),
        RelayMessage::Handshake { node_id, .. } => {
            let _ = protocol::write_message(
                &mut tls_stream,
                &RelayMessage::Ack {
                    status: "error: node_id mismatch with certificate".into(),
                },
            )
            .await;
            return Err(format!("node_id mismatch: {}", node_id).into());
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

    info!(
        "Handshake accepted: node_id={}, role={}, addr={}",
        node_id, role, addr
    );
    metrics.total_handshakes.fetch_add(1, Ordering::Relaxed);
    metrics.active_sessions.fetch_add(1, Ordering::Relaxed);

    protocol::write_message(
        &mut tls_stream,
        &RelayMessage::Ack {
            status: "ok".into(),
        },
    )
    .await?;

    let (mut reader, mut writer) = tokio::io::split(tls_stream);
    let mut routing_rx = routing_tx.subscribe();
    let mut subscriptions: HashSet<String> = HashSet::new();
    let mut last_activity = Instant::now();

    loop {
        let remaining = session_timeout.saturating_sub(last_activity.elapsed());

        tokio::select! {
            _ = time::sleep(remaining) => {
                warn!("Session timeout for {}", node_id);
                metrics.total_timeouts.fetch_add(1, Ordering::Relaxed);
                let _ = protocol::write_message(
                    &mut writer,
                    &RelayMessage::Ack {
                        status: "error: session timeout".into(),
                    },
                ).await;
                break;
            }

            routed = routing_rx.recv() => match routed {
                Ok((source, RelayMessage::Payload { topic, data }))
                    if source != node_id && subscriptions.contains(&topic) =>
                {
                    protocol::write_message(
                        &mut writer,
                        &RelayMessage::Payload { topic, data },
                    ).await?;
                    metrics.total_payloads_out.fetch_add(1, Ordering::Relaxed);
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    warn!("Subscriber {} lagged; skipped {} messages", node_id, skipped);
                    metrics.total_lagged.fetch_add(skipped, Ordering::Relaxed);
                }
                Err(broadcast::error::RecvError::Closed) => break,
            },

            result = protocol::read_message(&mut reader) => {
                let msg = match result {
                    Ok(m) => m,
                    Err(e) => {
                        info!("Connection closed for {}: {}", node_id, e);
                        break;
                    }
                };
                last_activity = Instant::now();

                match msg {
                    RelayMessage::Subscribe { topic } => {
                        if !acl.allows(&node_id, &topic) {
                            protocol::write_message(&mut writer, &RelayMessage::Ack { status: format!("error: unauthorized topic: {}", topic) }).await?;
                            continue;
                        }
                        let added = subscriptions.insert(topic.clone());
                        if added {
                            metrics.total_subscribes.fetch_add(1, Ordering::Relaxed);
                        }
                        let status = if added { "subscribed" } else { "already-subscribed" };
                        protocol::write_message(
                            &mut writer,
                            &RelayMessage::Ack {
                                status: format!("{}:{}", status, topic),
                            },
                        ).await?;
                    }
                    RelayMessage::Unsubscribe { topic } => {
                        if !acl.allows(&node_id, &topic) {
                            protocol::write_message(&mut writer, &RelayMessage::Ack { status: format!("error: unauthorized topic: {}", topic) }).await?;
                            continue;
                        }
                        let removed = subscriptions.remove(&topic);
                        if removed {
                            metrics.total_unsubscribes.fetch_add(1, Ordering::Relaxed);
                        }
                        let status = if removed { "unsubscribed" } else { "not-subscribed" };
                        protocol::write_message(
                            &mut writer,
                            &RelayMessage::Ack {
                                status: format!("{}:{}", status, topic),
                            },
                        ).await?;
                    }
                    RelayMessage::Heartbeat { timestamp: _timestamp } => {
                        protocol::write_message(
                            &mut writer,
                            &RelayMessage::Heartbeat {
                                timestamp: now_secs(),
                            },
                        ).await?;
                    }
                    RelayMessage::Payload { topic, data } => {
                        if !acl.allows(&node_id, &topic) {
                            protocol::write_message(&mut writer, &RelayMessage::Ack { status: format!("error: unauthorized topic: {}", topic) }).await?;
                            continue;
                        }
                        if let Err(error) = validate_skill_payload(&metrics, &topic, &data) {
                            protocol::write_message(
                                &mut writer,
                                &RelayMessage::Ack {
                                    status: format!("error: fail-closed skill policy: {}", error),
                                },
                            )
                            .await?;
                            continue;
                        }
                        metrics.total_payloads_in.fetch_add(1, Ordering::Relaxed);
                        let should_persist = outbox_filters.is_empty()
                            || outbox_filters.iter().any(|filter| filter == &topic);
                        if should_persist {
                            if let Err(error) = outbox.enqueue(&topic, &data) {
                                warn!(%error, topic = %topic, "failed to enqueue payload in outbox");
                            }
                        }
                        let receivers = routing_tx
                            .send((
                                node_id.clone(),
                                RelayMessage::Payload {
                                    topic: topic.clone(),
                                    data,
                                },
                            ))
                            .unwrap_or(0);
                        protocol::write_message(
                            &mut writer,
                            &RelayMessage::Ack {
                                status: format!(
                                    "published:{}:fanout={}",
                                    topic,
                                    receivers.saturating_sub(1)
                                ),
                            },
                        ).await?;
                    }
                    RelayMessage::Handshake { .. } => {
                        protocol::write_message(
                            &mut writer,
                            &RelayMessage::Ack {
                                status: "error: handshake already completed".into(),
                            },
                        ).await?;
                    }
                    RelayMessage::Ack { status } => {
                        info!("Ack from {}: {}", node_id, status);
                    }
                }
            }
        }
    }

    metrics.active_sessions.fetch_sub(1, Ordering::Relaxed);
    info!("Session ended for {}", node_id);
    Ok(())
}

fn validate_skill_payload(
    metrics: &Metrics,
    topic: &str,
    data: &[u8],
) -> Result<(), String> {
    let event = match skills::validate_payload(topic, data) {
        Ok(event) => event,
        Err(error) => {
            if skills::is_skill_topic(topic) {
                metrics
                    .total_skill_rejections
                    .fetch_add(1, Ordering::Relaxed);
            }
            return Err(error);
        }
    };
    let Some(event) = event else {
        return Ok(());
    };
    match event {
        SkillEvent::Request { kind } => {
            metrics.total_skill_requests.fetch_add(1, Ordering::Relaxed);
            match kind {
                SkillKind::BrowserUse => {
                    metrics
                        .total_browser_skill_requests
                        .fetch_add(1, Ordering::Relaxed);
                }
                SkillKind::Scientific => {
                    metrics
                        .total_scientific_skill_requests
                        .fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        SkillEvent::Result { outcome, .. } => {
            metrics.total_skill_results.fetch_add(1, Ordering::Relaxed);
            match outcome {
                SkillResultKind::Success => {
                    metrics.total_skill_successes.fetch_add(1, Ordering::Relaxed);
                }
                SkillResultKind::Failure => {
                    metrics.total_skill_failures.fetch_add(1, Ordering::Relaxed);
                }
                SkillResultKind::Timeout => {
                    metrics.total_skill_timeouts.fetch_add(1, Ordering::Relaxed);
                }
                SkillResultKind::Denied => {
                    metrics.total_skill_denied.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }
    Ok(())
}
