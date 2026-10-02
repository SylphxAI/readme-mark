//! Live service: cached readers composed into card and badge data.
//!
//! Upstream call budget per cold request (anonymous path):
//! - stats: profile (1 core) → repos (1–2 core), calendar (1 web),
//!   PR and issue counts (2 search), in parallel after the profile answers;
//! - top languages: repos (1–2 core, shared with stats);
//! - streak: calendar (1 web, shared with stats);
//! - repo card, GitHub badges: 1 core each (the repo read is shared);
//! - npm badges: 1 registry or downloads call.
//!
//! With a server token, stats and languages are one GraphQL call each, and
//! fall back to the anonymous path when GraphQL is unavailable.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use super::cache::{Lookup, Ttl, TtlCache};
use super::fixtures::{fixture_now, FixtureUpstream};
use super::github::{self, GqlActivity, GqlCore, GqlLang, GqlStars, Profile, RepoLite};
use super::npm;
use super::registries;
use super::upstream::{unix_now, HttpUpstream, Upstream, UpstreamError};

/// How long a stats card waits for an optional part (stars, commits, PRs,
/// issues) before rendering without it. The parts of one card run in parallel,
/// so this bounds the whole card; a slower part still fills the cache.
const OPTIONAL_WAIT: Duration = Duration::from_millis(1500);
use crate::capabilities::live::domain::calendar::{summarize, Calendar, StreakSummary};
use crate::capabilities::live::domain::languages::aggregate;
use crate::capabilities::live::domain::model::{
    BundleSize, ChromeItem, CommitSource, LangSource, NpmPackage, PackagistDownloads, PubScore,
    RepoInfo, StarHistory, TopLangs, UserStats, WorkflowRun,
};

/// Process-wide live data: the upstream port, a clock, and bounded caches.
pub struct LiveService {
    up: Arc<dyn Upstream>,
    clock: fn() -> i64,
    profiles: TtlCache<Profile>,
    repos: TtlCache<Vec<RepoLite>>,
    counts: TtlCache<u64>,
    calendars: TtlCache<Calendar>,
    gql_core: TtlCache<GqlCore>,
    gql_stars: TtlCache<GqlStars>,
    gql_activity: TtlCache<GqlActivity>,
    gql_langs: TtlCache<Vec<GqlLang>>,
    repo: TtlCache<RepoInfo>,
    releases: TtlCache<Option<(String, bool)>>,
    commits: TtlCache<Option<String>>,
    packages: TtlCache<NpmPackage>,
    downloads: TtlCache<u64>,
    versions: TtlCache<String>,
    pub_scores: TtlCache<PubScore>,
    packagist: TtlCache<PackagistDownloads>,
    bundles: TtlCache<BundleSize>,
    chrome: TtlCache<ChromeItem>,
    workflows: TtlCache<Option<WorkflowRun>>,
    star_history: TtlCache<StarHistory>,
    pub(super) switch: super::switch_service::SwitchCaches,
}

impl LiveService {
    fn build(up: Arc<dyn Upstream>, clock: fn() -> i64) -> Self {
        Self {
            up,
            clock,
            profiles: TtlCache::new("profile", 2000, Ttl::PROFILE),
            repos: TtlCache::new("repos", 1000, Ttl::PROFILE),
            counts: TtlCache::new("counts", 4000, Ttl::PROFILE),
            calendars: TtlCache::new("calendar", 2000, Ttl::BADGE),
            gql_core: TtlCache::new("gql-core", 2000, Ttl::PROFILE),
            gql_stars: TtlCache::new("gql-stars", 2000, Ttl::PROFILE),
            gql_activity: TtlCache::new("gql-activity", 2000, Ttl::PROFILE),
            gql_langs: TtlCache::new("gql-langs", 1000, Ttl::PROFILE),
            repo: TtlCache::new("repo", 2000, Ttl::BADGE),
            releases: TtlCache::new("release", 1000, Ttl::BADGE),
            commits: TtlCache::new("commit", 1000, Ttl::BADGE),
            packages: TtlCache::new("npm", 1000, Ttl::BADGE),
            downloads: TtlCache::new("npm-dl", 2000, Ttl::BADGE),
            versions: TtlCache::new("registry-version", 2000, Ttl::BADGE),
            pub_scores: TtlCache::new("pub-score", 1000, Ttl::BADGE),
            packagist: TtlCache::new("packagist-dl", 1000, Ttl::BADGE),
            bundles: TtlCache::new("bundle", 1000, Ttl::PROFILE),
            chrome: TtlCache::new("chrome", 500, Ttl::PROFILE),
            workflows: TtlCache::new("workflow", 2000, Ttl::STATUS),
            star_history: TtlCache::new("star-history", 500, Ttl::DAILY),
            switch: super::switch_service::SwitchCaches::new(),
        }
    }

