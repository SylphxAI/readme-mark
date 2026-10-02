//! Offline upstream for tests and snapshots: fixed GitHub and npm answers.
//!
//! `cargo test` never touches the network. Subjects:
//! - user `ada-dev` (profile, repos, searches, calendar, GraphQL);
//! - repository `SylphxAI/mark` (repo, release, last commit, workflows
//!   `ci.yml` and never-run `release.yml`, stargazer pages);
//! - npm packages `mark-demo`, `@sylphx/mark-demo`, and `fresh-demo` (no
//!   download counts yet); pub.dev `mark_demo`; Packagist `sylphx/mark`;
//!   Bundlephobia `mark-demo`; Chrome Web Store item
//!   `gighmmpiobklfepjocnamgkkbiglidom`;
//! - any URL naming `ghost-404` is not found; any naming `offline` times out.

use super::upstream::{BoxFut, Call, Reply, Resource, Upstream, UpstreamError};
use crate::capabilities::live::domain::date::{days_from_civil, format_long};
use crate::capabilities::mark::domain::hash::fnv1a_32;

/// The fixed "now" fixtures are recorded against (2026-09-25T12:00:00Z).
pub(crate) fn fixture_now() -> i64 {
    days_from_civil(2026, 9, 25) * 86_400 + 12 * 3600
}

pub(crate) struct FixtureUpstream {
    pub token: bool,
}

const PROFILE: &str = r#"{"login":"ada-dev","name":"Ada Lovelace","followers":318,"public_repos":6,"created_at":"2016-03-14T09:26:53Z"}"#;

const REPOS: &str = r#"[
{"name":"analytical-engine","language":"Rust","size":5200,"stargazers_count":1840,"fork":false},
{"name":"notes-on-bernoulli","language":"TypeScript","size":3100,"stargazers_count":642,"fork":false},
{"name":"loom-patterns","language":"Python","size":2400,"stargazers_count":215,"fork":false},
{"name":"difference-engine","language":"Go","size":900,"stargazers_count":97,"fork":false},
{"name":"dotfiles","language":"Shell","size":120,"stargazers_count":12,"fork":false},
{"name":"forked-lib","language":"C","size":8000,"stargazers_count":3,"fork":true}
]"#;

const REPO: &str = r#"{"name":"mark","owner":{"login":"SylphxAI"},
"description":"Beautiful README images from one URL: banners, badges, typing text, tech icons, and GitHub stats cards. Free, no token, no signup.",
"language":"Rust","stargazers_count":1523,"forks_count":87,"size":2048,"fork":false,"archived":false,"is_template":false,
"license":{"spdx_id":"MIT"},"pushed_at":"2026-09-22T10:00:00Z"}"#;

const GQL_CORE: &str = r#"{"data":{"user":{"name":"Ada Lovelace","login":"ada-dev","createdAt":"2016-03-14T09:26:53Z","followers":{"totalCount":318},
"pullRequests":{"totalCount":142},"openIssues":{"totalCount":7},"closedIssues":{"totalCount":30},
"repositories":{"totalCount":5}}}}"#;

const GQL_STARS: &str = r#"{"data":{"user":{"repositories":{"nodes":[{"name":"analytical-engine","stargazers":{"totalCount":1840}},{"name":"notes-on-bernoulli","stargazers":{"totalCount":642}}]}}}}"#;

const GQL_ACTIVITY: &str = r#"{"data":{"user":{"contributionsCollection":{"totalCommitContributions":1204,"totalPullRequestReviewContributions":88},
"repositoriesContributedTo":{"totalCount":41}}}}"#;

const GQL_LANGS: &str = r##"{"data":{"user":{"repositories":{"nodes":[
{"name":"analytical-engine","languages":{"edges":[{"size":520000,"node":{"color":"#dea584","name":"Rust"}},{"size":40000,"node":{"color":"#89e051","name":"Shell"}}]}},
{"name":"notes-on-bernoulli","languages":{"edges":[{"size":310000,"node":{"color":"#3178c6","name":"TypeScript"}},{"size":20000,"node":{"color":"#663399","name":"CSS"}}]}}
]}}}}"##;

