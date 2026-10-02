//! Upstream port and its HTTP adapter (GitHub REST/GraphQL/web, npm).
//!
//! The adapter bounds every call: connect/total timeouts, a process-wide
//! concurrency cap, one retry on a 5xx gateway error, and per-resource
//! rate-limit bookkeeping from GitHub's `X-RateLimit-*` headers so an
//! exhausted budget short-circuits instead of spending a round trip.
//! Optional server tokens (`GITHUB_TOKEN` / `GITHUB_TOKENS`) rotate
//! round-robin; without one every call is anonymous.

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Semaphore;

/// Which upstream budget a call spends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Resource {
    /// GitHub REST core (60/h anonymous, 5000/h per token).
    Core,
    /// GitHub REST search (10/min anonymous, 30/min per token).
    Search,
    /// GitHub GraphQL (token only).
    Graphql,
    /// Public github.com HTML (the contributions calendar); no API quota.
    Web,
    /// npm registry and downloads API.
    Npm,
    /// Other public registries (pub.dev, Packagist, Bundlephobia, Chrome Web
    /// Store); no token, no tracked quota.
    Registry,
    /// A user-chosen `https` URL (the shields `endpoint` badge). Served by a
    /// separate client: no redirects, public addresses only, a small body cap.
    Endpoint,
}

#[derive(Debug, Clone)]
pub(crate) struct Call {
    pub resource: Resource,
    pub url: String,
    /// GraphQL request body; `None` for GETs.
    pub body: Option<String>,
    /// A GitHub media type other than the default JSON one (e.g. the
    /// stargazer timestamps of `application/vnd.github.star+json`).
    pub accept: Option<&'static str>,
}

impl Call {
    pub(crate) fn read(resource: Resource, url: String) -> Self {
        Self {
            resource,
            url,
            body: None,
            accept: None,
        }
    }

    pub(crate) fn accepting(mut self, media_type: &'static str) -> Self {
        self.accept = Some(media_type);
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Reply {
    Body(String),
    NotFound,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum UpstreamError {
    RateLimited,
    Timeout,
    Busy,
    Status(u16),
    Transport(String),
    Malformed(String),
    NoToken,
}

impl fmt::Display for UpstreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RateLimited => write!(f, "rate limited"),
            Self::Timeout => write!(f, "timeout"),
            Self::Busy => write!(f, "too many concurrent upstream calls"),
            Self::Status(s) => write!(f, "upstream status {s}"),
            Self::Transport(e) => write!(f, "transport: {e}"),
            Self::Malformed(e) => write!(f, "malformed upstream body: {e}"),
            Self::NoToken => write!(f, "GraphQL needs a server token"),
        }
    }
}

pub(crate) type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// The one port live data is read through (HTTP in production, fixtures in
/// tests).
pub(crate) trait Upstream: Send + Sync {
    fn call(&self, call: Call) -> BoxFut<'_, Result<Reply, UpstreamError>>;
    /// Whether a server token is configured (enables GraphQL).
    fn has_token(&self) -> bool;
}

/// The body of a `200`, or `None` for not found.
pub(crate) async fn read_body(
    up: &dyn Upstream,
    call: Call,
) -> Result<Option<String>, UpstreamError> {
    Ok(match up.call(call).await? {
        Reply::Body(b) => Some(b),
        Reply::NotFound => None,
    })
}

/// A JSON body, or `None` for not found.
pub(crate) async fn read_json(
    up: &dyn Upstream,
    call: Call,
) -> Result<Option<serde_json::Value>, UpstreamError> {
    match read_body(up, call).await? {
        Some(b) => serde_json::from_str(&b)
            .map(Some)
            .map_err(|e| UpstreamError::Malformed(e.to_string())),
        None => Ok(None),
    }
}

pub(crate) fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Remaining calls and reset time for one (token, resource) budget.
#[derive(Debug, Clone, Copy)]
struct Budget {
    remaining: i64,
    reset: i64,
}

pub(crate) const MAX_CONCURRENT: usize = 16;
const MAX_BODY: usize = 4 * 1024 * 1024;
/// Identifies us to upstreams that ask for a contact (crates.io policy).
const USER_AGENT: &str = concat!(
    "readme-mark/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/SylphxAI/readme-mark; hi@sylphx.com)"
);
/// Endpoint fetches in flight, all hosts: its own pool, so a tarpit host can
/// never starve GitHub or registry calls.
const ENDPOINT_CONCURRENT: usize = 8;
/// Endpoint fetches in flight per target host.
const ENDPOINT_PER_HOST: usize = 2;
/// crates.io asks for about one request per second.
const CRATES_INTERVAL: Duration = Duration::from_secs(1);
/// Longest a request waits for a crates.io slot before answering `Busy`.
const CRATES_MAX_WAIT: Duration = Duration::from_secs(2);

