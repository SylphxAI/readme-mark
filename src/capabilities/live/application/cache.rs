//! Bounded TTL cache with request coalescing and stale-on-error serving.
//!
//! Every live read goes through [`TtlCache::fetch`]:
//! - a fresh entry answers without touching upstream;
//! - a missing or stale entry is (re)loaded under a per-key lock, so a burst
//!   of requests for one user becomes one upstream call;
//! - a failed reload keeps serving the stale value (until `stale` runs out)
//!   and backs off for `retry` before trying upstream again;
//! - "not found" is cached for `not_found`; a failure with nothing cached is
//!   remembered for `retry` so a dead upstream is not hammered.

use moka::future::Cache;
use moka::ops::compute::{CompResult, Op};
use std::future::Future;
use std::time::{Duration, Instant};

use super::upstream::UpstreamError;

/// The answer a live route renders from.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Lookup<V> {
    Found(V),
    Missing,
    Unavailable,
}

impl<V> Lookup<V> {
    pub(crate) fn found(self) -> Option<V> {
        match self {
            Self::Found(v) => Some(v),
            _ => None,
        }
    }
}

/// Cache lifetimes for one kind of data.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ttl {
    pub fresh: Duration,
    pub stale: Duration,
    pub not_found: Duration,
    pub retry: Duration,
}

const HOUR: Duration = Duration::from_secs(3600);

impl Ttl {
    /// Profile-scale data (stats, languages): hours fresh, a week stale.
    pub(crate) const PROFILE: Ttl = Ttl {
        fresh: Duration::from_secs(4 * 3600),
        stale: Duration::from_secs(7 * 24 * 3600),
        not_found: HOUR,
        retry: Duration::from_secs(300),
    };
    /// Badge numbers and the daily calendar: an hour fresh, a week stale.
    /// Build status: minutes fresh, so a red CI shows up quickly.
    pub(crate) const STATUS: Ttl = Ttl {
        fresh: Duration::from_secs(300),
        stale: Duration::from_secs(24 * 3600),
        not_found: HOUR,
        retry: Duration::from_secs(60),
    };
    /// Slow-moving history (star charts): a day fresh.
    pub(crate) const DAILY: Ttl = Ttl {
        fresh: Duration::from_secs(24 * 3600),
        stale: Duration::from_secs(14 * 24 * 3600),
        not_found: HOUR,
        retry: Duration::from_secs(600),
    };
    pub(crate) const BADGE: Ttl = Ttl {
        fresh: HOUR,
        stale: Duration::from_secs(7 * 24 * 3600),
        not_found: HOUR,
        retry: Duration::from_secs(300),
    };
}

#[derive(Clone)]
enum State<V> {
    Found(V),
    Missing,
    Failed,
}

#[derive(Clone)]
struct Entry<V> {
    state: State<V>,
    fresh_until: Instant,
    stale_until: Instant,
}

impl<V: Clone> Entry<V> {
    fn lookup(&self) -> Lookup<V> {
        match &self.state {
            State::Found(v) => Lookup::Found(v.clone()),
            State::Missing => Lookup::Missing,
            State::Failed => Lookup::Unavailable,
        }
    }
}

#[derive(Clone)]
pub(crate) struct TtlCache<V> {
    name: &'static str,
    inner: Cache<String, Entry<V>>,
    ttl: Ttl,
}

