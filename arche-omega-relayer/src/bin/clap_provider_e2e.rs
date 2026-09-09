use rustls::{Certificate, ClientConfig, PrivateKey, RootCertStore, ServerName};
use rustls_pemfile::{certs, pkcs8_private_keys};
use serde::{Deserialize, Serialize};
use std::{env, fs::File, io::BufReader, sync::Arc};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
    time::{sleep, Duration},
};
use tokio_rustls::TlsConnector;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "PascalCase")]
enum Message {
    Handshake { node_id: String, role: String },
    Subscribe { topic: String },
    Heartbeat { timestamp: u64 },
    Payload { topic: String, data: Vec<u8> },
    Ack { status: String },
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.into())
}

fn load_certs(path: &str) -> Vec<Certificate> {
    certs(&mut BufReader::new(File::open(path).expect("certificate file")))
        .expect("parse certificate")
        .into_iter()
        .map(Certificate)
        .collect()
}

fn load_key(path: &str) -> PrivateKey {
    PrivateKey(
        pkcs8_private_keys(&mut BufReader::new(File::open(path).expect("private key file")))
            .expect("parse private key")
            .into_iter()
            .next()
            .expect("private key missing"),
    )
}

async fn send<W: AsyncWrite + Unpin>(writer: &mut W, message: &Message) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let body = serde_json::to_vec(message)?;
    if body.is_empty() || body.len() > 16 * 1024 * 1024 { return Err("invalid frame size".into()); }
    writer.write_all(&(body.len() as u32).to_be_bytes()).await?;
    writer.write_all(&body).await?;
    writer.flush().await?;
    Ok(())
}

async fn receive<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Message, Box<dyn std::error::Error + Send + Sync>> {
    let mut header = [0u8; 4];
    reader.read_exact(&mut header).await?;
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 || length > 16 * 1024 * 1024 { return Err("invalid frame length".into()); }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).await?;
    Ok(serde_json::from_slice(&body)?)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut roots = RootCertStore::empty();
    for certificate in load_certs(&env_or("RELAY_CA_CERT", "certs/ca.crt")) { roots.add(&certificate)?; }
    let config = ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(roots)
        .with_client_auth_cert(
            load_certs(&env_or("CLAP_CLIENT_CERT", "certs/client.crt")),
            load_key(&env_or("CLAP_CLIENT_KEY", "certs/client.key")),
        )?;
    let connector = TlsConnector::from(Arc::new(config));
    let socket = TcpStream::connect(env_or("RELAY_ADDR", "127.0.0.1:8080")).await?;
    let server_name = ServerName::try_from(env_or("RELAY_SERVER_NAME", "localhost"))?.to_owned();
    let tls = connector.connect(server_name, socket).await?;
    let (mut reader, mut writer) = tokio::io::split(tls);
    let node_id = env_or("CLAP_NODE_ID", "clap-provider");

    send(&mut writer, &Message::Handshake { node_id, role: "clap-provider".into() }).await?;
    match receive(&mut reader).await? {
        Message::Ack { status } if status == "ok" => {}
        other => return Err(format!("handshake rejected: {other:?}").into()),
    }

    let request_topic = env_or("CLAP_REQUEST_TOPIC", "clap.embedding.request");
    send(&mut writer, &Message::Subscribe { topic: request_topic.clone() }).await?;
    match receive(&mut reader).await? {
        Message::Ack { status } if status.starts_with("subscribed:") || status.starts_with("already-subscribed:") => {}
        other => return Err(format!("subscription rejected: {other:?}").into()),
    }
    eprintln!("subscribed to {request_topic}");

    let heartbeat = Duration::from_secs(15);
    loop {
        tokio::select! {
            _ = sleep(heartbeat) => {
                if let Err(error) = send(&mut writer, &Message::Heartbeat { timestamp: 0 }).await {
                    eprintln!("heartbeat send failed; disconnecting: {error}");
                    break;
                }
            }
            incoming = receive(&mut reader) => match incoming {
                Ok(Message::Heartbeat { .. }) => eprintln!("heartbeat acknowledged"),
                Ok(Message::Ack { status }) => eprintln!("ack: {status}"),
                Ok(Message::Payload { topic, data }) if topic == request_topic => {
                    eprintln!("received embedding request: {} bytes", data.len());
                    send(&mut writer, &Message::Payload {
                        topic: "clap.embedding.response".into(),
                        data: format!("processed:{}", data.len()).into_bytes(),
                    }).await?;
                }
                Ok(Message::Payload { topic, .. }) => eprintln!("ignored unsubscribed topic: {topic}"),
                Ok(other) => eprintln!("received control message: {other:?}"),
                Err(error) => {
                    eprintln!("relay disconnected: {error}");
                    break;
                }
            }
        }
    }
    Ok(())
}
