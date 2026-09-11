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
    pub total_skill_requests: AtomicU64,
    pub total_skill_results: AtomicU64,
    pub total_skill_successes: AtomicU64,
    pub total_skill_failures: AtomicU64,
    pub total_skill_timeouts: AtomicU64,
    pub total_skill_denied: AtomicU64,
    pub total_skill_rejections: AtomicU64,
    pub total_browser_skill_requests: AtomicU64,
    pub total_scientific_skill_requests: AtomicU64,
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
            total_skill_requests: self.total_skill_requests.load(Ordering::Relaxed),
            total_skill_results: self.total_skill_results.load(Ordering::Relaxed),
            total_skill_successes: self.total_skill_successes.load(Ordering::Relaxed),
            total_skill_failures: self.total_skill_failures.load(Ordering::Relaxed),
            total_skill_timeouts: self.total_skill_timeouts.load(Ordering::Relaxed),
            total_skill_denied: self.total_skill_denied.load(Ordering::Relaxed),
            total_skill_rejections: self.total_skill_rejections.load(Ordering::Relaxed),
            total_browser_skill_requests: self
                .total_browser_skill_requests
                .load(Ordering::Relaxed),
            total_scientific_skill_requests: self
                .total_scientific_skill_requests
                .load(Ordering::Relaxed),
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
    pub total_skill_requests: u64,
    pub total_skill_results: u64,
    pub total_skill_successes: u64,
    pub total_skill_failures: u64,
    pub total_skill_timeouts: u64,
    pub total_skill_denied: u64,
    pub total_skill_rejections: u64,
    pub total_browser_skill_requests: u64,
    pub total_scientific_skill_requests: u64,
}

#[derive(serde::Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub metrics: MetricsSnapshot,
}
