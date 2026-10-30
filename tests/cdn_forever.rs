//! CDN-forever contract: every SVG URL pins its bytes and caches immutable long.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use mark::{app, AppState};
use tower::ServiceExt;

fn state() -> AppState {
    AppState::for_tests()
}

async fn get(path: &str, inm: Option<&str>) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    get_with(path, inm, None).await
}

async fn get_with(
    path: &str,
    inm: Option<&str>,
    origin: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let app = app(state());
    let mut req = Request::builder().uri(path);
    if let Some(v) = inm {
        req = req.header("if-none-match", v);
    }
    if let Some(o) = origin {
        req = req.header("origin", o);
    }
    let res = app.oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let body = res.into_body().collect().await.unwrap().to_bytes().to_vec();
    (status, headers, body)
}

fn hdr(h: &axum::http::HeaderMap, k: &str) -> String {
    h.get(k)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

fn vary_lists_origin(h: &axum::http::HeaderMap) -> bool {
    h.get_all("vary")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|s| s.split(','))
        .any(|t| t.trim().eq_ignore_ascii_case("origin"))
}

/// All 6 SVG endpoints (default surface + 5 forms + badge) share one cache contract.
#[tokio::test]
async fn every_svg_endpoint_is_immutable_long_with_etag_and_safety() {
    let paths = [
        "/api/v1/mark?text=Hi",
        "/api/v1/mark/hero?text=Hi",
        "/api/v1/mark/pill?label=build&message=passing",
        "/api/v1/mark/strip?icons=rust,ts",
        "/api/v1/mark/profile?text=Ada%20Lovelace",
        "/api/v1/mark/deploy?service=mark",
        "/badge/build-passing-brightgreen",
        // animated default (ambient) must also be immutable: bytes are pinned
        "/api/v1/mark/hero?type=soft&text=Hi&animation=glow",
        "/api/v1/mark/hero?type=soft&text=Hi&animation=none",
    ];
    for path in paths {
        let (status, h, body) = get(path, None).await;
        assert_eq!(status, StatusCode::OK, "path {path}");
        let cc = hdr(&h, "cache-control");
        assert!(
            cc.contains("max-age=31536000"),
            "immutable max-age: {path} cc={cc}"
        );
        assert!(
            cc.contains("s-maxage=31536000"),
            "edge s-maxage: {path} cc={cc}"
        );
        assert!(
            cc.contains("immutable"),
            "immutable directive: {path} cc={cc}"
        );
        assert!(
            !cc.contains("max-age=60"),
            "short TTL must be gone: {path} cc={cc}"
        );
        let cdn = hdr(&h, "cdn-cache-control");
        assert!(
            cdn.contains("s-maxage=31536000"),
            "cdn-cache-control: {path} cdn={cdn}"
        );
        let cf = hdr(&h, "cloudflare-cdn-cache-control");
        assert!(
            cf.contains("s-maxage=31536000"),
            "cf edge header: {path} cf={cf}"
        );
        let etag = hdr(&h, "etag");
        assert!(etag.len() >= 18, "strong ETag present: {path} etag={etag}");
        assert!(
            etag.starts_with('"') && etag.ends_with('"'),
            "quoted ETag: {etag}"
        );
        // safety headers intact
        let csp = hdr(&h, "content-security-policy");
        assert!(csp.contains("script-src 'none'"), "CSP: {path} csp={csp}");
        assert_eq!(
            hdr(&h, "x-content-type-options"),
            "nosniff",
            "nosniff: {path}"
        );
        assert_eq!(
            hdr(&h, "cross-origin-resource-policy"),
            "cross-origin",
            "CORP: {path}"
        );
        assert_eq!(
            hdr(&h, "access-control-allow-origin"),
            "*",
            "ACAO *: {path}"
        );
        assert!(
            !vary_lists_origin(&h),
            "SVG GET must not Vary: origin: {path} vary={:?}",
            hdr(&h, "vary")
        );
        assert!(!body.is_empty(), "body: {path}");
        // conditional GET -> 304 with same ETag, empty body (HIT-equivalent)
        let (s2, h2, b2) = get(path, Some(&etag)).await;
        assert_eq!(s2, StatusCode::NOT_MODIFIED, "304 for {path}");
        assert_eq!(hdr(&h2, "etag"), etag, "304 keeps ETag: {path}");
        assert!(b2.is_empty(), "304 empty body: {path}");
        assert!(
            hdr(&h2, "cache-control").contains("immutable"),
            "304 keeps cache: {path}"
        );
    }
}

