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
  
    let sink = Arc::new(EvmSink::from_env().map_err(|error| format!("invalid EVM sink configuration: {error}"))?);
  
    let dispatcher = OutboxDispatcher::new(outbox.clone(), sink, 100);
  
    tokio::spawn(async move {
      
        let mut interval = tok