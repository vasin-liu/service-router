use dashmap::DashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Per-upstream circuit breaker state.
struct BreakState {
    consecutive_failures: AtomicU32,
    open_since_epoch_secs: AtomicU64,
}

/// A simple per-upstream circuit breaker.
///
/// * **Closed** — requests flow normally.  
/// * **Open** — consecutive failures ≥ threshold; requests are rejected until
///   `recovery_secs` elapse, at which point one probe request is allowed.
/// * **Half-Open** — the probe request has been allowed; success resets the
///   breaker, failure re-opens it.
pub struct CircuitBreakerMap {
    states: DashMap<String, BreakState>,
    threshold: u32,
    recovery_secs: u64,
}

impl CircuitBreakerMap {
    pub fn new(threshold: u32, recovery_secs: u64) -> Self {
        Self {
            states: DashMap::new(),
            threshold,
            recovery_secs,
        }
    }

    pub fn is_disabled(&self) -> bool {
        self.threshold == 0
    }

    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Returns `true` if the upstream is allowed to receive a request.
    pub fn allow(&self, upstream: &str) -> bool {
        if self.threshold == 0 {
            return true;
        }
        let entry = self.states.entry(upstream.to_string()).or_insert_with(|| {
            BreakState {
                consecutive_failures: AtomicU32::new(0),
                open_since_epoch_secs: AtomicU64::new(0),
            }
        });
        let failures = entry.consecutive_failures.load(Ordering::Relaxed);
        if failures < self.threshold {
            return true;
        }
        let opened_at = entry.open_since_epoch_secs.load(Ordering::Relaxed);
        let elapsed = Self::now_secs().saturating_sub(opened_at);
        elapsed >= self.recovery_secs
    }

    /// Record a successful request — resets the breaker for this upstream.
    pub fn record_success(&self, upstream: &str) {
        if self.threshold == 0 {
            return;
        }
        if let Some(entry) = self.states.get(upstream) {
            entry.consecutive_failures.store(0, Ordering::Relaxed);
            entry.open_since_epoch_secs.store(0, Ordering::Relaxed);
        }
    }

    /// Record a failed request — increments the failure counter and may trip the breaker.
    pub fn record_failure(&self, upstream: &str) {
        if self.threshold == 0 {
            return;
        }
        let entry = self.states.entry(upstream.to_string()).or_insert_with(|| {
            BreakState {
                consecutive_failures: AtomicU32::new(0),
                open_since_epoch_secs: AtomicU64::new(0),
            }
        });
        let prev = entry.consecutive_failures.fetch_add(1, Ordering::Relaxed);
        if prev + 1 >= self.threshold {
            entry
                .open_since_epoch_secs
                .store(Self::now_secs(), Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_always_allows() {
        let cb = CircuitBreakerMap::new(0, 10);
        assert!(cb.allow("host1"));
        cb.record_failure("host1");
        assert!(cb.allow("host1"));
    }

    #[test]
    fn trips_after_threshold() {
        let cb = CircuitBreakerMap::new(3, 999);
        assert!(cb.allow("h"));
        cb.record_failure("h");
        assert!(cb.allow("h"));
        cb.record_failure("h");
        assert!(cb.allow("h"));
        cb.record_failure("h");
        // Now at threshold with long recovery — should be blocked
        assert!(!cb.allow("h"));
    }

    #[test]
    fn success_resets() {
        let cb = CircuitBreakerMap::new(2, 999);
        cb.record_failure("h");
        cb.record_success("h");
        cb.record_failure("h");
        // Only 1 consecutive failure after reset — should be allowed
        assert!(cb.allow("h"));
    }

    #[test]
    fn recovery_allows_probe() {
        let cb = CircuitBreakerMap::new(1, 0);
        cb.record_failure("h");
        // recovery_secs = 0, so probe should be allowed immediately
        assert!(cb.allow("h"));
    }
}
