use axum::body::Body;
use axum::http::{header::CONTENT_TYPE, Request, StatusCode};
use http_body_util::BodyExt;
use mark::{app, AppState};
use tower::ServiceExt;

#[tokio::test]
async fn llms_discovery_is_public_plain_text_not_an_image_or_html_fallback() {
    let response = app(AppState::for_tests())
        .oneshot(
            Request::builder()
                .uri("/llms.txt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let text = std::str::from_utf8(&bytes).unwrap();
    assert_eq!(text, include_str!("../static/llms.txt"));
    assert!(text.starts_with("# Mark\n\n> "));
    for link in [
        "https://mark.sylphx.com/docs",
        "https://mark.sylphx.com/api/v1/catalog",
    ] {
        assert!(text.contains(link));
    }
    assert!(!text.contains("<html"));
    assert!(!text.contains("<svg"));
}
