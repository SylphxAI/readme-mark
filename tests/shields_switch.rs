//! The shields host swap, completed: PyPI, crates.io, Docker Hub and the
//! `endpoint` badge answer on shields' paths, and any other badge-like path
//! answers a valid SVG instead of an HTML 404 (a broken image in a README).
//! Upstreams are the offline fixtures; `cargo test` never touches the network.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use mark::{app, AppState};
use tower::ServiceExt;

async fn get_accept(path: &str, accept: Option<&str>) -> (StatusCode, HeaderMap, String) {
    let mut req = Request::builder().uri(path);
    if let Some(a) = accept {
        req = req.header("accept", a);
    }
    let res = app(AppState::for_tests())
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, headers) = (res.status(), res.headers().clone());
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, headers, String::from_utf8_lossy(&body).into_owned())
}

async fn get(path: &str) -> (StatusCode, HeaderMap, String) {
    get_accept(path, None).await
}

fn hdr<'a>(h: &'a HeaderMap, k: &str) -> &'a str {
    h.get(k).and_then(|v| v.to_str().ok()).unwrap_or("")
}

fn assert_svg(path: &str, status: StatusCode, h: &HeaderMap, body: &str) {
    assert_eq!(status, StatusCode::OK, "{path}");
    assert!(
        hdr(h, "content-type").starts_with("image/svg+xml"),
        "{path}: {}",
        hdr(h, "content-type")
    );
    assert!(body.contains("<svg"), "{path}");
    assert!(
        hdr(h, "content-security-policy").contains("script-src 'none'"),
        "{path}"
    );
}

/// `(path, text the badge must carry)`.
const SOURCES: &[(&str, &[&str])] = &[
    ("/pypi/v/mark-demo", &["pypi", "v3.2.1"]),
    ("/pypi/dm/mark-demo", &["downloads", "2.3M/month"]),
    ("/pypi/dw/mark-demo", &["99k/week"]),
    ("/pypi/dd/mark-demo.svg", &["1.2k/day"]),
    ("/crates/v/mark-demo", &["crates.io", "v1.4.2"]),
    ("/crates/d/mark-demo", &["downloads", "12M"]),
    ("/crates/dr/mark-demo.svg", &["235k/90 days"]),
    ("/docker/pulls/library/nginx", &["docker pulls", "9.9G"]),
    ("/docker/pulls/sylphx/mark.svg", &["docker pulls", "4.2M"]),
    ("/docker/stars/sylphx/mark", &["docker stars", "88"]),
    ("/docker/v/sylphx/mark", &["docker", "v1.10.0"]),
    (
        "/endpoint?url=https%3A%2F%2Fstatus.example.test%2Fmark.json",
        &["build", "passing"],
    ),
];

#[tokio::test]
async fn each_new_source_renders_its_upstream_fact_with_live_caching() {
    for (path, needles) in SOURCES {
        let (status, h, body) = get(path).await;
        assert_svg(path, status, &h, &body);
        for n in *needles {
            assert!(body.contains(n), "{path} lacks {n}: {body}");
        }
        let cc = hdr(&h, "cache-control");
        if path.starts_with("/endpoint") {
            assert!(cc.contains("s-maxage=300"), "{path}: {cc}");
        } else {
            assert!(cc.contains("s-maxage=14400"), "{path}: {cc}");
            assert!(cc.contains("stale-if-error=604800"), "{path}: {cc}");
        }
        assert!(hdr(&h, "etag").starts_with('"'), "{path}");
    }
}

#[tokio::test]
async fn the_shields_query_restyles_the_new_badges() {
    let (_, _, plain) = get("/crates/v/mark-demo").await;
    let (_, _, styled) = get("/crates/v/mark-demo?style=for-the-badge&label=RUST").await;
    assert_ne!(plain, styled);
    assert!(styled.contains("RUST"));
}

#[tokio::test]
async fn unknown_subjects_and_kinds_stay_calm_badges() {
    for (path, needle, cache) in [
        ("/pypi/v/ghost-404", "package not found", "s-maxage=300"),
        ("/pypi/v/offline", "unavailable", "s-maxage=300"),
        ("/pypi/l/mark-demo", "unsupported", "s-maxage=300"),
        ("/crates/v/ghost-404", "crate not found", "s-maxage=300"),
        ("/crates/l/mark-demo", "unsupported", "s-maxage=300"),
        (
            "/docker/pulls/sylphx/Bad_Name",
            "image not found",
            "s-maxage=300",
        ),
        ("/docker/build/sylphx/mark", "unsupported", "s-maxage=300"),
    ] {
        let (status, h, body) = get(path).await;
        assert_svg(path, status, &h, &body);
        assert!(body.contains(needle), "{path}: {body}");
        assert!(hdr(&h, "cache-control").contains(cache), "{path}");
    }
}

