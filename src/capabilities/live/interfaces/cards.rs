//! Card routes: github-readme-stats and streak-stats dialects plus the native
//! `/api/v1/card/{kind}` surface. Every answer is `200 image/svg+xml`.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::time::Instant;

use crate::bootstrap::AppState;
use crate::capabilities::live::application::cache::Lookup;
use crate::capabilities::live::application::metrics;
use crate::capabilities::live::domain::card::{notice, CardStyle};
use crate::capabilities::live::domain::langs_card::{self, LangsOptions, Layout};
use crate::capabilities::live::domain::palette::ColorOverrides;
use crate::capabilities::live::domain::repo_card::{self, RepoOptions};
use crate::capabilities::live::domain::star_chart;
use crate::capabilities::live::domain::stats_card::{self, StatsOptions};
use crate::capabilities::live::domain::streak_card::{self, StreakColors};
use crate::capabilities::live::domain::trophy_card::{self, TrophyOptions};
use crate::capabilities::mark::domain::cap_text;
use crate::interfaces::http::response::{
    decode_text, if_none_match, parse_bool, svg_response_cached, CachePolicy,
};

/// Every card knob, github-readme-stats and streak-stats spellings. All
/// strings: a malformed value normalizes to its default instead of a 400.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct CardQuery {
    pub username: Option<String>,
    pub user: Option<String>,
    pub repo: Option<String>,
    pub theme: Option<String>,
    pub title_color: Option<String>,
    pub text_color: Option<String>,
    pub icon_color: Option<String>,
    pub bg_color: Option<String>,
    pub border_color: Option<String>,
    pub hide_border: Option<String>,
    pub border_radius: Option<String>,
    pub hide: Option<String>,
    pub show_icons: Option<String>,
    pub hide_title: Option<String>,
    pub hide_rank: Option<String>,
    pub hide_progress: Option<String>,
    pub custom_title: Option<String>,
    pub card_width: Option<String>,
    pub layout: Option<String>,
    pub langs_count: Option<String>,
    pub exclude_repo: Option<String>,
    pub show_owner: Option<String>,
    pub description_lines_count: Option<String>,
    // github-readme-streak-stats
    pub background: Option<String>,
    pub border: Option<String>,
    pub stroke: Option<String>,
    pub ring: Option<String>,
    pub fire: Option<String>,
    #[serde(rename = "currStreakNum")]
    pub curr_streak_num: Option<String>,
    #[serde(rename = "sideNums")]
    pub side_nums: Option<String>,
    #[serde(rename = "currStreakLabel")]
    pub curr_streak_label: Option<String>,
    #[serde(rename = "sideLabels")]
    pub side_labels: Option<String>,
    pub dates: Option<String>,
    // github-profile-trophy
    pub column: Option<String>,
    pub row: Option<String>,
    #[serde(rename = "margin-w")]
    pub margin_w: Option<String>,
    #[serde(rename = "margin-h")]
    pub margin_h: Option<String>,
    #[serde(rename = "no-bg")]
    pub no_bg: Option<String>,
    #[serde(rename = "no-frame")]
    pub no_frame: Option<String>,
    pub title: Option<String>,
    pub rank: Option<String>,
    // star-history.com (`repos=owner/name[,…]`; the first repository is drawn)
    pub repos: Option<String>,
}