impl<V: Clone + Send + Sync + 'static> TtlCache<V> {
    pub(crate) fn new(name: &'static str, capacity: u64, ttl: Ttl) -> Self {
        Self {
            name,
            inner: Cache::builder()
                .max_capacity(capacity)
                .time_to_live(ttl.stale)
                .build(),
            ttl,
        }
    }

    /// Whether `key` holds a fresh entry (a read that would not call upstream).
    pub(crate) async fn is_fresh(&self, key: &str) -> bool {
        self.inner
            .get(key)
            .await
            .is_some_and(|e| e.fresh_until > Instant::now())
    }

    /// Cached value for `key`, loading it with `load` when missing or stale.
    ///
    /// `load` answers `Ok(Some(v))` (found), `Ok(None)` (does not exist
    /// upstream), or an error (upstream failed).
    pub(crate) async fn fetch<F, Fut>(&self, key: String, load: F) -> Lookup<V>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<V>, UpstreamError>>,
    {
        let now = Instant::now();
        if let Some(e) = self.inner.get(&key).await {
            if e.fresh_until > now {
                tracing::debug!(cache = self.name, %key, "live cache hit");
                return e.lookup();
            }
        }
        let ttl = self.ttl;
        let name = self.name;
        let log_key = key.clone();
        let result = self
            .inner
            .entry(key)
            .and_compute_with(|existing| async move {
                let now = Instant::now();
                let old = existing
                    .map(|e| e.into_value())
                    .filter(|e| e.stale_until > now);
                if let Some(e) = &old {
                    if e.fresh_until > now {
                        // Another request refreshed it while this one waited.
                        return Op::Nop;
                    }
                }
                if !super::budget::charge() {
                    // This client is over its upstream budget: keep whatever
                    // is cached (stale or nothing) and cache no failure, so
                    // other clients still load on their own budget.
                    return Op::Nop;
                }
                match load().await {
                    Ok(v) => {
                        let (state, fresh) = match v {
                            Some(v) => (State::Found(v), ttl.fresh),
                            None => (State::Missing, ttl.not_found),
                        };
                        Op::Put(Entry {
                            state,
                            fresh_until: now + fresh,
                            stale_until: now + ttl.stale.max(fresh),
                        })
                    }
                    Err(err) => {
                        tracing::info!(cache = name, key = %log_key, error = %err, stale = old.is_some(), "live upstream failed");
                        match old {
                            // Serve stale, and back off before retrying upstream.
                            Some(e) if !matches!(e.state, State::Failed) => Op::Put(Entry {
                                fresh_until: now + ttl.retry,
                                ..e
                            }),
                            _ => Op::Put(Entry {
                                state: State::Failed,
                                fresh_until: now + ttl.retry.min(Duration::from_secs(60)),
                                stale_until: now + ttl.retry.min(Duration::from_secs(60)),
                            }),
                        }
                    }
                }
            })
            .await;
        match result {
            CompResult::Inserted(e) | CompResult::ReplacedWith(e) | CompResult::Unchanged(e) => {
                e.into_value().lookup()
            }
            CompResult::StillNone(_) | CompResult::Removed(_) => Lookup::Unavailable,
        }
    }
}

impl<V: Clone + Send + Sync + 'static> TtlCache<V> {
    /// Like [`TtlCache::fetch`], but waits at most `wait`: the load keeps
    /// running in the background and fills the cache for the next request,
    /// while this one answers `Unavailable` (an optional card row hides).
    pub(crate) async fn fetch_detached<F, Fut>(
        &self,
        key: String,
        wait: Duration,
        load: F,
    ) -> Lookup<V>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Option<V>, UpstreamError>> + Send + 'static,
    {
        let this = self.clone();
        let task = tokio::spawn(super::budget::carry(
            async move { this.fetch(key, load).await },
        ));
        match tokio::time::timeout(wait, task).await {
            Ok(Ok(v)) => v,
            _ => Lookup::Unavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn short() -> Ttl {
        Ttl {
            fresh: Duration::ZERO,
            stale: Duration::from_secs(60),
            not_found: Duration::from_secs(60),
            retry: Duration::ZERO,
        }
    }

    #[tokio::test]
    async fn fresh_hits_skip_upstream_and_misses_are_cached() {
        let cache: TtlCache<u32> = TtlCache::new("t", 10, Ttl::BADGE);
        let calls = Arc::new(AtomicUsize::new(0));
        for _ in 0..3 {
            let c = calls.clone();
            let v = cache
                .fetch("k".into(), || async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok(Some(7))
                })
                .await;
            assert_eq!(v, Lookup::Found(7));
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let missing = cache.fetch("gone".into(), || async { Ok(None) }).await;
        assert_eq!(missing, Lookup::Missing);
        let again = cache.fetch("gone".into(), || async { Ok(Some(1)) }).await;
        assert_eq!(again, Lookup::Missing, "not-found is cached");
    }

    #[tokio::test]
    async fn stale_value_survives_upstream_failure() {
        let cache: TtlCache<u32> = TtlCache::new("t", 10, short());
        assert_eq!(
            cache.fetch("k".into(), || async { Ok(Some(1)) }).await,
            Lookup::Found(1)
        );
        let v = cache
            .fetch("k".into(), || async { Err(UpstreamError::Timeout) })
            .await;
        assert_eq!(v, Lookup::Found(1), "stale on error");
        let none = cache
            .fetch("other".into(), || async { Err(UpstreamError::Timeout) })
            .await;
        assert_eq!(none, Lookup::Unavailable);
    }

    #[tokio::test]
    async fn detached_loads_finish_in_the_background() {
        let cache: TtlCache<u32> = TtlCache::new("t", 10, Ttl::BADGE);
        let slow = cache
            .fetch_detached("k".into(), Duration::from_millis(10), || async {
                tokio::time::sleep(Duration::from_millis(50)).await;
                Ok(Some(9))
            })
            .await;
        assert_eq!(slow, Lookup::Unavailable, "the request does not wait");
        tokio::time::sleep(Duration::from_millis(100)).await;
        let later = cache.fetch("k".into(), || async { Ok(Some(0)) }).await;
        assert_eq!(
            later,
            Lookup::Found(9),
            "the background load filled the cache"
        );
    }
}
