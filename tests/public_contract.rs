//! Public-contract oracles for the one grammar (ADR-0003/0004).
//!
//! The owning writer is the HTTP surface a stranger can call. Catalog JSON
//! is legal source-text of the published vocabulary. These tests do not
//! read product source.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use mark::capabilities::mark::domain::{MarkForm, MarkSpec};
use mark::capabilities::mark::render;
use mark::{app, AppState};
use tower::ServiceExt;

fn state() -> AppState {
    AppState::for_tests()
}

async fn get(path: &str) -> (StatusCode, String, String) {
    get_with(path, &[]).await
}

async fn get_with(path: &str, headers: &[(&str, &str)]) -> (StatusCode, String, String) {
    let app = app(state());
    let mut req = Request::builder().uri(path);
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    let res = app.oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
    let status = res.status();
    let ctype = res
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, ctype, String::from_utf8_lossy(&body).into_owned())
}

async fn catalog() -> serde_json::Value {
    let (status, _, body) = get("/api/v1/catalog").await;
    assert_eq!(status, StatusCode::OK);
    serde_json::from_str(&body).expect("catalog JSON")
}

fn strings(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .expect("string array")
        .iter()
        .map(|x| x.as_str().expect("string").to_string())
        .collect()
}

#[tokio::test]
async fn catalog_publishes_the_one_vocabulary() {
    let v = catalog().await;
    let obj = v.as_object().expect("catalog object");
    for key in [
        "forms",
        "art_types",
        "theme_palettes",
        "layouts",
        "themes",
        "icons",
        "badge_styles",
        "animations",
        "fonts",
        "limits",
        "notes",
    ] {
        assert!(obj.contains_key(key), "missing catalog key {key}");
    }
    assert_eq!(
        strings(&v["forms"]),
        ["hero", "pill", "strip", "profile", "deploy", "typing", "score"]
    );
    let limits = &v["limits"];
    assert_eq!(limits["text"], 500);
    assert_eq!(limits["desc"], 240);
    assert_eq!(limits["lines"], 8);
    assert_eq!(limits["pill_label"], 80);
    assert_eq!(limits["pill_message"], 120);
    assert_eq!(limits["strip_icons"], 60);
    assert_eq!(limits["deploy_service"], 40);
    assert_eq!(
        v["notes"]["grammar"].as_str().expect("notes.grammar"),
        "mark = form × art (type) × paint (theme/color, pill labelColor) × content (text/desc/font) × geometry (width/height, hero layout) × motion (animation)"
    );
    assert_eq!(
        v["notes"]["themes"].as_str().expect("notes.themes"),
        "neutral design themes — no personal or company names"
    );
}

#[tokio::test]
async fn catalog_publishes_only_curated_themes() {
    // This is the documented public vocabulary, not a list derived from the
    // renderer. Adding a theme requires an explicit change to the contract.
    const THEMES: [&str; 8] = [
        "dark", "light", "ocean", "sunset", "forest", "grape", "mono", "paper",
    ];
    let v = catalog().await;
    assert_eq!(strings(&v["themes"]), THEMES);

    let mut expected = THEMES;
    expected.sort_unstable();
    let mut palette_ids: Vec<_> = v["theme_palettes"]
        .as_object()
        .expect("theme palette object")
        .keys()
        .map(String::as_str)
        .collect();
    palette_ids.sort_unstable();
    assert_eq!(palette_ids, expected);
}

#[tokio::test]
async fn catalog_forms_serve_svg_to_a_stranger() {
    let v = catalog().await;
    for form in strings(&v["forms"]) {
        let path = match form.as_str() {
            "hero" => "/api/v1/mark/hero?type=soft&text=Hi&animation=none",
            "pill" => "/api/v1/mark/pill?label=build&message=passing",
            "strip" => "/api/v1/mark/strip?icons=rust,ts",
            "profile" => "/api/v1/mark/profile?text=Ada%20Lovelace",
            "deploy" => "/api/v1/mark/deploy?service=mark",
            "typing" => "/api/v1/mark/typing?lines=Hello;World",
            "score" => "/api/v1/mark/score?label=agent-ready&value=92",
            other => panic!("unexpected published form {other}"),
        };
        let (status, ctype, body) = get(path).await;
        assert_eq!(status, StatusCode::OK, "form {form}");
        assert!(ctype.contains("svg"), "form {form} ctype={ctype}");
        assert!(body.contains("<svg"), "form {form} must render");
    }
}

#[tokio::test]
async fn catalog_art_types_render() {
    let v = catalog().await;
    for ty in strings(&v["art_types"]) {
        let path = format!("/api/v1/mark/hero?type={ty}&text=T&animation=none");
        let (status, _, body) = get(&path).await;
        assert_eq!(status, StatusCode::OK, "art {ty}");
        assert!(
            body.starts_with("<?xml") && body.contains("</svg>"),
            "art {ty}"
        );
    }
}

