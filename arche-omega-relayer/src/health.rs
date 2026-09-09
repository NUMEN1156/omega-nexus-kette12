use std::sync::Arc;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tracing::{error, info};

use crate::metrics::{HealthResponse, Metrics};

/// Minimal plain-HTTP health/metrics server (no external HTTP framework).
pub async fn run_health_server(
    addr: std::net::SocketAddr,
    metrics: Arc<Metrics>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(addr).await?;
    info!("health/metrics endpoint listening on http://{}", addr);

    loop {
        let (mut stream, peer) = listener.accept().await?;
        let metrics = metrics.clone();

        tokio::spawn(async move {
            // Read and discard the request (we only care about any HTTP request).
            let mut buf = [0u8; 1024];
            let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await;

            let body = match serde_json::to_string(&HealthResponse {
                status: "ok",
                metrics: metrics.snapshot(),
            }) {
                Ok(b) => b,
                Err(e) => {
                    error!("failed to serialize health response: {}", e);
                    return;
                }
            };

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );

            if let Err(e) = stream.write_all(response.as_bytes()).await {
                error!("health write error to {}: {}", peer, e);
            }
        });
    }
}
