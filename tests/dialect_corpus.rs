//! Host-only migration contracts from pinned upstream README examples.

use axum::body::Body;
use axum::http::{header::CONTENT_TYPE, Request, StatusCode, Uri};
use http_body_util::BodyExt;
use mark::{app, AppState};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use tower::ServiceExt;

#[derive(Deserialize)]
struct Case {
    dialect: String,
    url: String,
    source: String,
    geometry: Vec<String>,
    text: Vec<String>,
}

#[tokio::test]
async fn readme_urls_render_after_only_changing_the_host() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("corpus/readme-urls.json")).unwrap();
    assert_eq!(cases.len(), 50);
    let mut urls = BTreeSet::new();
    let mut dialects = BTreeMap::new();
    let router = app(AppState::for_tests());
    let mut missing = Vec::new();
    for case in cases {
        assert!(urls.insert(case.url.clone()), "duplicate: {}", case.url);
        *dialects.entry(case.dialect.clone()).or_insert(0) += 1;
        assert!(!case.geometry.is_empty() && !case.text.is_empty());
        // Fragments are browser metadata, not part of an HTTP request.
        let uri: Uri = case.url.split('#').next().unwrap().parse().unwrap();
        let path = uri.path_and_query().unwrap().as_str();
        let context = format!("{} ({}, {})", case.url, case.dialect, case.source);
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("host", "mark.sylphx.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{context}");
        assert!(
            response.headers()[CONTENT_TYPE]
                .to_str()
                .unwrap()
                .starts_with("image/svg+xml"),
            "{context}"
        );
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let svg = std::str::from_utf8(&bytes).unwrap();
        for expected in case.geometry.iter().chain(&case.text) {
            if !svg.contains(expected) {
                missing.push(format!("{context}: missing {expected}"));
            }
        }
        assert!(!svg.contains("temporarily unavailable"), "{context}");
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
    assert_eq!(
        dialects,
        BTreeMap::from([
            ("capsule-render".into(), 16),
            ("github-readme-stats".into(), 10),
            ("readme-typing-svg".into(), 2),
            ("shields".into(), 18),
            ("skill-icons".into(), 4),
        ])
    );
}
