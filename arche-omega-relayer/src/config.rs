use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct RelayConfig {
    pub bind_addr: SocketAddr,
    /// Plain HTTP health/metrics endpoint (no TLS).
    pub health_addr: SocketAddr,
    /// Expected client heartbeat interval (seconds).
    pub heartbeat_interval_secs: u64,
    /// Maximum time (seconds) without any message before the session is considered dead.
    pub session_timeout_secs: u64,
    /// Server certificate (PEM)
    pub cert_path: PathBuf,
    /// Server private key (PEM)
    pub key_path: PathBuf,
    /// CA certificate used to verify client certificates (mTLS)
    pub ca_cert_path: PathBuf,
    /// SQLite file used by the durable outbox.
    pub outbox_db_path: PathBuf,
    /// Topics persisted to the outbox; empty means persist every payload.
    pub outbox_topic_filters: Vec<String>,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:8080".parse().unwrap(),
            health_addr: "0.0.0.0:9090".parse().unwrap(),
            heartbeat_interval_secs: 15,
            session_timeout_secs: 45,
            cert_path: PathBuf::from("certs/server.crt"),
            key_path: PathBuf::from("certs/server.key"),
            ca_cert_path: PathBuf::from("certs/ca.crt"),
            outbox_db_path: PathBuf::from("data/outbox.sqlite3"),
            outbox_topic_filters: std::env::var("OUTBOX_TOPIC_FILTERS")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|topic| !topic.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
        }
    }
}