#[tokio::test]
async fn same_url_same_bytes_across_fetches_and_etag_stable() {
    let path = "/api/v1/mark/hero?type=soft&text=Hi&animation=glow";
    let (s1, h1, b1) = get(path, None).await;
    let (s2, h2, b2) = get(path, None).await;
    assert_eq!(s1, StatusCode::OK);
    assert_eq!(s2, StatusCode::OK);
    assert_eq!(b1, b2, "double-fetch bytes identical");
    assert_eq!(hdr(&h1, "etag"), hdr(&h2, "etag"), "ETag stable");
}

#[tokio::test]
async fn svg_get_is_origin_independent_for_cdn() {
    let path = "/api/v1/mark/hero?type=soft&text=Hi";
    let (s1, h1, b1) = get_with(path, None, None).await;
    let (s2, h2, b2) = get_with(path, None, Some("https://example.com")).await;
    assert_eq!(s1, StatusCode::OK);
    assert_eq!(s2, StatusCode::OK);
    assert_eq!(b1, b2, "Origin header must not change SVG bytes");
    assert_eq!(
        hdr(&h1, "etag"),
        hdr(&h2, "etag"),
        "ETag independent of Origin"
    );
    assert_eq!(hdr(&h1, "access-control-allow-origin"), "*");
    assert_eq!(hdr(&h2, "access-control-allow-origin"), "*");
    assert!(!vary_lists_origin(&h1), "no Origin: {:?}", hdr(&h1, "vary"));
    assert!(
        !vary_lists_origin(&h2),
        "with Origin: {:?}",
        hdr(&h2, "vary")
    );
}

/// An opaque entity tag may contain commas (RFC 9110 section 8.8.3).
/// Splitting inside its quotes can mistake a different cached representation
/// for this image and suppress the body with a false 304.
#[tokio::test]
async fn conditional_images_compare_whole_quoted_validators() {
    for path in [
        "/badge/build-passing-brightgreen",
        "/api/v1/mark/hero?text=Hi",
        "/icons?i=rust,ts,react,nextjs,tailwind,docker,kubernetes,postgres,redis,go,python,bun",
        "/typing?lines=Hello;World",
        "/api?username=ada-dev",
        "/api?username=no-such-user",
        "/github/stars/SylphxAI/mark.svg",
    ] {
        let (status, headers, body) = get(path, None).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        let tag = hdr(&headers, "etag");
        let digest = tag.trim_matches('"');

        for validator in [
            format!("\"previous,{digest}\""),
            format!("W/\"previous,{digest}\""),
            format!("\"{digest},next\""),
            format!("\"previous,{digest},next\""),
            digest.to_string(),
            format!("W/{digest}"),
            format!("\"\"{digest}\"\""),
            format!("{tag} trailing"),
            format!("{tag}, malformed"),
        ] {
            let (s, h, b) = get(path, Some(&validator)).await;
            assert_eq!(s, StatusCode::OK, "{path}: {validator}");
            assert_eq!(b, body, "mismatch must send the complete SVG: {path}");
            assert_eq!(hdr(&h, "etag"), tag, "validator identity: {path}");
            assert_eq!(hdr(&h, "cache-control"), hdr(&headers, "cache-control"));
        }

        for validator in [
            tag.clone(),
            format!("W/{tag}"),
            "*".into(),
            format!("\"previous,other\", W/{tag}, \"next\""),
            format!(" ,\tW/{tag} , ,"),
        ] {
            let (s, h, b) = get(path, Some(&validator)).await;
            assert_eq!(s, StatusCode::NOT_MODIFIED, "{path}: {validator}");
            assert!(b.is_empty(), "matching validator saves the body: {path}");
            assert_eq!(hdr(&h, "etag"), tag);
            assert_eq!(hdr(&h, "cache-control"), hdr(&headers, "cache-control"));
        }
    }
}