/// One endpoint fetch's claim on its host's concurrency; released on drop.
struct HostSlot<'a> {
    hosts: &'a Mutex<HashMap<String, usize>>,
    host: String,
}

impl Drop for HostSlot<'_> {
    fn drop(&mut self) {
        let mut hosts = self.hosts.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(n) = hosts.get_mut(&self.host) {
            *n -= 1;
            if *n == 0 {
                hosts.remove(&self.host);
            }
        }
    }
}

/// An endpoint document is a few hundred bytes; 32 KiB is already generous.
const ENDPOINT_MAX_BODY: usize = 32 * 1024;

/// Resolves names for endpoint calls and drops every address that is not
/// globally routable. The connector dials exactly what this returns, so a
/// name that points (or later rebinds) at a private address cannot be reached.
struct PublicOnly;

impl reqwest::dns::Resolve for PublicOnly {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        Box::pin(async move {
            let host = name.as_str().to_string();
            let public: Vec<std::net::SocketAddr> = tokio::net::lookup_host((host.as_str(), 443))
                .await?
                .filter(|a| crate::capabilities::live::domain::endpoint::is_public_ip(a.ip()))
                .collect();
            if public.is_empty() {
                return Err("no public address".into());
            }
            Ok(Box::new(public.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

/// A decoded response body that refuses to grow past its cap.
struct BodyBuf {
    bytes: Vec<u8>,
    cap: usize,
}

impl BodyBuf {
    fn new(cap: usize) -> Self {
        Self {
            bytes: Vec::new(),
            cap,
        }
    }

    fn push(&mut self, chunk: &[u8]) -> Result<(), UpstreamError> {
        if self.bytes.len().saturating_add(chunk.len()) > self.cap {
            return Err(UpstreamError::Malformed("body too large".into()));
        }
        self.bytes.extend_from_slice(chunk);
        Ok(())
    }

    fn into_string(self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }
}

pub(crate) struct HttpUpstream {
    client: reqwest::Client,
    /// Hardened client for [`Resource::Endpoint`].
    endpoint_client: reqwest::Client,
    tokens: Vec<String>,
    next: AtomicUsize,
    /// Keyed by (token index, or `None` for anonymous; resource).
    budgets: Mutex<HashMap<(Option<usize>, Resource), Budget>>,
    permits: Semaphore,
    endpoint_permits: Semaphore,
    endpoint_hosts: Mutex<HashMap<String, usize>>,
    /// Earliest start of the next crates.io request.
    crates_next: Mutex<std::time::Instant>,
}

impl HttpUpstream {
    /// Build the bounded HTTP client. A client that cannot be built is a
    /// startup failure: falling back to a default client would drop every
    /// timeout the adapter promises.
    pub(crate) fn new(tokens: Vec<String>) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(5))
            .user_agent(USER_AGENT)
            .gzip(true)
            .build()?;
        let endpoint_client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(3))
            .user_agent(USER_AGENT)
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .dns_resolver(std::sync::Arc::new(PublicOnly))
            .gzip(true)
            .build()?;
        Ok(Self {
            client,
            endpoint_client,
            tokens,
            next: AtomicUsize::new(0),
            budgets: Mutex::new(HashMap::new()),
            permits: Semaphore::new(MAX_CONCURRENT),
            endpoint_permits: Semaphore::new(ENDPOINT_CONCURRENT),
            endpoint_hosts: Mutex::new(HashMap::new()),
            crates_next: Mutex::new(std::time::Instant::now()),
        })
    }

    /// Keep a TLS connection to `api.github.com` open: a cold card otherwise
    /// pays DNS, TCP and TLS on its first call. `/rate_limit` spends no quota.
    /// Without a runtime (unit tests) this does nothing.
    pub(crate) fn keep_warm(self: &std::sync::Arc<Self>) {
        if tokio::runtime::Handle::try_current().is_err() {
            return;
        }
        let this = self.clone();
        tokio::spawn(async move {
            loop {
                let _ = this
                    .client
                    .head("https://api.github.com/rate_limit")
                    .send()
                    .await;
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        });
    }

    /// Tokens from `GITHUB_TOKENS` (comma-separated) and `GITHUB_TOKEN`.
    pub(crate) fn tokens_from_env() -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for key in ["GITHUB_TOKENS", "GITHUB_TOKEN"] {
            if let Ok(v) = std::env::var(key) {
                for t in v.split(',').map(str::trim).filter(|t| !t.is_empty()) {
                    if !out.iter().any(|o| o == t) {
                        out.push(t.to_string());
                    }
                }
            }
        }
        out
    }

    #[cfg(test)]
    fn crates_slot_reserve_only(&self) -> bool {
        let mut next = self.crates_next.lock().unwrap();
        let now = std::time::Instant::now();
        let start = (*next).max(now);
        if start - now > CRATES_MAX_WAIT {
            return false;
        }
        *next = start + CRATES_INTERVAL;
        true
    }

    fn spendable(&self, slot: Option<usize>, resource: Resource, now: i64) -> bool {
        let budgets = self.budgets.lock().unwrap_or_else(|e| e.into_inner());
        match budgets.get(&(slot, resource)) {
            Some(b) => b.remaining > 0 || b.reset <= now,
            None => true,
        }
    }

    /// Pick the identity for a GitHub API call: the next token with budget
    /// left, else anonymous (REST only), else rate limited.
    fn pick(&self, resource: Resource) -> Result<Option<usize>, UpstreamError> {
        let now = unix_now();
        if matches!(
            resource,
            Resource::Web | Resource::Npm | Resource::Registry | Resource::Endpoint
        ) {
            return Ok(None);
        }
        let n = self.tokens.len();
        let base = self.next.fetch_add(1, Ordering::Relaxed);
        for i in 0..n {
            let slot = (base + i) % n;
            if self.spendable(Some(slot), resource, now) {
                return Ok(Some(slot));
            }
        }
        if resource == Resource::Graphql {
            return Err(if n == 0 {
                UpstreamError::NoToken
            } else {
                UpstreamError::RateLimited
            });
        }
        if self.spendable(None, resource, now) {
            Ok(None)
        } else {
            Err(UpstreamError::RateLimited)
        }
    }

    fn record(
        &self,
        slot: Option<usize>,
        resource: Resource,
        headers: &reqwest::header::HeaderMap,
    ) {
        let num = |k: &str| {
            headers
                .get(k)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<i64>().ok())
        };
        if let (Some(remaining), Some(reset)) =
            (num("x-ratelimit-remaining"), num("x-ratelimit-reset"))
        {
            let mut budgets = self.budgets.lock().unwrap_or_else(|e| e.into_inner());
            budgets.insert((slot, resource), Budget { remaining, reset });
            if remaining == 0 {
                tracing::info!(
                    ?resource,
                    token = slot.is_some(),
                    reset,
                    "GitHub rate limit exhausted"
                );
            }
        }
    }

    fn exhaust(&self, slot: Option<usize>, resource: Resource, seconds: i64) {
        let mut budgets = self.budgets.lock().unwrap_or_else(|e| e.into_inner());
        budgets.insert(
            (slot, resource),
            Budget {
                remaining: 0,
                reset: unix_now() + seconds,
            },
        );
    }

    async fn once(&self, call: &Call, slot: Option<usize>) -> Result<(u16, String), UpstreamError> {
        let (client, cap) = if call.resource == Resource::Endpoint {
            (&self.endpoint_client, ENDPOINT_MAX_BODY)
        } else {
            (&self.client, MAX_BODY)
        };
        let mut req = match &call.body {
            Some(body) => client
                .post(&call.url)
                .header("content-type", "application/json")
                .body(body.clone()),
            None => client.get(&call.url),
        };
        if matches!(call.resource, Resource::Core | Resource::Search) {
            req = req
                .header(
                    "accept",
                    call.accept.unwrap_or("application/vnd.github+json"),
                )
                .header("x-github-api-version", "2022-11-28");
        }
        if let Some(i) = slot {
            req = req.bearer_auth(&self.tokens[i]);
        }
        let mut res = req.send().await.map_err(|e| {
            if e.is_timeout() {
                UpstreamError::Timeout
            } else {
                UpstreamError::Transport(e.without_url().to_string())
            }
        })?;
        let status = res.status().as_u16();
        self.record(slot, call.resource, res.headers());
        if status == 200 && res.content_length().is_some_and(|l| l as usize > cap) {
            return Err(UpstreamError::Malformed("body too large".into()));
        }
        // Content-Length is absent (or describes the compressed size) when the
        // body is gzip-encoded, so the cap is enforced on the decoded stream.
        let mut buf = BodyBuf::new(cap);
        loop {
            match res.chunk().await {
                Ok(Some(chunk)) => buf.push(&chunk)?,
                Ok(None) => break,
                Err(e) if e.is_timeout() => return Err(UpstreamError::Timeout),
                Err(e) => return Err(UpstreamError::Transport(e.without_url().to_string())),
            }
        }
        let body = buf.into_string();
        Ok((status, body))
    }

    /// Claim one of the host's endpoint slots, or `Busy`.
    fn host_slot(&self, url: &str) -> Result<HostSlot<'_>, UpstreamError> {
        let host = reqwest::Url::parse(url)
            .ok()
            .and_then(|u| {
                u.host_str()
                    .map(|h| h.trim_end_matches('.').to_ascii_lowercase())
            })
            .ok_or(UpstreamError::Busy)?;
        let mut hosts = self
            .endpoint_hosts
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let n = hosts.entry(host.clone()).or_insert(0);
        if *n >= ENDPOINT_PER_HOST {
            return Err(UpstreamError::Busy);
        }
        *n += 1;
        Ok(HostSlot {
            hosts: &self.endpoint_hosts,
            host,
        })
    }

