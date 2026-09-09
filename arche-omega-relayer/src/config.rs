use std::net::SocketAddr;

#[derive(Clone, Debug)]
pub struct RelayConfig {
    pub bind_addr: SocketAddr,
    pub heartbeat_interval_secs: u64,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:8080".parse().unwrap(),
            heartbeat_interval_secs: 15,
        }
    }
}
