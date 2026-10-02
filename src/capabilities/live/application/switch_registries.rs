//! Readers behind the shields paths that complete the host swap: PyPI,
//! crates.io, Docker Hub, and the `endpoint` JSON badge. Same shape as
//! [`super::registries`]: one bounded GET each, no token.

use serde_json::Value;

use super::registries::{counters, json, num, token, version_at};
use super::upstream::{Call, Resource, Upstream, UpstreamError};
use crate::capabilities::live::domain::endpoint::{self, EndpointBadge};
use crate::capabilities::live::domain::model::{CrateInfo, DockerRepo, PypiDownloads};

// duplicate-exception: identifier shape checks are one token() call each; the charsets differ.
/// A PyPI project name (PEP 508): letters, digits, `.`, `_`, `-`.
pub(crate) fn valid_pypi(name: &str) -> bool {
    token(name, 214, |c| {
        c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')
    }) && !name.starts_with(['.', '-'])
}

/// A crates.io crate name.
pub(crate) fn valid_crate(name: &str) -> bool {
    token(name, 64, |c| {
        c.is_ascii_alphanumeric() || matches!(c, '_' | '-')
    })
}

// duplicate-exception: identifier shape checks are one token() call each; the charsets differ.
/// One Docker Hub namespace or repository name.
pub(crate) fn valid_docker(part: &str) -> bool {
    token(part, 128, |c| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | '-')
    }) && !part.starts_with(['.', '-'])
}

/// The latest PyPI version.
pub(crate) async fn pypi_version(
    up: &dyn Upstream,
    name: &str,
) -> Result<Option<String>, UpstreamError> {
    let url = format!("https://pypi.org/pypi/{name}/json");
    version_at(up, url, "/info/version").await
}

// duplicate-exception: one counters() call mapped onto a typed record, like Packagist's.
pub(crate) async fn pypi_downloads(
    up: &dyn Upstream,
    name: &str,
) -> Result<Option<PypiDownloads>, UpstreamError> {
    let url = format!("https://pypistats.org/api/packages/{name}/recent");
    let got = counters(up, url, "data", ["last_day", "last_week", "last_month"]).await?;
    Ok(got.map(|[day, week, month]| PypiDownloads { day, week, month }))
}

pub(crate) async fn crate_info(
    up: &dyn Upstream,
    name: &str,
) -> Result<Option<CrateInfo>, UpstreamError> {
    let v = json(up, format!("https://crates.io/api/v1/crates/{name}")).await?;
    Ok(v.and_then(|v| {
        let c = v.get("crate")?;
        let version = c
            .get("max_stable_version")
            .and_then(Value::as_str)
            .or_else(|| c.get("max_version").and_then(Value::as_str))?;
        Some(CrateInfo {
            version: version.to_string(),
            downloads: num(c, "downloads"),
            recent: num(c, "recent_downloads"),
        })
    }))
}

pub(crate) async fn docker_repo(
    up: &dyn Upstream,
    user: &str,
    image: &str,
) -> Result<Option<DockerRepo>, UpstreamError> {
    let v = json(
        up,
        format!("https://hub.docker.com/v2/repositories/{user}/{image}"),
    )
    .await?;
    Ok(v.map(|v| DockerRepo {
        pulls: num(&v, "pull_count"),
        stars: num(&v, "star_count"),
    }))
}

/// Numeric components of a version-looking tag (`v1.2.3` → `[1, 2, 3]`).
fn tag_key(tag: &str) -> Option<Vec<u64>> {
    let bare = tag.strip_prefix('v').unwrap_or(tag);
    if !bare.chars().next()?.is_ascii_digit() {
        return None;
    }
    // `1.2.3-alpine` counts as `1.2.3`; a suffix keeps it from a stable win.
    let core = bare.split(['-', '+']).next()?;
    core.split('.').map(|p| p.parse().ok()).collect()
}

/// The highest version-looking tag among the most recently pushed ones, else
/// the newest tag that is not `latest`, else `latest`.
pub(crate) fn pick_tag(names: &[&str]) -> Option<String> {
    let best = names
        .iter()
        .filter(|n| !n.contains(['-', '+']))
        .filter_map(|n| tag_key(n).map(|k| (k, *n)))
        .max_by(|a, b| a.0.cmp(&b.0));
    best.map(|(_, n)| n.to_string())
        .or_else(|| {
            names
                .iter()
                .find(|n| **n != "latest")
                .map(|n| n.to_string())
        })
        .or_else(|| names.first().map(|n| n.to_string()))
}

pub(crate) async fn docker_version(
    up: &dyn Upstream,
    user: &str,
    image: &str,
) -> Result<Option<String>, UpstreamError> {
    let url = format!(
        "https://hub.docker.com/v2/repositories/{user}/{image}/tags?page_size=50&ordering=last_updated"
    );
    let v = json(up, url).await?;
    Ok(v.and_then(|v| {
        let names: Vec<&str> = v
            .get("results")?
            .as_array()?
            .iter()
            .filter_map(|t| t.get("name").and_then(Value::as_str))
            .collect();
        pick_tag(&names)
    }))
}

/// The endpoint document at an already vetted `https` URL. A reply that is
/// not a schema-v1 document reads as `None`.
pub(crate) async fn endpoint(
    up: &dyn Upstream,
    url: String,
) -> Result<Option<EndpointBadge>, UpstreamError> {
    let body = super::upstream::read_body(up, Call::read(Resource::Endpoint, url)).await?;
    Ok(body.as_deref().and_then(endpoint::parse))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_validated() {
        assert!(valid_pypi("requests") && valid_pypi("zope.interface") && valid_pypi("a_b-c"));
        assert!(!valid_pypi("../x") && !valid_pypi("") && !valid_pypi("-x") && !valid_pypi("a/b"));
        assert!(valid_crate("serde_json") && valid_crate("tokio-util") && !valid_crate("a.b"));
        assert!(valid_docker("library") && valid_docker("nginx") && !valid_docker("Nginx"));
        assert!(!valid_docker("a/b") && !valid_docker(".x"));
    }

    #[test]
    fn docker_picks_the_highest_stable_version_tag() {
        assert_eq!(
            pick_tag(&["latest", "1.9.0", "1.27.2", "1.27.2-alpine", "1.10.0"]).as_deref(),
            Some("1.27.2")
        );
        assert_eq!(pick_tag(&["latest", "stable"]).as_deref(), Some("stable"));
        assert_eq!(pick_tag(&["latest"]).as_deref(), Some("latest"));
        assert_eq!(pick_tag(&[]), None);
        assert_eq!(pick_tag(&["v2.0.0", "v10.1.0"]).as_deref(), Some("v10.1.0"));
    }
}
