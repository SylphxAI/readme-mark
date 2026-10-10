//! Dynamic badges on shields' paths: `/github/{stars,forks,license,…}` and
//! `/npm/{v,dm,dw,dt,l}/…`. Faces render through the one pill renderer, so
//! the shields query (`style`, `label`, `color`, `labelColor`, …) restyles
//! them exactly as it restyles a static `/badge/…`.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use super::cards::{valid_login, valid_repo};
use crate::bootstrap::AppState;
use crate::capabilities::live::application::cache::Lookup;
use crate::capabilities::live::application::npm::valid_package;
use crate::capabilities::live::domain::badges::{self, Face};
use crate::capabilities::mark::domain::MarkForm;
use crate::capabilities::mark::interfaces::MarkQuery;
use crate::capabilities::mark::render;
use crate::interfaces::http::response::{if_none_match, svg_response_cached, CachePolicy};

#[derive(Debug, Default, Deserialize)]
pub(crate) struct BadgeExtra {
    pub include_prereleases: Option<String>,
}

pub(crate) fn respond(
    face: Face,
    policy: CachePolicy,
    st: &AppState,
    q: &MarkQuery,
    headers: &HeaderMap,
) -> Response {
    let mut spec = q.to_spec(MarkForm::Pill, st.default_credit);
    spec.pill.label = Some(spec.pill.label.take().unwrap_or(face.label));
    spec.pill.message = Some(face.message);
    spec.color = spec.color.or(Some(face.color));
    svg_response_cached(&render(&spec), if_none_match(headers), policy)
}

pub(super) fn from_lookup<T>(
    lookup: Lookup<T>,
    label: &str,
    what: &str,
    ok: impl FnOnce(T) -> Face,
) -> (Face, CachePolicy) {
    match lookup {
        Lookup::Found(v) => (ok(v), CachePolicy::Live),
        Lookup::Missing => (badges::not_found(label, what), CachePolicy::Fallback),
        Lookup::Unavailable => {
            // A live upstream read failed: the card falls back, and the
            // failure is one `tool_failed` occurrence for that upstream.
            crate::signals::issue(crate::signals::Kind::ToolFailed, what, "unavailable");
            (badges::unavailable(label), CachePolicy::Fallback)
        }
    }
}

async fn github_face(
    st: &AppState,
    kind: &str,
    owner: &str,
    repo: &str,
    branch: Option<&str>,
    pre: bool,
) -> (Face, CachePolicy) {
    let label = match kind {
        "stars" => "stars",
        "forks" => "forks",
        "license" => "license",
        "release" => "release",
        _ => "last commit",
    };
    if !valid_login(owner) || !valid_repo(repo) {
        return (badges::not_found(label, "repo"), CachePolicy::Fallback);
    }
    let live = &st.live;
    match kind {
        "stars" => from_lookup(live.repo(owner, repo).await, label, "repo", |r| {
            badges::stars(r.stars)
        }),
        "forks" => from_lookup(live.repo(owner, repo).await, label, "repo", |r| {
            badges::forks(r.forks)
        }),
        "license" => from_lookup(live.repo(owner, repo).await, label, "repo", |r| {
            badges::license(r.license.as_deref())
        }),
        "release" => from_lookup(
            live.release(owner, repo, pre).await,
            label,
            "repo",
            |r| match r {
                Some((tag, pre)) => badges::release(Some(&tag), pre),
                None => badges::release(None, false),
            },
        ),
        _ => {
            let now = live.now_unix();
            from_lookup(
                live.last_commit(owner, repo, branch).await,
                label,
                "repo",
                |c| badges::last_commit(c.as_deref(), now),
            )
        }
    }
}

fn prereleases(extra: &BadgeExtra) -> bool {
    extra
        .include_prereleases
        .as_deref()
        .is_some_and(|v| v != "false" && v != "0")
}

/// `/github/{stars|forks|license|last-commit|release}/{owner}/{repo}`.
pub(crate) async fn github_badge(
    State(st): State<AppState>,
    Path((kind, owner, repo)): Path<(String, String, String)>,
    Query(q): Query<MarkQuery>,
    Query(extra): Query<BadgeExtra>,
    headers: HeaderMap,
) -> Response {
    if !matches!(
        kind.as_str(),
        "stars" | "forks" | "license" | "last-commit" | "release"
    ) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let (face, policy) = github_face(&st, &kind, &owner, &repo, None, prereleases(&extra)).await;
    respond(face, policy, &st, &q, &headers)
}

