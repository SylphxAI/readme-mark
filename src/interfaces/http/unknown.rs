//! The answer for a path no route claims.
//!
//! A path that reads as a badge (a shields family we do not serve yet, a
//! `.svg` URL, or an image `Accept`) is asked for by an `<img>` on someone
//! else's page, so it gets a valid `unsupported` SVG badge, never HTML (a
//! broken image). Browser navigation (`text/html` accepted, JSON not, the same
//! rule `/api` negotiates with) and everything that is not image-like keep
//! the Mark-styled HTML 404.

use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, Uri};
use axum::response::{IntoResponse, Response};

use super::dispatch::prefers_html;
use super::pages;
use crate::bootstrap::AppState;
use crate::capabilities::live::domain::badges;
use crate::capabilities::live::interfaces::respond_face;
use crate::capabilities::mark::interfaces::MarkQuery;
use crate::interfaces::http::response::CachePolicy;

/// First path segments of shields badge families (plus ours that can miss on
/// shape): a request under one of these is a badge whatever the headers say.
const BADGE_FAMILIES: &[&str] = &[
    "appveyor",
    "badge",
    "bitbucket",
    "bundlephobia",
    "chrome-web-store",
    "circleci",
    "clojars",
    "cocoapods",
    "codacy",
    "codeclimate",
    "codecov",
    "codefactor",
    "conda",
    "coveralls",
    "cpan",
    "cran",
    "crates",
    "ctan",
    "discord",
    "docker",
    "docsrs",
    "dub",
    "dynamic",
    "endpoint",
    "flathub",
    "gem",
    "github",
    "gitlab",
    "go",
    "hackage",
    "hexpm",
    "homebrew",
    "jenkins",
    "jsdelivr",
    "liberapay",
    "librariesio",
    "maven-central",
    "mastodon",
    "matrix",
    "npm",
    "nuget",
    "opencollective",
    "packagist",
    "pub",
    "pypi",
    "readthedocs",
    "reddit",
    "snapcraft",
    "snyk",
    "sonar",
    "static",
    "travis",
    "twitter",
    "uptimerobot",
    "vscode-marketplace",
    "wakatime",
    "website",
    "youtube",
];

fn first_segment(path: &str) -> &str {
    let seg = path.trim_start_matches('/').split('/').next().unwrap_or("");
    seg.strip_suffix(".svg").unwrap_or(seg)
}

fn is_badge_family(path: &str) -> bool {
    let seg = first_segment(path).to_ascii_lowercase();
    BADGE_FAMILIES.contains(&seg.as_str())
}

/// The file extension of the last path segment, if it has one.
fn extension(path: &str) -> Option<&str> {
    let last = path.rsplit('/').next().unwrap_or("");
    last.rsplit_once('.').map(|(_, e)| e)
}

fn accepts_image(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|a| {
            a.split(',')
                .any(|p| p.trim().to_ascii_lowercase().starts_with("image/"))
        })
}

/// Whether an unrouted `path` is an image consumer's request.
pub(super) fn wants_badge(path: &str, headers: &HeaderMap) -> bool {
    let accept = headers.get(header::ACCEPT).and_then(|v| v.to_str().ok());
    if prefers_html(accept) {
        return false;
    }
    is_badge_family(path)
        || extension(path).is_some_and(|e| e.eq_ignore_ascii_case("svg"))
        || (extension(path).is_none() && accepts_image(headers))
}

/// A short, safe label from the first path segment (the renderer escapes it
/// anyway; this keeps the pill readable).
fn label(path: &str) -> String {
    let seg: String = first_segment(path)
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .take(24)
        .collect();
    if is_badge_family(path) && !seg.is_empty() {
        seg.to_string()
    } else {
        "badge".to_string()
    }
}

pub(super) async fn not_found(
    State(st): State<AppState>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    if !wants_badge(uri.path(), &headers) {
        return pages::not_found(State(st)).await.into_response();
    }
    // A malformed styling query must not turn a badge into a 400.
    let q = Query::<MarkQuery>::try_from_uri(&uri)
        .or_else(|_| Query::<MarkQuery>::try_from_uri(&Uri::from_static("/")))
        .map(|q| q.0);
    match q {
        Ok(q) => {
            let face = badges::unsupported(&label(uri.path()));
            respond_face(face, CachePolicy::Fallback, &st, &q, &headers)
        }
        Err(_) => pages::not_found(State(st)).await.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn accept(v: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::ACCEPT, HeaderValue::from_str(v).unwrap());
        h
    }

    #[test]
    fn badge_like_paths_get_an_image_and_navigation_gets_html() {
        let none = HeaderMap::new();
        let img = accept("image/avif,image/webp,image/svg+xml,image/*,*/*;q=0.8");
        let nav = accept("text/html,application/xhtml+xml,*/*;q=0.8");
        assert!(wants_badge("/codecov/c/github/a/b", &none));
        assert!(wants_badge("/whatever/thing.svg", &none));
        assert!(wants_badge("/some/unknown/path", &img));
        assert!(!wants_badge("/favicon.ico", &img), "files stay 404");
        assert!(!wants_badge("/some/unknown/path", &none));
        assert!(!wants_badge("/codecov/c/github/a/b", &nav), "navigation");
        assert!(!wants_badge("/nope", &nav));
    }

    #[test]
    fn labels_are_short_and_safe() {
        assert_eq!(label("/codecov/c/github/a/b"), "codecov");
        assert_eq!(label("/zzz/<x>/y.svg"), "badge");
        assert_eq!(label("/pypi.svg"), "pypi");
    }
}