/// A GitHub login: 1–39 ASCII letters, digits, or hyphens.
pub(crate) fn valid_login(s: &str) -> bool {
    (1..=39).contains(&s.len()) && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// A GitHub repository name: 1–100 of letters, digits, `.`, `_`, `-`.
pub(crate) fn valid_repo(s: &str) -> bool {
    (1..=100).contains(&s.len())
        && s != "."
        && s != ".."
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn list(v: &Option<String>) -> Vec<String> {
    v.as_deref()
        .map(|s| {
            s.split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .take(50)
                .collect()
        })
        .unwrap_or_default()
}

fn flag(v: &Option<String>, default: bool) -> bool {
    parse_bool(v.as_deref(), default)
}

impl CardQuery {
    fn login(&self) -> Option<String> {
        self.username
            .as_deref()
            .or(self.user.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }

    fn style(&self) -> CardStyle {
        let colors = ColorOverrides {
            title: self.title_color.clone(),
            icon: self.icon_color.clone(),
            text: self.text_color.clone(),
            bg: self.bg_color.clone().or_else(|| self.background.clone()),
            border: self.border_color.clone().or_else(|| self.border.clone()),
        };
        let mut s = CardStyle::new(self.theme.as_deref(), &colors);
        s.hide_border = flag(&self.hide_border, false);
        if let Some(r) = self
            .border_radius
            .as_deref()
            .and_then(|r| r.parse::<f32>().ok())
            .filter(|r| r.is_finite())
        {
            s.radius = r;
        }
        s.hide_title = flag(&self.hide_title, false);
        s.custom_title = self
            .custom_title
            .clone()
            .map(|t| cap_text(&decode_text(t), 80));
        s.card_width = self.card_width.as_deref().and_then(|w| w.parse().ok());
        s
    }

    fn stats_options(&self) -> StatsOptions {
        StatsOptions {
            hide: list(&self.hide),
            show_icons: flag(&self.show_icons, true),
            hide_rank: flag(&self.hide_rank, false),
        }
    }

    fn langs_options(&self) -> LangsOptions {
        LangsOptions {
            layout: Layout::parse(self.layout.as_deref()),
            langs_count: self.langs_count.as_deref().and_then(|n| n.parse().ok()),
            hide_progress: flag(&self.hide_progress, false),
        }
    }

    fn repo_options(&self) -> RepoOptions {
        RepoOptions {
            show_owner: flag(&self.show_owner, false),
            description_lines: self
                .description_lines_count
                .as_deref()
                .and_then(|n| n.parse().ok()),
        }
    }

    fn trophy_options(&self) -> TrophyOptions {
        let num = |v: &Option<String>| v.as_deref().and_then(|n| n.trim().parse::<i64>().ok());
        let d = TrophyOptions::default();
        TrophyOptions {
            column: num(&self.column)
                .map(|c| c.clamp(-1, 30) as i32)
                .unwrap_or(d.column),
            row: num(&self.row)
                .map(|r| r.clamp(1, 10) as u32)
                .unwrap_or(d.row),
            margin_w: num(&self.margin_w)
                .map(|m| m.clamp(0, 100) as u32)
                .unwrap_or(0),
            margin_h: num(&self.margin_h)
                .map(|m| m.clamp(0, 100) as u32)
                .unwrap_or(0),
            no_bg: flag(&self.no_bg, false),
            no_frame: flag(&self.no_frame, false),
            titles: list(&self.title),
            ranks: list(&self.rank),
        }
    }

    fn streak_colors(&self) -> StreakColors {
        StreakColors {
            ring: self.ring.clone(),
            fire: self.fire.clone(),
            curr_streak_num: self.curr_streak_num.clone(),
            side_nums: self.side_nums.clone(),
            curr_streak_label: self.curr_streak_label.clone(),
            side_labels: self.side_labels.clone(),
            dates: self.dates.clone(),
            stroke: self.stroke.clone(),
        }
    }
}

/// Rendered card plus its cache policy.
type Card = (String, CachePolicy);

const UNAVAILABLE: [&str; 2] = [
    "GitHub did not answer in time.",
    "This card refreshes on its own in a few minutes.",
];

/// A short-lived notice card in place of data.
fn fallback(style: &CardStyle, size: (u32, u32), title: &str, lines: &[&str]) -> Card {
    (
        notice(style, size.0, size.1, title, lines),
        CachePolicy::Fallback,
    )
}

/// Why a card has no data: the subject does not exist, or the URL lacks it
/// (`Ask` carries the query hint to show).
enum Why<'a> {
    Missing(&'a str),
    Ask(&'a str),
}

fn explain(style: &CardStyle, size: (u32, u32), title: &str, why: Why) -> Card {
    let (first, second) = match why {
        Why::Missing(what) => (
            format!("No GitHub {what} found."),
            "Check the spelling in the image URL.",
        ),
        Why::Ask(hint) => (format!("Add ?{hint} to the URL."), ""),
    };
    let lines: Vec<&str> = [first.as_str(), second]
        .into_iter()
        .filter(|l| !l.is_empty())
        .collect();
    fallback(style, size, title, &lines)
}

fn outcome<T>(
    lookup: Lookup<T>,
    style: &CardStyle,
    size: (u32, u32),
    title: &str,
    what: &str,
    ok: impl FnOnce(T) -> String,
) -> Card {
    match lookup {
        Lookup::Found(v) => (ok(v), CachePolicy::Live),
        Lookup::Missing => explain(style, size, title, Why::Missing(what)),
        Lookup::Unavailable => fallback(style, size, title, &UNAVAILABLE),
    }
}

/// The stats card plus its `Server-Timing` value (cache state, upstream and
/// render milliseconds), so production latency is visible from the response.
async fn stats_timed(st: &AppState, q: &CardQuery) -> (Card, Option<String>) {
    let (style, o) = (q.style(), q.stats_options());
    let size = stats_card::fallback_size(&style, &o);
    let Some(login) = q.login().filter(|l| valid_login(l)) else {
        let card = match q.login() {
            None => explain(
                &style,
                size,
                "GitHub Stats",
                Why::Ask("username=your-github-name"),
            ),
            Some(_) => explain(&style, size, "GitHub Stats", Why::Missing("user")),
        };
        return (card, None);
    };
    let title = style.title_or(stats_card::default_title(&login));
    let cached = st.live.stats_cached(&login).await;
    let started = Instant::now();
    let data = st.live.stats(&login, &list(&q.exclude_repo)).await;
    let upstream = started.elapsed();
    // A card missing a part (a slow upstream hid it) must be replaced soon,
    // not kept for hours: the part is filling the cache meanwhile.
    let partial = matches!(&data, Lookup::Found(d)
        if d.stars.is_none() || d.commits.is_none() || d.prs.is_none() || d.issues.is_none());
    let started = Instant::now();
    let (svg, policy) = outcome(data, &style, size, &title, "user", |d| {
        stats_card::render(&d, &style, &o)
    });
    let timing = format!(
        "cache;desc=\"{}\", upstream;dur={:.1}, render;dur={:.2}",
        if cached { "hit" } else { "miss" },
        upstream.as_secs_f64() * 1000.0,
        started.elapsed().as_secs_f64() * 1000.0,
    );
    let policy = if partial {
        CachePolicy::Fallback
    } else {
        policy
    };
    metrics::stats_served(policy == CachePolicy::Fallback);
    ((svg, policy), Some(timing))
}

/// Attach `Server-Timing` to a response.
fn with_timing(mut res: Response, timing: Option<String>) -> Response {
    if let Some(v) = timing.and_then(|t| axum::http::HeaderValue::from_str(&t).ok()) {
        res.headers_mut()
            .insert(axum::http::HeaderName::from_static("server-timing"), v);
    }
    res
}

async fn langs(st: &AppState, q: &CardQuery) -> Card {
    let (style, o) = (q.style(), q.langs_options());
    let size = langs_card::fallback_size(&style, &o);
    let title = style.title_or(langs_card::TITLE.into());
    let Some(login) = q.login().filter(|l| valid_login(l)) else {
        return match q.login() {
            None => explain(&style, size, &title, Why::Ask("username=your-github-name")),
            Some(_) => explain(&style, size, &title, Why::Missing("user")),
        };
    };
    let data = st
        .live
        .top_langs(&login, &list(&q.exclude_repo), &list(&q.hide))
        .await;
    outcome(data, &style, size, &title, "user", |d| {
        langs_card::render(&d, &style, &o)
    })
}

async fn streak(st: &AppState, q: &CardQuery) -> Card {
    let style = q.style();
    let size = (style.width(streak_card::WIDTH, 300), streak_card::HEIGHT);
    let title = "GitHub Streak";
    let Some(login) = q.login().filter(|l| valid_login(l)) else {
        return match q.login() {
            None => explain(&style, size, title, Why::Ask("user=your-github-name")),
            Some(_) => explain(&style, size, title, Why::Missing("user")),
        };
    };
    let colors = q.streak_colors();
    let themed = q.theme.is_some();
    let data = st.live.streak(&login).await;
    outcome(data, &style, size, title, "user", |s| {
        streak_card::render(&s, &style, &colors, themed)
    })
}

async fn trophy(st: &AppState, q: &CardQuery) -> Card {
    let (style, o) = (q.style(), q.trophy_options());
    let size = trophy_card::fallback_size(&o);
    let title = "GitHub Trophies";
    let Some(login) = q.login().filter(|l| valid_login(l)) else {
        return match q.login() {
            None => explain(&style, size, title, Why::Ask("username=your-github-name")),
            Some(_) => explain(&style, size, title, Why::Missing("user")),
        };
    };
    let now = st.live.now_unix();
    let data = st.live.stats(&login, &[]).await;
    outcome(data, &style, size, title, "user", |s| {
        let all = trophy_card::trophies(&s, now);
        let label = format!("{} GitHub trophies", s.login);
        trophy_card::render(&all, &style.palette, &o, &label)
    })
}

async fn stars(st: &AppState, q: &CardQuery) -> Card {
    let style = q.style();
    let size = (star_chart::WIDTH, star_chart::HEIGHT);
    let title = "Star history";
    let wanted = q
        .repos
        .as_deref()
        .or(q.repo.as_deref())
        .and_then(|r| r.split(',').next())
        .map(str::trim)
        .and_then(|r| r.split_once('/'))
        .map(|(o, n)| (o.to_string(), n.to_string()));
    let Some((owner, name)) = wanted else {
        return explain(&style, size, title, Why::Ask("repos=owner/name"));
    };
    if !valid_login(&owner) || !valid_repo(&name) {
        return explain(&style, size, title, Why::Missing("repository"));
    }
    let data = st.live.star_history(&owner, &name).await;
    outcome(data, &style, size, title, "repository", |h| {
        star_chart::render(&h, &style)
    })
}

async fn repo(st: &AppState, q: &CardQuery) -> Card {
    let (style, o) = (q.style(), q.repo_options());
    let size = repo_card::fallback_size(&style, &o);
    // `repo=owner/name` also works without `username`.
    let (owner, name) = match (q.login(), q.repo.as_deref().map(str::trim)) {
        (_, Some(r)) if r.contains('/') => {
            let (o, n) = r.split_once('/').unwrap_or((r, ""));
            (Some(o.to_string()), n.to_string())
        }
        (owner, Some(r)) => (owner, r.to_string()),
        (owner, None) => (owner, String::new()),
    };
    let title = "GitHub repository";
    let (Some(owner), false) = (owner, name.is_empty()) else {
        return explain(&style, size, title, Why::Ask("username=owner&repo=name"));
    };
    if !valid_login(&owner) || !valid_repo(&name) {
        return explain(&style, size, title, Why::Missing("repository"));
    }
    let data = st.live.repo(&owner, &name).await;
    outcome(data, &style, size, title, "repository", |r| {
        repo_card::render(&r, &style, &o)
    })
}

fn respond((svg, policy): Card, headers: &HeaderMap) -> Response {
    svg_response_cached(&svg, if_none_match(headers), policy)
}

/// `/api?username=…` (github-readme-stats stats card).
pub(crate) async fn stats_card(st: &AppState, q: &CardQuery, headers: &HeaderMap) -> Response {
    let (card, timing) = stats_timed(st, q).await;
    with_timing(respond(card, headers), timing)
}

/// `/api/top-langs?username=…`.
pub(crate) async fn top_langs_handler(
    State(st): State<AppState>,
    Query(q): Query<CardQuery>,
    headers: HeaderMap,
) -> Response {
    respond(langs(&st, &q).await, &headers)
}

/// `/api/pin?username=…&repo=…`.
pub(crate) async fn pin_handler(
    State(st): State<AppState>,
    Query(q): Query<CardQuery>,
    headers: HeaderMap,
) -> Response {
    respond(repo(&st, &q).await, &headers)
}

/// `/?user=…` (github-readme-streak-stats' root path).
pub(crate) async fn trophy_card(st: &AppState, q: &CardQuery, headers: &HeaderMap) -> Response {
    respond(trophy(st, q).await, headers)
}

/// `/trophy?username=…`.
pub(crate) async fn trophy_handler(
    State(st): State<AppState>,
    Query(q): Query<CardQuery>,
    headers: HeaderMap,
) -> Response {
    respond(trophy(&st, &q).await, &headers)
}

/// `/?user=…` (github-readme-streak-stats' root path).
pub(crate) async fn streak_card(st: &AppState, q: &CardQuery, headers: &HeaderMap) -> Response {
    respond(streak(st, q).await, headers)
}

/// `/streak?user=…`.
pub(crate) async fn streak_handler(
    State(st): State<AppState>,
    Query(q): Query<CardQuery>,
    headers: HeaderMap,
) -> Response {
    respond(streak(&st, &q).await, &headers)
}

/// `/svg?repos=owner/name` (api.star-history.com's image path).
pub(crate) async fn star_history_handler(
    State(st): State<AppState>,
    Query(q): Query<CardQuery>,
    headers: HeaderMap,
) -> Response {
    respond(stars(&st, &q).await, &headers)
}

/// Native surface: `/api/v1/card/{stats|langs|streak|repo}` (the router's `.svg`
/// suffix layer also serves `…/stats.svg`).
pub(crate) async fn card_handler(
    State(st): State<AppState>,
    Path(kind): Path<String>,
    Query(q): Query<CardQuery>,
    headers: HeaderMap,
) -> Response {
    let mut timing = None;
    let card = match kind.as_str() {
        "stats" => {
            let (card, t) = stats_timed(&st, &q).await;
            timing = t;
            card
        }
        "langs" | "top-langs" => langs(&st, &q).await,
        "streak" => streak(&st, &q).await,
        "repo" | "pin" => repo(&st, &q).await,
        "trophy" | "trophies" => trophy(&st, &q).await,
        "stars" | "star-history" => stars(&st, &q).await,
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    with_timing(respond(card, &headers), timing)
}
