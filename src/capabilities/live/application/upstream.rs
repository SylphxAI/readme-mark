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
    tokens: Vec<String>,
    next: AtomicUsize,
    /// Keyed by (token index, or `None` for anonymous; resource).
    budgets: Mutex<HashMap<(Option<usize>, Resource), Budget>>,
    permits: Semaphore,
}

impl HttpUpstream {
    /// Build the bounded HTTP client. A client that cannot be built is a
    /// startup failure: falling back to a default client would drop every
    /// timeout the adapter promises.
    pub(crate) fn new(tokens: Vec<String>) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(5))
            .user_agent(concat!(
                "readme-mark/",
                env!("CARGO_PKG_VERSION"),
                " (+https://mark.sylphx.com)"
            ))
            .gzip(true)
            .build()?;
        Ok(Self {
            client,
            tokens,
            next: AtomicUsize::new(0),
            budgets: Mutex::new(HashMap::new()),
            permits: Semaphore::new(MAX_CONCURRENT),
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
        if matches!(resource, Resource::Web | Resource::Npm | Resource::Registry) {
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
        let mut req = match &call.body {
            Some(body) => self
                .client
                .post(&call.url)
                .header("content-type", "application/json")
                .body(body.clone()),
            None => self.client.get(&call.url),
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
        if status == 200 && res.content_length().is_some_and(|l| l as usize > MAX_BODY) {
            return Err(UpstreamError::Malformed("body too large".into()));
        }
        // Content-Length is absent (or describes the compressed size) when the
        // body is gzip-encoded, so the cap is enforced on the decoded stream.
        let mut buf = BodyBuf::new(MAX_BODY);
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

    async fn run(&self, call: Call) -> Result<Reply, UpstreamError> {
        let slot = self.pick(call.resource)?;
        let _permit = tokio::time::timeout(Duration::from_secs(2), self.permits.acquire())
            .await
            .map_err(|_| UpstreamError::Busy)?
            .map_err(|_| UpstreamError::Busy)?;
        let started = std::time::Instant::now();
        let mut outcome = self.once(&call, slot).await;
        // One retry for a quick gateway error (GitHub's calendar answers 503
        // under load); never when the first try already spent the budget.
        let quick = started.elapsed() < Duration::from_millis(1500);
        if quick && matches!(outcome, Ok((502..=504, _))) && call.body.is_none() {
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
