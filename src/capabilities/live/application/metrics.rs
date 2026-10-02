//! Abuse and degradation counters, counts only (no client, login or URL).
//!
//! The share of stats cards answered with the short-cached fallback is
//! `stats_fallback / stats_total`; `upstream_throttled` counts upstream loads
//! refused by the per-client budget. One structured line every
//! [`LOG_EVERY`] stats requests carries the running totals.

use std::sync::atomic::{AtomicU64, Ordering};

/// Stats requests between two log lines.
const LOG_EVERY: u64 = 100;

pub(crate) struct Counts {
    stats: AtomicU64,
    fallback: AtomicU64,
    throttled: AtomicU64,
}

impl Counts {
    pub(crate) const fn new() -> Self {
        Self {
            stats: AtomicU64::new(0),
            fallback: AtomicU64::new(0),
            throttled: AtomicU64::new(0),
        }
    }

    /// Count one stats answer. Every [`LOG_EVERY`]th call returns the totals
    /// `(stats, fallback, throttled)` to log.
    pub(crate) fn record_stats(&self, fallback: bool) -> Option<(u64, u64, u64)> {
        let fb = if fallback {
            self.fallback.fetch_add(1, Ordering::Relaxed) + 1
        } else {
            self.fallback.load(Ordering::Relaxed)
        };
        let total = self.stats.fetch_add(1, Ordering::Relaxed) + 1;
        (total % LOG_EVERY == 0).then(|| (total, fb, self.throttled.load(Ordering::Relaxed)))
    }

    pub(crate) fn record_throttled(&self) {
        self.throttled.fetch_add(1, Ordering::Relaxed);
    }
}

static COUNTS: Counts = Counts::new();

/// A stats card was answered; `fallback` is true for the short-cached stand-in.
pub(crate) fn stats_served(fallback: bool) {
    if let Some((total, fallbacks, throttled)) = COUNTS.record_stats(fallback) {
        tracing::info!(
            target: "mark::metrics",
            stats_total = total,
            stats_fallback = fallbacks,
            upstream_throttled = throttled,
            "stats fallback share (counts)"
        );
    }
}

/// An upstream load was refused by the per-client budget.
pub(crate) fn upstream_throttled() {
    COUNTS.record_throttled();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totals_are_logged_every_hundredth_stats_request() {
        let c = Counts::new();
        c.record_throttled();
        let mut logged = Vec::new();
        for i in 1..=200u64 {
            if let Some(t) = c.record_stats(i % 4 == 0) {
                logged.push(t);
            }
        }
        assert_eq!(logged, vec![(100, 25, 1), (200, 50, 1)]);
    }
}