#[tokio::test]
async fn catalog_icons_render_glyphs() {
    let v = catalog().await;
    let icons = strings(&v["icons"]);
    let skill_icons = strings(&v["skill_icons"]);
    for id in &icons {
        assert!(
            skill_icons.contains(id) || matches!(id.as_str(), "csharp" | "helm" | "node"),
            "featured icon outside the public technology vocabulary: {id}"
        );
    }
    assert!(!icons.is_empty(), "catalog must advertise icons");
    let path = format!(
        "/api/v1/mark/strip?icons={}&animation=none",
        icons.join(",")
    );
    let (status, _, body) = get(&path).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !body.contains(">?</text>"),
        "advertised icons must not fall back to the unknown tile"
    );
    for id in &icons {
        assert!(
            body.contains(&format!("<title>{id}</title>")),
            "catalog icon {id} missing from the strip"
        );
    }
}

#[tokio::test]
async fn unknown_icon_is_the_unknown_tile() {
    let (status, _, body) = get("/api/v1/mark/strip?icons=not-an-icon,sylphx").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body.matches(">?</text>").count(),
        2,
        "unpublished ids must use the unknown tile"
    );
}

#[tokio::test]
async fn unknown_theme_names_use_default_paint() {
    let path = "/api/v1/mark/hero?text=Hi&animation=none";
    let (status, _, plain) = get(path).await;
    assert_eq!(status, StatusCode::OK);
    for name in ["custom-company", "personal", "sylphx"] {
        let (status, _, themed) = get(&format!("{path}&theme={name}")).await;
        assert_eq!(status, StatusCode::OK, "theme {name}");
        assert_eq!(themed, plain, "unknown theme {name} must use default paint");
    }
    let (status, _, grape) = get(&format!("{path}&theme=grape")).await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(grape, plain, "curated theme must change paint");
    let (status, _, neon) = get(&format!("{path}&theme=neon")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(neon, grape, "legacy theme alias must still resolve");
}

#[tokio::test]
async fn staff_headers_do_not_change_the_mark() {
    let path = "/api/v1/mark/hero?type=soft&text=probe&animation=none";
    let (_, _, stranger) = get(path).await;
    let (_, _, privileged) = get_with(
        path,
        &[
            ("authorization", "Bearer staff"),
            ("x-staff", "1"),
            ("x-app-secret", "internal"),
            ("x-sylphx-internal", "1"),
            ("x-first-party", "true"),
        ],
    )
    .await;
    assert_eq!(
        privileged, stranger,
        "staff/first-party headers must not skip the public mark"
    );

    let inject =
        "/api/v1/mark/hero?type=soft&text=probe&animation=none&color=%22%20onload=%22alert(7)";
    let (status, _, body) = get_with(
        inject,
        &[("authorization", "Bearer staff"), ("x-staff", "1")],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !body.contains("onload="),
        "staff header must not skip paint grammar"
    );
}

#[tokio::test]
async fn public_http_matches_the_render_writer() {
    let hero = MarkSpec {
        form: MarkForm::Hero,
        art: Some("aurora".into()),
        text: Some("Ship your release".into()),
        animation: Some("none".into()),
        ..Default::default()
    };
    let (_, _, http_hero) =
        get("/api/v1/mark/hero?type=aurora&text=Ship%20your%20release&animation=none").await;
    assert_eq!(http_hero, render(&hero), "HTTP hero must equal render()");

    let pill = MarkSpec {
        form: MarkForm::Pill,
        pill: mark::capabilities::mark::domain::PillSpec {
            label: Some("build".into()),
            message: Some("passing".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    let (_, _, http_pill) = get("/api/v1/mark/pill?label=build&message=passing").await;
    assert_eq!(http_pill, render(&pill), "HTTP pill must equal render()");
}

#[tokio::test]
async fn unknown_form_still_renders_svg() {
    let (status, ctype, body) = get("/api/v1/mark/not-a-form?text=Hi&animation=none").await;
    assert_eq!(status, StatusCode::OK);
    assert!(ctype.contains("svg"));
    assert!(body.contains("<svg"));
    assert!(body.contains("Hi"));
}

#[tokio::test]
async fn catalog_text_limit_is_enforced_over_http() {
    let v = catalog().await;
    let cap = v["limits"]["text"].as_u64().expect("text limit") as usize;
    let overflow = "x".repeat(cap + 80);
    let path = format!("/api/v1/mark/hero?text={overflow}&animation=none");
    let (status, _, body) = get(&path).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains('…'), "truncation must be marked");
    assert!(
        !body.contains(&overflow),
        "catalog text cap must bind the public path"
    );
}
