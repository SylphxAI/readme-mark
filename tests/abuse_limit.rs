//! Per-client budget on upstream loads (issue 102): a heavy client is held to
//! cached or fallback cards, and a cached `.svg` is never limited.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use mark::{app, AppState};
use tower::ServiceExt;

async fn get(router: &Router, path: &str, client: &str) -> (StatusCode, HeaderMap, String) {
    let req = Request::builder()
        .uri(path)
        .header("cf-connecting-ip", client)
        .body(Body::empty())
        .unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    let (status, headers) = (res.status(), res.headers().clone());
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, headers, String::from_utf8_lossy(&body).into_owned())
}

fn cache(h: &HeaderMap) -> &str {
    h.get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
}

const THROTTLED: &str = "GitHub did not answer in time.";
const CACHED: &str = "max-age=1800";

/// Cold users (unknown to the fixtures) until `client` is refused; true once seen.
async fn exhaust(router: &Router, client: &str) -> bool {
    for i in 0..40 {
        let (status, _, body) = get(
            router,
            &format!("/api?username=cold-{}-{i}", client.replace('.', "-")),
            client,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "a refused load is still a 200 card");
        if body.contains(THROTTLED) {
            return true;
        }
    }
    false
}

#[tokio::test]
async fn a_heavy_client_is_throttled_to_a_short_cached_fallback_card() {
    let router = app(AppState::for_tests_with_budget(6));
    assert!(exhaust(&router, "203.0.113.7").await, "the budget runs out");
    let (status, h, body) = get(&router, "/api?username=cold-new", "203.0.113.7").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains(THROTTLED));
    assert!(cache(&h).contains("max-age=300"), "{}", cache(&h));
    let (_, _, other) = get(&router, "/api?username=cold-new", "203.0.113.8").await;
    assert!(
        !other.contains(THROTTLED),
        "another client has its own budget, and the refusal cached nothing"
    );
}

#[tokio::test]
async fn cached_svg_is_never_limited_even_over_budget() {
    let router = app(AppState::for_tests_with_budget(12));
    let svg = "/api/v1/card/stats.svg?username=ada-dev";
    let (_, h, body) = get(&router, svg, "203.0.113.7").await;
    assert!(
        cache(&h).contains(CACHED) && !body.contains(THROTTLED),
        "warm"
    );
    assert!(exhaust(&router, "203.0.113.7").await, "now over budget");
    for _ in 0..50 {
        let (status, h, body) = get(&router, svg, "203.0.113.7").await;
        assert_eq!(status, StatusCode::OK);
        assert!(cache(&h).contains(CACHED), "{}", cache(&h));
        assert!(!body.contains(THROTTLED));
    }
    for path in ["/badge/a-b-c.svg", "/api/v1/mark/hero.svg"] {
        let (status, h, _) = get(&router, path, "203.0.113.7").await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(cache(&h).contains("immutable"), "{path}: {}", cache(&h));
    }
}
