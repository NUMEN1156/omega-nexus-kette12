use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RelayMessage {
    Handshake { node_id: String, role: String },
    Heartbeat { timestamp: u64 },
    Payload { topic: String, data: Vec<u8> },
    Ack { status: String },
}

/// Length-prefixed JSON framing (u32 big-endian length + UTF-8 JSON body).
const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024; // 16 MiB hard limit

pub async fn read_message<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<RelayMessage, Box<dyn std::error::Error + Send + Sync>> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;

    if len == 0 || len > MAX_FRAME_SIZE {
        return Err(format!("invalid frame length: {}", len).into());
    }

    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).await?;

    let msg: RelayMessage = serde_json::from_slice(&body)?;
    Ok(msg)
}

pub async fn write_message<W: AsyncWrite + Unpin>(
    writer: &mut W,
    msg: &RelayMessage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let body = serde_json::to_vec(msg)?;
    if body.len() > MAX_FRAME_SIZE {
        return Err("message too large".into());
    }

    let len = (body.len() as u32).to_be_bytes();
    writer.write_all(&len).await?;
    writer.write_all(&body).await?;
    writer.flush().await?;
    Ok(())
}