    /// Reserve the next crates.io slot (a token bucket of one per second) and
    /// wait for it; `Busy` when the queue is already too long.
    async fn crates_slot(&self) -> Result<(), UpstreamError> {
        let wait = {
            let mut next = self.crates_next.lock().unwrap_or_else(|e| e.into_inner());
            let now = std::time::Instant::now();
            let start = (*next).max(now);
            let wait = start - now;
            if wait > CRATES_MAX_WAIT {
                return Err(UpstreamError::Busy);
            }
            *next = start + CRATES_INTERVAL;
            wait
        };
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
        Ok(())
    }

    async fn run(&self, call: Call) -> Result<Reply, UpstreamError> {
        // Second layer behind the route's own check: a literal-IP host skips
        // the resolver, so the vetting is repeated at the one place that dials.
        if call.resource == Resource::Endpoint
            && crate::capabilities::live::domain::endpoint::check_url(&call.url).is_err()
        {
            return Err(UpstreamError::Transport("endpoint url refused".into()));
        }
        let slot = self.pick(call.resource)?;
        let endpoint = call.resource == Resource::Endpoint;
        // Endpoint fetches never spend crates.io pacing slots, so they cannot starve real crate badges.
        if !endpoint && call.url.starts_with("https://crates.io/") {
            self.crates_slot().await?;
        }
        let _host_slot = if endpoint {
            Some(self.host_slot(&call.url)?)
        } else {
            None
        };
        let _permit = if endpoint {
            // Never wait: a slow endpoint host must not queue up behind itself.
            self.endpoint_permits
                .try_acquire()
                .map_err(|_| UpstreamError::Busy)?
        } else {
            tokio::time::timeout(Duration::from_secs(2), self.permits.acquire())
                .await
                .map_err(|_| UpstreamError::Busy)?
                .map_err(|_| UpstreamError::Busy)?
        };
        let started = std::time::Instant::now();
        let mut outcome = self.once(&call, slot).await;
        // One retry for a quick gateway error (GitHub's calendar answers 503
        // under load); never when the first try already spent the budget.
        let quick = started.elapsed() < Duration::from_millis(1500);
        if quick && !endpoint && matches!(outcome, Ok((502..=504, _))) && call.body.is_none() {
            outcome = self.once(&call, slot).await;
        }
        tracing::debug!(
            resource = ?call.resource,
            token = slot.is_some(),
            ms = started.elapsed().as_millis() as u64,
            status = outcome.as_ref().map(|o| o.0).unwrap_or(0),
            "live upstream call"
        );
        let (status, body) = outcome?;
        match status {
            200 => Ok(Reply::Body(body)),
            404 | 410 | 451 => Ok(Reply::NotFound),
            401 => {
                // A revoked token must not keep failing calls: park it.
                self.exhaust(slot, call.resource, 3600);
                Err(UpstreamError::Status(401))
            }
            403 | 429 if body.contains("rate limit") || status == 429 => {
                self.exhaust(slot, call.resource, 60);
                Err(UpstreamError::RateLimited)
            }
            s => Err(UpstreamError::Status(s)),
        }
    }
}