#[tokio::test]
async fn endpoint_failures_are_badges_not_errors() {
    let enc = |u: &str| format!("/endpoint?url={}", urlencoding::encode(u));
    for (path, needle) in [
        ("/endpoint".to_string(), "url required"),
        (
            enc("https://status.example.test/bad.json"),
            "invalid response",
        ),
        (
            enc("https://status.example.test/missing.json"),
            "invalid response",
        ),
        (enc("https://status.example.test/offline"), "unavailable"),
        (enc("http://status.example.test/mark.json"), "https only"),
        (enc("not a url"), "invalid url"),
    ] {
        let (status, h, body) = get(&path).await;
        assert_svg(&path, status, &h, &body);
        assert!(body.contains(needle), "{path}: {body}");
        assert!(hdr(&h, "cache-control").contains("s-maxage=300"), "{path}");
    }
    // isError wins over a caller's color.
    let (_, _, err) = get(&format!(
        "{}&color=blue",
        enc("https://status.example.test/error.json")
    ))
    .await;
    assert!(err.contains("deploy") && err.contains("down"));
    let (_, _, blue) = get(&format!(
        "{}&color=blue",
        enc("https://status.example.test/mark.json")
    ))
    .await;
    assert_ne!(err, blue);
}

/// The SSRF guard: nothing private, plain-http, credentialed or off-port ever
/// reaches the fetcher. The fixture upstream would answer any of these hosts
/// with a valid document if it were asked, so a rendered "passing" would
/// prove the guard leaked.
#[tokio::test]
async fn endpoint_refuses_private_and_unsafe_urls() {
    for raw in [
        "https://127.0.0.1/mark.json",
        "https://localhost/mark.json",
        "https://[::1]/mark.json",
        "https://[::ffff:10.0.0.1]/mark.json",
        "https://10.1.2.3/mark.json",
        "https://192.168.0.1/mark.json",
        "https://169.254.169.254/latest/meta-data/",
        "https://2130706433/mark.json",
        "https://metadata.internal/mark.json",
        "https://status.example.test:8443/mark.json",
        "https://user:pw@status.example.test/mark.json",
        "http://status.example.test/mark.json",
        "file:///etc/passwd",
        "ftp://status.example.test/mark.json",
    ] {
        let path = format!("/endpoint?url={}", urlencoding::encode(raw));
        let (status, h, body) = get(&path).await;
        assert_svg(&path, status, &h, &body);
        assert!(!body.contains("passing"), "{raw} leaked: {body}");
        assert!(
            [
                "host not allowed",
                "https only",
                "port not allowed",
                "invalid url"
            ]
            .iter()
            .any(|m| body.contains(m)),
            "{raw}: {body}"
        );
    }
}

#[tokio::test]
async fn unknown_badge_paths_answer_an_svg_never_html() {
    for path in [
        "/codecov/c/github/SylphxAI/mark",
        "/codecov/c/github/SylphxAI/mark.svg",
        "/maven-central/v/org.apache.commons/commons-lang3",
        "/vscode-marketplace/v/ms-python.python.svg",
        "/github/issues/SylphxAI",
        "/pypi",
        "/some/new/thing.svg",
    ] {
        let (status, h, body) = get(path).await;
        assert_svg(path, status, &h, &body);
        assert!(body.contains("unsupported"), "{path}: {body}");
        assert!(!body.contains("<html"), "{path}");
        let cc = hdr(&h, "cache-control");
        assert!(
            cc.contains("max-age=300") && !cc.contains("immutable"),
            "{path}: {cc}"
        );
    }
    let (_, _, body) = get("/codecov/c/github/SylphxAI/mark").await;
    assert!(body.contains("codecov"));
    // An <img> sends an image Accept for any URL.
    let img = "image/avif,image/webp,image/svg+xml,image/*,*/*;q=0.8";
    let (status, h, body) = get_accept("/never/heard/of/it", Some(img)).await;
    assert_svg("/never/heard/of/it", status, &h, &body);
    // A malformed styling query does not become a 400.
    let (status, _, _) = get("/codecov/c/github/a/b?width=abc").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn browsers_and_plain_files_keep_the_html_404() {
    let nav = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";
    for (path, accept) in [
        ("/codecov/c/github/SylphxAI/mark", Some(nav)),
        ("/no-such-page", Some(nav)),
        ("/no-such-page", None),
        ("/favicon-missing.ico", Some("image/avif,image/*,*/*;q=0.8")),
        ("/api/v1/nope", None),
        ("/banner", None),
    ] {
        let (status, h, body) = get_accept(path, accept).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(hdr(&h, "content-type").starts_with("text/html"), "{path}");
        assert!(body.contains("This page does not exist."), "{path}");
    }
}
