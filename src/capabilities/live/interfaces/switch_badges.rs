//! Shields paths that complete the host swap: PyPI, crates.io, Docker Hub,
//! and the `endpoint` JSON badge. Faces render through the one pill renderer,
//! so the shields query restyles them like any other badge.

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use serde::Deserialize;

use super::badges::{from_lookup, respond};
use crate::bootstrap::AppState;
use crate::capabilities::live::application::cache::Lookup;
use crate::capabilities::live::application::switch_registries::{
    valid_crate, valid_docker, valid_pypi,
};
use crate::capabilities::live::domain::badges::{self, Face};
use crate::capabilities::live::domain::endpoint::{check_url, EndpointBadge};
use crate::capabilities::mark::domain::MarkForm;
use crate::capabilities::mark::interfaces::MarkQuery;
use crate::capabilities::mark::render;
use crate::interfaces::http::response::{if_none_match, svg_response_cached, CachePolicy};

fn missing(label: &str, what: &str) -> (Face, CachePolicy) {
    (badges::not_found(label, what), CachePolicy::Fallback)
}

fn unsupported(label: &str) -> (Face, CachePolicy) {
    (badges::unsupported(label), CachePolicy::Fallback)
}

async fn pypi_face(st: &AppState, kind: &str, name: &str) -> (Face, CachePolicy) {
    let label = if kind == "v" { "pypi" } else { "downloads" };
    if !matches!(kind, "v" | "dd" | "dw" | "dm") {
        return unsupported(&format!("pypi {kind}"));
    }
    if !valid_pypi(name) {
        return missing(label, "package");
    }
    if kind == "v" {
        return from_lookup(st.live.pypi_version(name).await, label, "package", |v| {
            badges::version("pypi", &v)
        });
    }
    from_lookup(
        st.live.pypi_downloads(name).await,
        label,
        "package",
        |d| match kind {
            "dd" => badges::downloads(d.day, Some("day")),
            "dw" => badges::downloads(d.week, Some("week")),
            _ => badges::downloads(d.month, Some("month")),
        },
    )
}

async fn crates_face(st: &AppState, kind: &str, name: &str) -> (Face, CachePolicy) {
    let label = if kind == "v" {
        "crates.io"
    } else {
        "downloads"
    };
    if !matches!(kind, "v" | "d" | "dr") {
        return unsupported(&format!("crates {kind}"));
    }
    if !valid_crate(name) {
        return missing(label, "crate");
    }
    from_lookup(
        st.live.crate_info(name).await,
        label,
        "crate",
        |c| match kind {
            "v" => badges::version("crates.io", &c.version),
            "d" => badges::downloads(c.downloads, None),
            _ => badges::downloads(c.recent, Some("90 days")),
        },
    )
}

async fn docker_face(st: &AppState, kind: &str, user: &str, image: &str) -> (Face, CachePolicy) {
    let label = match kind {
        "pulls" => "docker pulls",
        "stars" => "docker stars",
        _ => "docker",
    };
    if !matches!(kind, "pulls" | "stars" | "v") {
        return unsupported(&format!("docker {kind}"));
    }
    if !valid_docker(user) || !valid_docker(image) {
        return missing(label, "image");
    }
    if kind == "v" {
        let found = st.live.docker_version(user, image).await;
        return from_lookup(found, label, "image", |v| badges::version("docker", &v));
    }
    from_lookup(
        st.live.docker_repo(user, image).await,
        label,
        "image",
        |r| Face {
            label: label.into(),
            ..if kind == "pulls" {
                badges::downloads(r.pulls, None)
            } else {
                badges::likes(r.stars)
            }
        },
    )
}

/// `/pypi/{v|dd|dw|dm}/{package}`.
// duplicate-exception: thin route adapter (extractors, one face reader, respond); each route owns its path shape.
pub(crate) async fn pypi_badge(
    State(st): State<AppState>,
    Path((kind, name)): Path<(String, String)>,
    Query(q): Query<MarkQuery>,
    headers: HeaderMap,
) -> Response {
    let (face, policy) = pypi_face(&st, &kind, &name).await;
    respond(face, policy, &st, &q, &headers)
}

/// `/crates/{v|d|dr}/{crate}`: version, total downloads, recent (90 day)
/// downloads.
// duplicate-exception: thin route adapter (extractors, one face reader, respond); each route owns its path shape.
pub(crate) async fn crates_badge(
    State(st): State<AppState>,
    Path((kind, name)): Path<(String, String)>,
    Query(q): Query<MarkQuery>,
    headers: HeaderMap,
) -> Response {
    let (face, policy) = crates_face(&st, &kind, &name).await;
    respond(face, policy, &st, &q, &headers)
}

/// `/docker/{pulls|stars|v}/{user}/{image}` (`library` for official images).
pub(crate) async fn docker_badge(
    headers: HeaderMap,
    State(st): State<AppState>,
    Query(q): Query<MarkQuery>,
    Path((kind, user, image)): Path<(String, String, String)>,
) -> Response {
    let (face, policy) = docker_face(&st, &kind, &user, &image).await;
    respond(face, policy, &st, &q, &headers)
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct EndpointQuery {
    pub url: Option<String>,
}

/// `/endpoint?url=<https JSON>`: shields' endpoint badge (schema version 1).
/// The URL is vetted before anything is fetched; every failure is a calm
/// badge, never an error page.
pub(crate) async fn endpoint_badge(
    State(st): State<AppState>,
    Query(q): Query<MarkQuery>,
    Query(e): Query<EndpointQuery>,
    headers: HeaderMap,
) -> Response {
    let url = match e.url.as_deref().map(check_url) {
        None => return respond_plain("url required", &st, &q, &headers),
        Some(Err(why)) => return respond_plain(why.message(), &st, &q, &headers),
        Some(Ok(u)) => u,
    };
    let (badge, policy): (EndpointBadge, CachePolicy) = match st.live.endpoint(url.as_str()).await {
        Lookup::Found(b) => (b, CachePolicy::Short),
        Lookup::Missing => return respond_plain("invalid response", &st, &q, &headers),
        Lookup::Unavailable => return respond_plain("unavailable", &st, &q, &headers),
    };
    let mut spec = q.to_spec(MarkForm::Pill, st.default_credit);
    spec.pill.label = Some(spec.pill.label.take().unwrap_or(badge.label));
    spec.pill.message = Some(badge.message);
    if spec.pill.label_color.is_none() {
        spec.pill.label_color = badge.label_color;
    }
    spec.color = if badge.is_error {
        Some("red".into())
    } else {
        spec.color.or(badge.color).or(Some("lightgrey".into()))
    };
    svg_response_cached(&render(&spec), if_none_match(&headers), policy)
}

fn respond_plain(message: &str, st: &AppState, q: &MarkQuery, headers: &HeaderMap) -> Response {
    let face = Face {
        message: message.into(),
        ..badges::unsupported("endpoint")
    };
    respond(face, CachePolicy::Fallback, st, q, headers)
}
