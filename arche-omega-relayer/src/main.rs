mod config;
mod health;
mod metrics;
mod outbox;
mod protocol;
mod relay;

use config::RelayConfig;
use health::run_health_server;
use metrics::Metrics;
use outbox::{LogSink, OutboxDispatcher, OutboxWriter};
use relay::RelayServer;
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = RelayConfig::default();
    let metrics = Metrics::new();
    let outbox = OutboxWriter::open(&config.outbox_db_path)?;
    let dispatcher = OutboxDispatcher::new(outbox.clone(), Arc::new(LogSink), 100);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            dispatcher.drain_once().await;
        }
    });

    // Health/metrics endpoint on a separate plain-HTTP port
    let health_addr = config.health_addr;
    let metrics_for_health = metrics.clone();
    tokio::spawn(async move {
        if let Err(e) = run_health_server(health_addr, metrics_for_health).await {
            tracing::error!("health server error: {}", e);
        }
    });

    let server = RelayServer::new(config, metrics, outbox)?;

    info!("Initializing kette12-relay backbone with mTLS + metrics...");
    server.run().await?;

    Ok(())
}
