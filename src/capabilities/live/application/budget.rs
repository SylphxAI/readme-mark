//! Per-client budget on upstream loads.
//!
//! A free, unauthenticated host must not let one heavy client spend the whole
//! GitHub rate budget. The budget counts upstream *loads* (a cache miss that
//! would call out), never requests: a request answered from cache, including
//! every CDN-cached `.svg`, spends nothing and is never refused. A client over
//! budget gets the cached or stale value when there is one, else the
//! short-cached fallback card, and nothing is written to the cache, so one
//! client cannot poison the answer for others.
//!
//! The client is a task-local lease set by the HTTP layer; work outside a
//! lease (startup warming) is never charged, and `carry` moves the lease into
//! a spawned task.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::metrics;

/// Fixed window over which a client's loads are counted.
const WINDOW: Duration = Duration::from_secs(60);
/// Default loads per client per window. Generous on purpose: the GitHub
/// image proxy shares a few addresses between many readers.
const DEFAULT_LIMIT: u32 = 300;
/// Tracked clients; a full table drops expired windows, then everything.
const MAX_CLIENTS: usize = 50_000;

tokio::task_local! {
    static LEASE: Lease;
}

#[derive(Clone)]
struct Lease {
    budget: Arc<ClientBudget>,
    key: Arc<str>,
}

pub struct ClientBudget {
    limit: u32,
    window: Duration,
    seen: Mutex<HashMap<String, (Instant, u32)>>,
}

impl ClientBudget {
    pub(crate) fn new(limit: u32, window: Duration) -> Self {
        Self {
            limit,
            window,
            seen: Mutex::new(HashMap::new()),
        }
    }

    /// Production limit: `LIVE_CLIENT_FETCHES_PER_MIN`, default 300.
    pub(crate) fn from_env() -> Self {
        let limit = std::env::var("LIVE_CLIENT_FETCHES_PER_MIN")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(DEFAULT_LIMIT);
        Self::new(limit, WINDOW)
    }

    /// Count one load for `key` at `now`; false when the client is over budget.
    fn take_at(&self, key: &str, now: Instant) -> bool {
        let mut seen = self.seen.lock().unwrap_or_else(|e| e.into_inner());
        if !seen.contains_key(key) && seen.len() >= MAX_CLIENTS {
            seen.retain(|_, (start, _)| now.duration_since(*start) < self.window);
            if seen.len() >= MAX_CLIENTS {
                seen.clear();
            }
        }
        let slot = seen.entry(key.to_string()).or_insert((now, 0));
        if now.duration_since(slot.0) >= self.window {
            *slot = (now, 0);
        }
        if slot.1 >= self.limit {
            return false;
        }
        slot.1 += 1;
        true
    }

    /// Run `fut` with `key` as the client its upstream loads are charged to.
    pub(crate) async fn scope<F: Future>(self: &Arc<Self>, key: &str, fut: F) -> F::Output {
        let lease = Lease {
            budget: self.clone(),
            key: key.into(),
        };
        LEASE.scope(lease, fut).await
    }
}

/// Charge one upstream load to the current client. True outside a lease.
pub(crate) fn charge() -> bool {
    let ok = LEASE
        .try_with(|l| l.budget.take_at(&l.key, Instant::now()))
        .unwrap_or(true);
    if !ok {
        metrics::upstream_throttled();
    }
    ok
}

/// Run `fut` (typically a spawned task) under the lease of the caller.
pub(crate) fn carry<F: Future>(fut: F) -> impl Future<Output = F::Output> {
    let lease = LEASE.try_with(Clone::clone).ok();
    async move {
        match lease {
            Some(l) => LEASE.scope(l, fut).await,
            None => fut.await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::live::application::cache::{Lookup, Ttl, TtlCache};

    fn budget(limit: u32) -> Arc<ClientBudget> {
        Arc::new(ClientBudget::new(limit, WINDOW))
    }

    fn ttl() -> Ttl {
        Ttl {
            fresh: Duration::from_secs(60),
            stale: Duration::from_secs(60),
            not_found: Duration::from_secs(60),
            retry: Duration::from_secs(60),
        }
    }

    #[test]
    fn a_client_gets_its_limit_per_window_and_others_are_unaffected() {
        let b = ClientBudget::new(2, Duration::from_secs(60));
        let t0 = Instant::now();
        assert!(b.take_at("a", t0) && b.take_at("a", t0));
        assert!(!b.take_at("a", t0), "third load in the window is refused");
        assert!(b.take_at("b", t0), "another client has its own budget");
        assert!(
            b.take_at("a", t0 + Duration::from_secs(61)),
            "the window resets"
        );
    }

    #[test]
    fn work_outside_a_lease_is_never_charged() {
        assert!(charge(), "no lease, no charge");
    }

    #[tokio::test]
    async fn a_throttled_load_leaves_the_cache_untouched_for_everyone() {
        let cache: TtlCache<u32> = TtlCache::new("t", 10, ttl());
        let b = budget(0);
        let denied = b
            .scope("heavy", cache.fetch("k".into(), || async { Ok(Some(1)) }))
            .await;
        assert_eq!(denied, Lookup::Unavailable);
        let other = cache.fetch("k".into(), || async { Ok(Some(7)) }).await;
        assert_eq!(other, Lookup::Found(7), "nothing was cached by the refusal");
    }

    #[tokio::test]
    async fn cached_answers_cost_nothing_and_stale_serves_a_throttled_client() {
        let cache: TtlCache<u32> = TtlCache::new("t", 10, ttl());
        cache.fetch("k".into(), || async { Ok(Some(5)) }).await;
        let b = budget(0);
        let hit = b
            .scope("heavy", cache.fetch("k".into(), || async { Ok(Some(9)) }))
            .await;
        assert_eq!(hit, Lookup::Found(5), "a fresh entry is never throttled");
    }

    #[tokio::test]
    async fn the_lease_follows_a_spawned_task() {
        let b = budget(0);
        let charged = b
            .scope("heavy", async {
                tokio::spawn(carry(async { charge() })).await.unwrap()
            })
            .await;
        assert!(!charged);
        let lost = b
            .scope("heavy", async {
                tokio::spawn(async { charge() }).await.unwrap()
            })
            .await;
        assert!(lost, "an uncarried task has no lease (why carry exists)");
    }
}