    /// Production wiring: real HTTP, optional server tokens from the env.
    ///
    /// Fails when the bounded HTTP client cannot be built, so startup never
    /// serves live cards through a client without timeouts.
    pub fn from_env() -> Result<Self, reqwest::Error> {
        let tokens = HttpUpstream::tokens_from_env();
        tracing::info!(tokens = tokens.len(), "live upstream configured");
        let up = Arc::new(HttpUpstream::new(tokens)?);
        up.keep_warm();
        Ok(Self::build(up, unix_now))
    }

    /// Offline fixtures and a fixed clock (anonymous path).
    pub fn for_tests() -> Self {
        Self::build(Arc::new(FixtureUpstream { token: false }), fixture_now)
    }

    /// Offline fixtures on the server-token (GraphQL) path.
    pub fn for_tests_with_token() -> Self {
        Self::build(Arc::new(FixtureUpstream { token: true }), fixture_now)
    }

    pub(crate) fn now_unix(&self) -> i64 {
        (self.clock)()
    }

    pub(super) fn up(&self) -> &dyn Upstream {
        self.up.as_ref()
    }

    // duplicate-exception: cache-read wrappers bind one TtlCache to one upstream reader under a normalized key; the shared shape is the design (one line of body each).
    async fn calendar(&self, login: &str) -> Lookup<Calendar> {
        let key = login.to_ascii_lowercase();
        self.calendars
            .fetch(key, || github::calendar(self.up(), login))
            .await
    }

    async fn owned_repos(&self, login: &str) -> Lookup<Vec<RepoLite>> {
        let key = login.to_ascii_lowercase();
        self.repos
            .fetch(key, || github::owned_repos(self.up(), login))
            .await
    }

