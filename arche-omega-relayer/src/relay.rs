use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::config::RelayConfig;
use tracing::{info, error};

pub struct RelayServer {
    config: RelayConfig,
}

impl RelayServer {
    pub fn new(config: RelayConfig) -> Self {
        Self { config }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(&self.config.bind_addr).await?;
        info!("==> kette12-relay listening on {}", self.config.bind_addr);

        loop {
            let (stream, addr) = listener.accept().await?;
            info!("Incoming connection from: {}", addr);

            tokio::spawn(async move {
                if let Err(e) = handle_client(stream).await {
                    error!("Error handling client {}: {}", addr, e);
                }
            });
        }
    }
}

async fn handle_client(mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let mut buf = vec![0u8; 1024];

    loop {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            break; // Connection closed
        }

        // Echo back / basic frame parsing stub
        stream.write_all(&buf[..n]).await?;
    }

    Ok(())
}
