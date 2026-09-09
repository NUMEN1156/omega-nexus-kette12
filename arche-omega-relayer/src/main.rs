mod config;
mod protocol;
mod relay;

use config::RelayConfig;
use relay::RelayServer;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = RelayConfig::default();
    let server = RelayServer::new(config)?;

    info!("Initializing kette12-relay backbone with mTLS...");
    server.run().await?;

    Ok(())
}