/// `/github/v/release/{owner}/{repo}`.
// duplicate-exception: release_badge/last_commit_branch are two shields path shapes (extra query vs extra path segment) over the one github_face reader.
pub(crate) async fn release_badge(
    State(st): State<AppState>,
    Path((owner, repo)): Path<(String, String)>,
    Query(q): Query<MarkQuery>,
    Query(extra): Query<BadgeExtra>,
    headers: HeaderMap,
) -> Response {
    let (face, policy) =
        github_face(&st, "release", &owner, &repo, None, prereleases(&extra)).await;
    respond(face, policy, &st, &q, &headers)
}

/// `/github/last-commit/{owner}/{repo}/{branch}`.
pub(crate) async fn last_commit_branch(
    State(st): State<AppState>,
    Path((owner, repo, branch)): Path<(String, String, String)>,
    Query(q): Query<MarkQuery>,
    headers: HeaderMap,
) -> Response {
    let (face, policy) = github_face(&st, "last-commit", &owner, &repo, Some(&branch), false).await;
    respond(face, policy, &st, &q, &headers)
}

/// Split `name`, `name/tag`, `@scope/name`, `@scope/name/tag`.
fn split_package(tail: &str) -> (String, Option<String>) {
    let tail = tail.trim_matches('/');
    let parts: Vec<&str> = tail.split('/').collect();
    let (name_parts, tag) = if tail.starts_with('@') {
        (parts.len().min(2), parts.get(2))
    } else {
        (1, parts.get(1))
    };
    (parts[..name_parts].join("/"), tag.map(|t| t.to_string()))
}

/// `/npm/{v|dm|dw|dt|l}/{package}` (scoped packages and a dist-tag for `v`).
pub(crate) async fn npm_badge(
    State(st): State<AppState>,
    Path((kind, tail)): Path<(String, String)>,
    Query(q): Query<MarkQuery>,
    headers: HeaderMap,
) -> Response {
    let (name, tag) = split_package(&tail);
    let label = match kind.as_str() {
        "v" => "npm",
        "l" => "license",
        _ => "downloads",
    };
    let tag = tag.unwrap_or_else(|| "latest".into());
    let valid_tag = tag.len() <= 64
        && tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'));
    let (face, policy) = if !valid_package(&name) || !valid_tag {
        (badges::not_found(label, "package"), CachePolicy::Fallback)
    } else {
        let live = &st.live;
        match kind.as_str() {
            "v" => from_lookup(live.npm_package(&name, &tag).await, label, "package", |p| {
                badges::npm_version(&p.version)
            }),
            "l" => from_lookup(
                live.npm_package(&name, "latest").await,
                label,
                "package",
                |p| badges::license(p.license.as_deref()),
            ),
            period => {
                let (range, unit) = match period {
                    "dw" => ("last-week", Some("week")),
                    "dt" => ("1000-01-01:3000-01-01", None),
                    _ => ("last-month", Some("month")),
                };
                match live.npm_downloads(&name, range).await {
                    // npm's downloads API has no counts for new packages yet.
                    Lookup::Missing => match live.npm_package(&name, "latest").await {
                        Lookup::Found(_) => (badges::downloads_pending(), CachePolicy::Fallback),
                        other => from_lookup(other, label, "package", |_| unreachable_face()),
                    },
                    found => from_lookup(found, label, "package", |n| badges::downloads(n, unit)),
                }
            }
        }
    };
    respond(face, policy, &st, &q, &headers)
}

/// `from_lookup` over a non-`Found` lookup never calls its mapper.
fn unreachable_face() -> Face {
    badges::unavailable("downloads")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_paths_split_scope_and_tag() {
        assert_eq!(split_package("react"), ("react".into(), None));
        assert_eq!(
            split_package("react/next"),
            ("react".into(), Some("next".into()))
        );
        assert_eq!(split_package("@types/node"), ("@types/node".into(), None));
        assert_eq!(
            split_package("@types/node/beta"),
            ("@types/node".into(), Some("beta".into()))
        );
    }
}
