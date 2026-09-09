use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Process-wide relay metrics (lock-free).
#[derive(Default)]
pub struct Metrics {
    pub active_sessions: AtomicU64,
    pub total_handshakes: AtomicU64,
    pub total_payloads_in: AtomicU64,
    pub total_payloads_out: AtomicU64,
    pub total_subscribes: AtomicU64,
    pub total_unsubscribes: AtomicU64,
    pub total_timeouts: AtomicU64,
    pub total_lagged: AtomicU64,
}

impl Metrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            active_sessions: self.active_sessions.load(Ordering::Relaxed),
            total_handshakes: self.total_handshakes.load(Ordering::Relaxed),
            total_payloads_in: self.total_payloads_in.load(Ordering::Relaxed),
            total_payloads_out: self.total_payloads_out.load(Ordering::Relaxed),
            total_subscribes: self.total_subscribes.load(Ordering::Relaxed),
            total_unsubscribes: self.total_unsubscribes.load(Ordering::Relaxed),
            total_timeouts: self.total_timeouts.load(Ordering::Relaxed),
            total_lagged: self.total_lagged.load(Ordering::Relaxed),
        }
    }
}

#[derive(serde::Serialize)]
pub struct MetricsSnapshot {
    pub active_sessions: u64,
    pub total_handshakes: u64,
    pub total_payloads_in: u64,
    pub total_payloads_out: u64,
    pub total_subscribes: u64,
    pub total_unsubscribes: u64,
    pub total_timeouts: u64,
    pub total_lagged: u64,
}

#[derive(serde::Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub metrics: MetricsSnapshot,
}