/// Deterministic year of contributions in GitHub's calendar markup.
pub(crate) fn calendar_html() -> String {
    let last = days_from_civil(2026, 9, 25);
    let first = last - 364;
    let mut total = 0u64;
    let mut cells = String::new();
    for (i, day) in (first..=last).enumerate() {
        let h = fnv1a_32(&day.to_le_bytes());
        // A quiet day every so often, an unbroken run over the last 23 days,
        // and a longer run in spring.
        let recent = day > last - 23;
        let spring = (days_from_civil(2026, 3, 2)..days_from_civil(2026, 4, 12)).contains(&day);
        let n = if recent || spring {
            1 + h % 6
        } else if h.is_multiple_of(3) {
            0
        } else {
            h % 9
        };
        total += n as u64;
        let (y, m, d) = crate::capabilities::live::domain::date::civil_from_days(day);
        let label = match n {
            0 => format!("No contributions on {}.", format_long(day)),
            1 => format!("1 contribution on {}.", format_long(day)),
            n => format!("{n} contributions on {}.", format_long(day)),
        };
        cells.push_str(&format!(
            "<td tabindex=\"0\" data-ix=\"{i}\" data-date=\"{y:04}-{m:02}-{d:02}\" id=\"contribution-day-component-{i}\" \
             data-level=\"{}\" class=\"ContributionCalendar-day\"></td>\n\
             <tool-tip id=\"tooltip-{i}\" for=\"contribution-day-component-{i}\" class=\"sr-only\">{label}</tool-tip>\n",
            n.min(4)
        ));
    }
    format!(
        "<h2 id=\"js-contribution-activity-description\" class=\"f4 text-normal mb-2\">\n  {total}\n  contributions\n  in the last year\n</h2>\n<table>{cells}</table>"
    )
}

