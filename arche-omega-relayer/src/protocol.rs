use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RelayMessage {
    Handshake { node_id: String, role: String },
    Subscribe { topic: String },
    Unsubscribe { topic: String },
    Heartbeat { timestamp: u64 },
    Payload { topic: String, data: Vec<u8> },
    Ack { status: String },
}

const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;

pub async fn read_message<R: AsyncRead + Unpin>(reader: &mut R) -> Result<RelayMessage, Box<dyn std::error::Error + Send + Sync>> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len == 0 || len > MAX_FRAME_SIZE { return Err(format!("invalid frame length: {}", len).into()); }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).await?;
    Ok(serde_json::from_slice(&body)?)
}

pub async fn write_message<W: AsyncWrite + Unpin>(writer: &mut W, msg: &RelayMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let body = serde_json::to_vec(msg)?;
    if body.len() > MAX_FRAME_SIZE { return Err("message too large".into()); }
    writer.write_all(&(body.len() as u32).to_be_bytes()).await?;
    writer.write_all(&body).await?;
    writer.flush().await?;
    Ok(())
}
