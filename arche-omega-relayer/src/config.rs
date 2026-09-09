use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct RelayConfig {
    pub bind_addr: SocketAddr,
    /// Expected client heartbeat interval (seconds). Used for logging / future pacing.
    pub heartbeat_interval_secs: u64,
    /// Maximum time (seconds) without any message before the session is considered dead.
    /// Typically 2–3× heartbeat_interval_secs.
    pub session_timeout_secs: u64,
    /// Server certificate (PEM)
    pub cert_path: PathBuf,
    /// Server private key (PEM)
    pub key_path: PathBuf,
    /// CA certificate used to verify client certificates (mTLS)
    pub ca_cert_path: PathBuf,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:8080".parse().unwrap(),
            heartbeat_interval_secs: 15,
            session_timeout_secs: 45, // 3× default heartbeat
            cert_path: PathBuf::from("certs/server.crt"),
            key_path: PathBuf::from("certs/server.key"),
            ca_cert_path: PathBuf::from("certs/ca.crt"),
        }
    }
}
