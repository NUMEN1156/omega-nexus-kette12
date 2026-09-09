mod config;
mod health;
mod metrics;
mod protocol;
mod relay;

use config::RelayConfig;
use health::run_health_server;
use metrics::Metrics;
use relay::RelayServer;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = RelayConfig::default();
    let metrics = Metrics::new();

    // Health/metrics endpoint on a separate plain-HTTP port
    let health_addr = config.health_addr;
    let metrics_for_health = metrics.clone();
    tokio::spawn(async move {
        if let Err(e) = run_health_server(health_addr, metrics_for_health).await {
            tracing::error!("health server error: {}", e);
        }
    });

    let server = RelayServer::new(config, metrics)?;

    info!("Initializing kette12-relay backbone with mTLS + metrics...");
    server.run().await?;

    Ok(())
}