    /// A stats-card part that may hide: waits at most [`OPTIONAL_WAIT`], and
    /// a slower upstream still fills the cache for the next request.
    async fn optional<V, F, Fut>(&self, cache: &TtlCache<V>, key: String, read: F) -> Option<V>
    where
        V: Clone + Send + Sync + 'static,
        F: FnOnce(Arc<dyn Upstream>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<Option<V>, UpstreamError>> + Send + 'static,
    {
        let up = self.up.clone();
        cache
            .fetch_detached(key, OPTIONAL_WAIT, move || read(up))
            .await
            .found()
    }

    /// Whether the stats card for `login` would answer from a fresh cache
    /// entry (for the `Server-Timing` header).
    pub(crate) async fn stats_cached(&self, login: &str) -> bool {
        let key = login.to_ascii_lowercase();
        if self.up.has_token() {
            self.gql_core.is_fresh(&key).await
        } else {
            self.profiles.is_fresh(&key).await
        }
    }

    pub(crate) async fn stats(&self, login: &str, exclude: &[String]) -> Lookup<UserStats> {
        let up = self.up.as_ref();
        let key = login.to_ascii_lowercase();
        let excluded = |name: &str| exclude.iter().any(|e| e.eq_ignore_ascii_case(name));
        if up.has_token() {
            let (l1, l2) = (login.to_string(), login.to_string());
            // The cheap core is required; the star list and the contribution
            // collection are the slow fields on a large account, so they are
            // optional parts like the anonymous path's.
            let (core, stars, activity) = tokio::join!(
                self.gql_core
                    .fetch(key.clone(), || github::gql_core(up, login)),
                self.optional(&self.gql_stars, key.clone(), move |up| async move {
                    github::gql_stars(up.as_ref(), &l1).await
                }),
                self.optional(&self.gql_activity, key.clone(), move |up| async move {
                    github::gql_activity(up.as_ref(), &l2).await
                }),
            );
            match core {
                Lookup::Found(g) => {
                    return Lookup::Found(UserStats {
                        stars: stars.map(|rs| {
                            rs.iter()
                                .filter(|(n, _)| !excluded(n))
                                .map(|(_, s)| s)
                                .sum()
                        }),
                        login: g.login,
                        name: g.name,
                        followers: g.followers,
                        repos: Some(g.repo_count),
                        created_at: g.created_at,
                        commits: activity
                            .as_ref()
                            .map(|a| (a.commits, CommitSource::CommitsLastYear)),
                        prs: Some(g.prs),
                        issues: Some(g.issues),
                        reviews: activity.as_ref().map(|a| a.reviews),
                        contributed_to: activity.map(|a| a.contributed_to),
                    })
                }
                Lookup::Missing => return Lookup::Missing,
                Lookup::Unavailable => {}
            }
        }
        let key = login.to_ascii_lowercase();
        let (l1, l2, l3, l4) = (
            login.to_string(),
            login.to_string(),
            login.to_string(),
            login.to_string(),
        );
        // One round trip: the profile and the optional parts start together
        // (bounded: five calls, under the adapter's concurrency cap), so a
        // cold card no longer pays the profile read before the rest.
        let (profile, repos, calendar, prs, issues) = tokio::join!(
            self.profiles
                .fetch(key.clone(), || github::profile(up, login)),
            self.optional(&self.repos, key.clone(), move |up| async move {
                github::owned_repos(up.as_ref(), &l1).await
            }),
            self.optional(&self.calendars, key.clone(), move |up| async move {
                github::calendar(up.as_ref(), &l2).await
            }),
            self.optional(&self.counts, format!("pr:{key}"), move |up| async move {
                github::search_count(up.as_ref(), &format!("author:{l3} type:pr")).await
            }),
            self.optional(&self.counts, format!("issue:{key}"), move |up| async move {
                github::search_count(up.as_ref(), &format!("author:{l4} type:issue")).await
            }),
        );
        let profile = match profile {
            Lookup::Found(p) => p,
            Lookup::Missing => return Lookup::Missing,
            Lookup::Unavailable => return Lookup::Unavailable,
        };
        Lookup::Found(UserStats {
            stars: repos.map(|rs| {
                rs.iter()
                    .filter(|r| !excluded(&r.name))
                    .map(|r| r.stars)
                    .sum()
            }),
            commits: calendar.map(|c| (c.total, CommitSource::ContributionsLastYear)),
            login: profile.login,
            name: profile.name,
            followers: profile.followers,
            repos: Some(profile.public_repos),
            created_at: profile.created_at,
            prs,
            issues,
            reviews: None,
            contributed_to: None,
        })
    }

    /// Top languages. Anonymous path: each owned non-fork repository's primary
    /// language weighted by its size (an approximation of bytes per language).
    pub(crate) async fn top_langs(
        &self,
        login: &str,
        exclude: &[String],
        hide: &[String],
    ) -> Lookup<TopLangs> {
        let up = self.up.as_ref();
        let excluded = |name: &str| exclude.iter().any(|e| e.eq_ignore_ascii_case(name));
        if up.has_token() {
            let key = login.to_ascii_lowercase();
            match self
                .gql_langs
                .fetch(key, || github::gql_langs(up, login))
                .await
            {
                Lookup::Found(rows) => {
                    let samples = rows
                        .iter()
                        .filter(|r| !excluded(&r.0))
                        .map(|r| (r.1.as_str(), r.2.as_deref(), r.3));
                    return Lookup::Found(TopLangs {
                        login: login.to_string(),
                        langs: aggregate(samples, hide),
                        source: LangSource::Bytes,
                    });
                }
                Lookup::Missing => return Lookup::Missing,
                Lookup::Unavailable => {}
            }
        }
        match self.owned_repos(login).await {
            Lookup::Found(repos) => {
                let samples = repos
                    .iter()
                    .filter(|r| !r.fork && !excluded(&r.name))
                    .filter_map(|r| r.language.as_deref().map(|l| (l, None, r.size.max(1))));
                Lookup::Found(TopLangs {
                    login: login.to_string(),
                    langs: aggregate(samples, hide),
                    source: LangSource::PrimaryBySize,
                })
            }
            Lookup::Missing => Lookup::Missing,
            Lookup::Unavailable => Lookup::Unavailable,
        }
    }

    pub(crate) async fn streak(&self, login: &str) -> Lookup<StreakSummary> {
        match self.calendar(login).await {
            Lookup::Found(c) => Lookup::Found(summarize(&c)),
            Lookup::Missing => Lookup::Missing,
            Lookup::Unavailable => Lookup::Unavailable,
        }
    }

    pub(crate) async fn repo(&self, owner: &str, name: &str) -> Lookup<RepoInfo> {
        let key = format!("{owner}/{name}").to_ascii_lowercase();
        self.repo
            .fetch(key, || github::repo(self.up(), owner, name))
            .await
    }

    /// Latest release; `Found(None)` when the repository has none.
    // duplicate-exception: cache-read wrappers bind one TtlCache to one upstream reader under a normalized key; the shared shape is the design (one line of body each).
    pub(crate) async fn release(
        &self,
        owner: &str,
        name: &str,
        pre: bool,
    ) -> Lookup<Option<(String, bool)>> {
        let key = format!("{owner}/{name}:{pre}").to_ascii_lowercase();
        let load = || github::release(self.up(), owner, name, pre);
        self.releases.fetch(key, load).await
    }

    /// Newest commit date; `Found(None)` for an empty repository.
    pub(crate) async fn last_commit(
        &self,
        owner: &str,
        name: &str,
        branch: Option<&str>,
    ) -> Lookup<Option<String>> {
        let key = format!("{owner}/{name}@{}", branch.unwrap_or("")).to_ascii_lowercase();
        let load = || github::last_commit(self.up(), owner, name, branch);
        self.commits.fetch(key, load).await
    }

    // duplicate-exception: cache-read wrappers bind one TtlCache to one upstream reader under a normalized key; the shared shape is the design (one line of body each).
    pub(crate) async fn npm_package(&self, name: &str, tag: &str) -> Lookup<NpmPackage> {
        let load = || npm::package(self.up(), name, tag);
        self.packages.fetch(format!("{name}@{tag}"), load).await
    }

    pub(crate) async fn npm_downloads(&self, name: &str, period: &str) -> Lookup<u64> {
        let load = || npm::downloads(self.up(), name, period);
        self.downloads.fetch(format!("{name}:{period}"), load).await
    }

    // duplicate-exception: cache-read wrapper binding one TtlCache to one registry reader.
    /// Latest pub.dev version.
    pub(crate) async fn pub_version(&self, name: &str) -> Lookup<String> {
        let load = || registries::pub_version(self.up(), name);
        self.versions.fetch(format!("pub:{name}"), load).await
    }

    // duplicate-exception: cache-read wrapper binding one TtlCache to one registry reader.
    pub(crate) async fn pub_score(&self, name: &str) -> Lookup<PubScore> {
        let load = || registries::pub_score(self.up(), name);
        self.pub_scores.fetch(name.to_string(), load).await
    }

    // duplicate-exception: cache-read wrapper binding one TtlCache to one registry reader.
    /// Latest stable Packagist version.
    pub(crate) async fn packagist_version(&self, vendor: &str, package: &str) -> Lookup<String> {
        let load = || registries::packagist_version(self.up(), vendor, package);
        let key = format!("packagist:{vendor}/{package}");
        self.versions.fetch(key, load).await
    }

    // duplicate-exception: cache-read wrapper binding one TtlCache to one registry reader.
    pub(crate) async fn packagist_downloads(
        &self,
        vendor: &str,
        package: &str,
    ) -> Lookup<PackagistDownloads> {
        let load = || registries::packagist_downloads(self.up(), vendor, package);
        self.packagist
            .fetch(format!("{vendor}/{package}"), load)
            .await
    }

    // duplicate-exception: cache-read wrapper binding one TtlCache to one registry reader.
    pub(crate) async fn bundle_size(&self, package: &str) -> Lookup<BundleSize> {
        let load = || registries::bundle_size(self.up(), package);
        self.bundles.fetch(package.to_string(), load).await
    }

    // duplicate-exception: cache-read wrapper binding one TtlCache to one registry reader.
    pub(crate) async fn chrome_item(&self, id: &str) -> Lookup<ChromeItem> {
        let load = || registries::chrome_item(self.up(), id);
        self.chrome.fetch(id.to_string(), load).await
    }

    /// Newest run of a workflow; `Found(None)` when it never ran.
    pub(crate) async fn workflow_run(
        &self,
        owner: &str,
        name: &str,
        file: &str,
        branch: Option<&str>,
        event: Option<&str>,
    ) -> Lookup<Option<WorkflowRun>> {
        let key = format!(
            "{owner}/{name}/{file}@{}#{}",
            branch.unwrap_or(""),
            event.unwrap_or("")
        )
        .to_ascii_lowercase();
        let load = || github::workflow_run(self.up(), owner, name, file, branch, event);
        self.workflows.fetch(key, load).await
    }

    /// Star counts over time, sampled from the stargazer pages.
    pub(crate) async fn star_history(&self, owner: &str, name: &str) -> Lookup<StarHistory> {
        let key = format!("{owner}/{name}").to_ascii_lowercase();
        let samples = if self.up.has_token() { 16 } else { 8 };
        let now = self.now_unix();
        let load = || star_samples(self.up.clone(), owner, name, samples, now);
        self.star_history.fetch(key, load).await
    }
}

/// GitHub lists at most 400 stargazer pages of 100.
const MAX_STAR_PAGES: u64 = 400;

/// Sample `count` stargazer pages evenly: stargazer `i` on page `p` is star
/// number `(p - 1) * 100 + i + 1`, at their `starred_at`. The last point is
/// today's total.
async fn star_samples(
    up: Arc<dyn Upstream>,
    owner: &str,
    name: &str,
    count: u64,
    now: i64,
) -> Result<Option<StarHistory>, UpstreamError> {
    let Some(info) = github::repo(up.as_ref(), owner, name).await? else {
        return Ok(None);
    };
    let pages = info.stars.div_ceil(100).clamp(1, MAX_STAR_PAGES);
    let mut picks: Vec<u64> = (0..count.min(pages))
        .map(|i| 1 + i * (pages - 1) / count.min(pages).saturating_sub(1).max(1))
        .collect();
    picks.dedup();
    let mut set = tokio::task::JoinSet::new();
    for page in picks {
        let (up, owner, name) = (up.clone(), owner.to_string(), name.to_string());
        set.spawn(async move {
            let dates = github::stargazer_page(up.as_ref(), &owner, &name, page).await;
            (page, dates)
        });
    }
    let mut points = Vec::new();
    while let Some(joined) = set.join_next().await {
        let Ok((page, dates)) = joined else { continue };
        for (i, date) in dates?.iter().enumerate() {
            if let Some(t) = crate::capabilities::live::domain::date::parse_timestamp(date) {
                points.push((t, (page - 1) * 100 + i as u64 + 1));
            }
        }
    }
    points.sort();
    // Keep the chart light: at most ~80 points, the newest always kept.
    let stride = points.len().div_ceil(80).max(1);
    let mut points: Vec<(i64, u64)> = points
        .iter()
        .enumerate()
        .filter(|(i, _)| i % stride == 0)
        .map(|(_, p)| *p)
        .collect();
    points.push((now, info.stars));
    Ok(Some(StarHistory {
        owner: info.owner,
        repo: info.name,
        points,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::live::application::upstream::{BoxFut, Call, Reply};

    /// Fixture answers after a fixed network delay.
    struct Slow(FixtureUpstream, Duration);

    impl Upstream for Slow {
        fn call(&self, call: Call) -> BoxFut<'_, Result<Reply, UpstreamError>> {
            let (inner, delay) = (self.0.call(call), self.1);
            Box::pin(async move {
                tokio::time::sleep(delay).await;
                inner.await
            })
        }

        fn has_token(&self) -> bool {
            self.0.has_token()
        }
    }

    /// A cold anonymous card costs one round trip, not profile + the rest.
    #[tokio::test]
    async fn cold_anonymous_stats_fan_out_in_one_round_trip() {
        let delay = Duration::from_millis(200);
        let live = LiveService::build(
            Arc::new(Slow(FixtureUpstream { token: false }, delay)),
            fixture_now,
        );
        let started = std::time::Instant::now();
        let found = live.stats("ada-dev", &[]).await;
        let took = started.elapsed();
        eprintln!("cold stats with {delay:?} per call: {took:?}");
        assert!(matches!(found, Lookup::Found(_)));
        assert!(took < delay * 3 / 2, "calls must overlap, took {took:?}");
    }

    #[tokio::test]
    async fn anonymous_stats_compose_rest_search_and_calendar() {
        let live = LiveService::for_tests();
        let Lookup::Found(s) = live.stats("ada-dev", &[]).await else {
            panic!("fixture user resolves");
        };
        assert_eq!(s.name.as_deref(), Some("Ada Lovelace"));
        assert_eq!(s.stars, Some(1840 + 642 + 215 + 97 + 12 + 3));
        assert_eq!(s.prs, Some(142));
        assert!(matches!(
            s.commits,
            Some((_, CommitSource::ContributionsLastYear))
        ));
        assert_eq!(s.contributed_to, None);
        assert_eq!(live.stats("ghost-404", &[]).await, Lookup::Missing);
        assert_eq!(live.stats("offline-user", &[]).await, Lookup::Unavailable);
    }

    #[tokio::test]
    async fn token_path_uses_graphql() {
        let live = LiveService::for_tests_with_token();
        let Lookup::Found(s) = live.stats("ada-dev", &["notes-on-bernoulli".into()]).await else {
            panic!("fixture user resolves");
        };
        assert_eq!(s.stars, Some(1840));
        assert_eq!(s.contributed_to, Some(41));
        let Lookup::Found(l) = live.top_langs("ada-dev", &[], &[]).await else {
            panic!("langs resolve");
        };
        assert_eq!(l.source, LangSource::Bytes);
        assert_eq!(l.langs[0].name, "Rust");
    }

    #[tokio::test]
    async fn token_stats_carry_every_part_and_cache_state() {
        let live = LiveService::for_tests_with_token();
        assert!(!live.stats_cached("ada-dev").await);
        let Lookup::Found(s) = live.stats("ada-dev", &[]).await else {
            panic!("fixture user resolves");
        };
        assert_eq!(s.commits.map(|c| c.0), Some(1204));
        assert_eq!(s.reviews, Some(88));
        assert_eq!(s.prs, Some(142));
        assert!(live.stats_cached("ada-dev").await);
    }

    #[tokio::test]
    async fn anonymous_langs_skip_forks_and_weight_by_size() {
        let live = LiveService::for_tests();
        let Lookup::Found(l) = live.top_langs("ada-dev", &[], &["go".into()]).await else {
            panic!("langs resolve");
        };
        assert_eq!(l.source, LangSource::PrimaryBySize);
        let names: Vec<&str> = l.langs.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["Rust", "TypeScript", "Python", "Shell"]);
    }
}
