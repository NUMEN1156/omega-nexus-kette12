use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;

use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls::server::AllowAnyAuthenticatedClient;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::TlsAcceptor;
use tracing::{info, error, warn};
use x509_parser::prelude::*;

use crate::config::RelayConfig;
use crate::protocol::RelayMessage;

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
        info!("==> kette12-relay (mTLS) listening on {}", self.config.bind_addr);

        loop {
            let (stream, addr) = listener.accept().await?;
            info!("Incoming TCP connection from: {}", addr);

            let acceptor = self.tls_acceptor.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_client(acceptor, stream, addr).await {
                    error!("Error handling client {}: {}", addr, e);
                }
            });
        }
    }
}

fn build_tls_acceptor(config: &RelayConfig) -> Result<TlsAcceptor, Box<dyn std::error::Error>> {
    // Load server certificate
    let cert_file = File::open(&config.cert_path)?;
    let mut cert_reader = BufReader::new(cert_file);
    let certs = rustls_pemfile::certs(&mut cert_reader)?
        .into_iter()
        .map(Certificate)
        .collect();

    // Load server private key
    let key_file = File::open(&config.key_path)?;
    let mut key_reader = BufReader::new(key_file);
    let keys = rustls_pemfile::pkcs8_private_keys(&mut key_reader)?;
    let key = PrivateKey(
        keys.into_iter()
            .next()
            .ok_or("No private key found in key file")?,
    );

    // Load CA certificate for client authentication (mTLS)
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

/// Extract a usable node_id from the client certificate (CN preferred, first SAN as fallback).
fn extract_node_id(cert_der: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let (_, cert) = X509Certificate::from_der(cert_der)?;

    // Prefer Common Name
    if let Some(cn) = cert.subject().iter_common_name().next() {
        if let Ok(s) = cn.as_str() {
            return Ok(s.to_string());
        }
    }

    // Fallback: first DNS SAN
    if let Some(sans) = cert.subject_alternative_name()? {
        for san in &sans.value.general_names {
            if let GeneralName::DNSName(name) = san {
                return Ok(name.to_string());
            }
        }
    }

    Err("Could not extract node_id from client certificate (no CN or DNS SAN)".into())
}

async fn handle_client(
    acceptor: TlsAcceptor,
    stream: TcpStream,
    addr: std::net::SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    // Perform TLS handshake (requires valid client certificate)
    let mut tls_stream = acceptor.accept(stream).await?;
    info!("TLS handshake completed for {}", addr);

    // Extract peer certificate and derive node_id
    let node_id = {
        let (_, session) = tls_stream.get_ref();
        let peer_certs = session
            .peer_certificates()
            .ok_or("No peer certificate presented (mTLS required)")?;

        let first = peer_certs
            .first()
            .ok_or("Empty peer certificate chain")?;

        extract_node_id(&first.0)?
    };

    info!("Authenticated node_id={} from {}", node_id, addr);

    // Basic application-level loop (still echo for now, ready for protocol framing)
    let mut buf = vec![0u8; 4096];

    loop {
        let n = tls_stream.read(&mut buf).await?;
        if n == 0 {
            info!("Connection closed by {}", node_id);
            break;
        }

        // Future: deserialize RelayMessage, validate Handshake against node_id, route Payload, etc.
        // For now we just echo the raw bytes as a safe stub.
        if let Err(e) = tls_stream.write_all(&buf[..n]).await {
            warn!("Write error to {}: {}", node_id, e);
            break;
        }
    }

    Ok(())
}
