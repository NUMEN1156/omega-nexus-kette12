mod config;
mod evm_sink;
mod health;
mod metrics;
mod outbox;
mod protocol;
mod relay;

use config::RelayConfig;
use evm_sink::EvmSink;
use health::run_health_server;
use metrics::Metrics;
use outbox::{OutboxDispatcher, OutboxWriter};
use relay::RelayServer;
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = RelayConfig::default();
    let metrics = Metrics::new();
    let outbox = OutboxWriter::open(&config.outbox_db_path)?;
    let sink = Arc::new(
        EvmSink::from_env().map_err(|error| format!("invalid EVM sink configuration: {error}"))?,
    );
    let dispatcher = OutboxDispatcher::new(outbox.clone(), sink, 100);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            dispatcher.drain_once().await;
        }
    });

    let health_addr = config.health_addr;
    let metrics_for_health = metrics.clone();
    tokio::spawn(async move {
        if let Err(error) = run_health_server(health_addr, metrics_for_health).await {
            tracing::error!(%error, "health server error");
        }
    });

    let server = RelayServer::new(config, metrics, outbox)?;
    info!("Initializing kette12-relay backbone with mTLS, metrics, outbox, and EVM sink...");
    server.run().await?;

    Ok(())
}