fn answer(call: &Call, token: bool) -> Result<Reply, UpstreamError> {
    let url = call.url.as_str();
    if url.contains("offline") || call.body.as_deref().is_some_and(|b| b.contains("offline")) {
        return Err(UpstreamError::Timeout);
    }
    if url.contains("ghost-404")
        || call
            .body
            .as_deref()
            .is_some_and(|b| b.contains("ghost-404"))
    {
        return Ok(Reply::NotFound);
    }
    let body = |s: &str| Ok(Reply::Body(s.to_string()));
    match call.resource {
        Resource::Graphql if !token => Err(UpstreamError::NoToken),
        Resource::Graphql => {
            let b = call.body.as_deref().unwrap_or("");
            body(if b.contains("languages(") {
                GQL_LANGS
            } else if b.contains("stargazers{") {
                GQL_STARS
            } else if b.contains("contributionsCollection") {
                GQL_ACTIVITY
            } else {
                GQL_CORE
            })
        }
        Resource::Web if url.ends_with("/users/ada-dev/contributions") => {
            Ok(Reply::Body(calendar_html()))
        }
        Resource::Search if url.contains("type%3Apr") => body(r#"{"total_count":142}"#),
        Resource::Search if url.contains("type%3Aissue") => body(r#"{"total_count":37}"#),
        Resource::Core => match url.trim_start_matches("https://api.github.com") {
            "/users/ada-dev" => body(PROFILE),
            u if u.starts_with("/users/ada-dev/repos?") && u.ends_with("page=1") => body(REPOS),
            "/repos/SylphxAI/mark" => body(REPO),
            "/repos/SylphxAI/mark/releases/latest" => {
                body(r#"{"tag_name":"v1.4.0","prerelease":false}"#)
            }
            "/repos/SylphxAI/mark/releases?per_page=1" => {
                body(r#"[{"tag_name":"v1.5.0-rc.1","prerelease":true}]"#)
            }
            u if u.starts_with("/repos/SylphxAI/mark/commits?per_page=1") => {
                body(r#"[{"commit":{"committer":{"date":"2026-09-22T10:00:00Z"}}}]"#)
            }
            u if u.starts_with("/repos/SylphxAI/mark/actions/workflows/ci.yml/runs?") => body(
                r#"{"workflow_runs":[{"name":"CI","status":"completed","conclusion":"success"}]}"#,
            ),
            u if u.starts_with("/repos/SylphxAI/mark/actions/workflows/release.yml/runs?") => {
                body(r#"{"workflow_runs":[]}"#)
            }
            u if u.starts_with("/repos/SylphxAI/mark/stargazers?per_page=100&page=")
                && call.accept == Some("application/vnd.github.star+json") =>
            {
                let page: i64 = u
                    .rsplit('=')
                    .next()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(1);
                Ok(Reply::Body(stargazer_page(page)))
            }
            _ => Ok(Reply::NotFound),
        },
        Resource::Registry => match url {
            "https://pub.dev/api/packages/mark_demo" => body(r#"{"latest":{"version":"2.1.0"}}"#),
            "https://pub.dev/api/packages/mark_demo/score" => body(
                r#"{"grantedPoints":150,"maxPoints":160,"likeCount":1234,"downloadCount30Days":56789}"#,
            ),
            "https://repo.packagist.org/p2/sylphx/mark.json" => body(
                r#"{"packages":{"sylphx/mark":[{"version":"2.0.0-RC1"},{"version":"1.9.2"}]}}"#,
            ),
            "https://packagist.org/packages/sylphx/mark/stats.json" => {
                body(r#"{"downloads":{"total":1071657,"monthly":21293,"daily":742}}"#)
            }
            "https://bundlephobia.com/api/size?package=mark-demo" => {
                body(r#"{"size":7998,"gzip":3058}"#)
            }
            "https://chromewebstore.google.com/detail/gighmmpiobklfepjocnamgkkbiglidom" => body(
                r#"<script>AF_initDataCallback({key: 'ds:0', hash: '2', data:[["gighmmpiobklfepjocnamgkkbiglidom","i","Demo",4.47,290290,"x","s","u",1,null,null,["c",null,4],1,1,64000000,1,"i",[1,2],"{\"version\": \"6.29.0\"}"]], sideChannel: {}});</script>"#,
            ),
            _ => Ok(Reply::NotFound),
        },
        Resource::Npm => match url {
            "https://registry.npmjs.org/mark-demo/latest" => {
                body(r#"{"version":"2.3.1","license":"MIT"}"#)
            }
            "https://registry.npmjs.org/fresh-demo/latest" => {
                body(r#"{"version":"0.1.0","license":"MIT"}"#)
            }
            "https://registry.npmjs.org/@sylphx%2Fmark-demo/latest" => {
                body(r#"{"version":"0.4.0-beta.2","license":{"type":"Apache-2.0"}}"#)
            }
            u if u.contains("/downloads/point/last-month/") && u.contains("mark-demo") => {
                body(r#"{"downloads":1234567}"#)
            }
            u if u.contains("/downloads/point/last-week/") && u.contains("mark-demo") => {
                body(r#"{"downloads":98765}"#)
            }
            u if u.contains("/downloads/point/1000-01-01:3000-01-01/")
                && u.contains("mark-demo") =>
            {
                body(r#"{"downloads":20345678}"#)
            }
            _ => Ok(Reply::NotFound),
        },
        _ => Ok(Reply::NotFound),
    }
}

/// One page of stargazers: the first starred 30 days after the previous
/// page's, from 2024-01-01.
fn stargazer_page(page: i64) -> String {
    let day = days_from_civil(2024, 1, 1) + (page - 1) * 30;
    let (y, m, d) = crate::capabilities::live::domain::date::civil_from_days(day);
    format!(r#"[{{"starred_at":"{y:04}-{m:02}-{d:02}T12:00:00Z"}}]"#)
}

impl Upstream for FixtureUpstream {
    fn call(&self, call: Call) -> BoxFut<'_, Result<Reply, UpstreamError>> {
        let token = self.token;
        Box::pin(async move { answer(&call, token) })
    }

    fn has_token(&self) -> bool {
        self.token
    }
}