impl Upstream for HttpUpstream {
    fn call(&self, call: Call) -> BoxFut<'_, Result<Reply, UpstreamError>> {
        Box::pin(self.run(call))
    }

    fn has_token(&self) -> bool {
        !self.tokens.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anonymous_graphql_is_refused_without_a_round_trip() {
        let up = HttpUpstream::new(Vec::new()).expect("client builds");
        assert_eq!(up.pick(Resource::Graphql), Err(UpstreamError::NoToken));
        assert_eq!(up.pick(Resource::Core), Ok(None));
        up.exhaust(None, Resource::Core, 60);
        assert_eq!(up.pick(Resource::Core), Err(UpstreamError::RateLimited));
        assert_eq!(
            up.pick(Resource::Search),
            Ok(None),
            "budgets are per resource"
        );
    }

    #[tokio::test]
    async fn endpoint_calls_never_dial_private_or_plain_targets() {
        let up = HttpUpstream::new(Vec::new()).expect("client builds");
        for url in [
            "https://127.0.0.1/x",
            "https://[::1]/x",
            "https://169.254.169.254/x",
            "http://example.com/x",
            "https://localhost/x",
        ] {
            let got = up.call(Call::read(Resource::Endpoint, url.into())).await;
            assert_eq!(
                got,
                Err(UpstreamError::Transport("endpoint url refused".into())),
                "{url}"
            );
        }
    }

    #[tokio::test]
    async fn a_full_endpoint_pool_never_blocks_other_upstreams() {
        let up = HttpUpstream::new(Vec::new()).expect("client builds");
        let _held: Vec<_> = (0..ENDPOINT_CONCURRENT)
            .map(|_| up.endpoint_permits.try_acquire().expect("free"))
            .collect();
        let got = up
            .call(Call::read(
                Resource::Endpoint,
                "https://example.com/x".into(),
            ))
            .await;
        assert_eq!(got, Err(UpstreamError::Busy), "endpoint pool is full");
        assert!(
            up.permits.try_acquire().is_ok(),
            "the shared pool still has a permit for a Core call"
        );
    }

    #[tokio::test]
    async fn endpoint_calls_never_take_crates_pacing_slots() {
        let up = HttpUpstream::new(Vec::new()).expect("client builds");
        let before = *up.crates_next.lock().unwrap();
        // The call itself fails (no network in tests); only the pacing state matters.
        let _ = up
            .call(Call::read(
                Resource::Endpoint,
                "https://crates.io/api/v1/crates/serde".into(),
            ))
            .await;
        assert_eq!(
            *up.crates_next.lock().unwrap(),
            before,
            "endpoint left crates.io pacing untouched"
        );
    }

    #[test]
    fn one_host_gets_two_endpoint_slots() {
        let up = HttpUpstream::new(Vec::new()).expect("client builds");
        let a = up.host_slot("https://slow.example.com/a").expect("first");
        let _b = up.host_slot("https://SLOW.example.com/b").expect("second");
        assert!(up.host_slot("https://slow.example.com/c").is_err());
        assert!(up.host_slot("https://other.example.com/c").is_ok());
        drop(a);
        assert!(up.host_slot("https://slow.example.com/c").is_ok());
    }

    #[tokio::test]
    async fn crates_io_is_paced_to_one_request_per_second() {
        let up = HttpUpstream::new(Vec::new()).expect("client builds");
        // Slots at +0, +1, +2 s are granted; the fourth would wait 3 s.
        for _ in 0..3 {
            assert!(up.crates_slot_reserve_only());
        }
        assert!(!up.crates_slot_reserve_only(), "queue too long: Busy");
    }

    #[tokio::test]
    async fn the_endpoint_resolver_drops_non_public_addresses() {
        use reqwest::dns::Resolve;
        let name: reqwest::dns::Name = "localhost".parse().expect("a valid name");
        assert!(
            PublicOnly.resolve(name).await.is_err(),
            "localhost has no public address"
        );
    }

    #[test]
    fn decoded_body_is_capped_while_streaming() {
        let mut buf = BodyBuf::new(8);
        assert_eq!(buf.push(b"12345"), Ok(()));
        assert_eq!(buf.push(b"678"), Ok(()), "exactly the cap is allowed");
        assert_eq!(
            buf.push(b"9"),
            Err(UpstreamError::Malformed("body too large".into())),
            "one byte past the cap fails without buffering it"
        );
        assert_eq!(buf.into_string(), "12345678");
    }

    #[test]
    fn tokens_rotate_and_skip_exhausted_ones() {
        let up = HttpUpstream::new(vec!["a".into(), "b".into()]).expect("client builds");
        up.exhaust(Some(0), Resource::Core, 60);
        for _ in 0..4 {
            assert_eq!(up.pick(Resource::Core), Ok(Some(1)));
        }
        up.exhaust(Some(1), Resource::Core, 60);
        assert_eq!(up.pick(Resource::Core), Ok(None), "falls back to anonymous");
        assert!(
            matches!(up.pick(Resource::Graphql), Ok(Some(_))),
            "budgets are per resource"
        );
    }
}
