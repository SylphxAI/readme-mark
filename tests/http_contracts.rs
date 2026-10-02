//! HTTP composition contracts — the single mark surface.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use mark::{app, AppState};
use tower::ServiceExt;

fn state() -> AppState {
    AppState::for_tests()
}

fn state_with_credit() -> AppState {
    AppState {
        default_credit: true,
        ..AppState::for_tests()
    }
}

async fn get_with(st: AppState, path: &str) -> (StatusCode, String) {
    let app = app(st);
    let res = app
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&body).into_owned())
}

async fn get(path: &str) -> (StatusCode, String, String) {
    let app = app(state());
    let res = app
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
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

fn studio_boot(html: &str) -> serde_json::Value {
    const MARKER: &str = "window.__MARK_BOOT__ = ";
    let start = html.find(MARKER).expect("studio boot marker");
    let rest = html[start + MARKER.len()..].trim_start();
    let end = rest.find(';').expect("boot assignment terminator");
    serde_json::from_str(&rest[..end]).expect("studio boot JSON")
}

#[tokio::test]
async fn default_credit_applies_unless_query_overrides() {
    let (status, on) = get_with(
        state_with_credit(),
        "/api/v1/mark/hero?text=Hi&animation=none",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(on.contains(">mark</text>"), "DEFAULT_CREDIT must watermark");

    let (_, off) = get_with(
        state_with_credit(),
        "/api/v1/mark/hero?text=Hi&animation=none&credit=0",
    )
    .await;
    assert!(
        !off.contains(">mark</text>"),
        "credit=0 must win over default"
    );

    let (_, _, unset) = get("/api/v1/mark/hero?text=Hi&animation=none").await;
    assert!(
        !unset.contains(">mark</text>"),
        "default off stays unmarked"
    );
}

#[tokio::test]
async fn health_is_json_liveness_with_revision() {
    let (status, ctype, body) = get("/health").await;
    assert_eq!(status, StatusCode::OK);
    assert!(ctype.contains("json"), "ctype={ctype}");
    assert!(body.contains("\"ok\":true") || body.contains("\"ok\": true"));
    let v: serde_json::Value = serde_json::from_str(&body).expect("json");
    let rev = v.get("revision").and_then(|x| x.as_str()).unwrap_or("");
    assert!(!rev.is_empty(), "revision must be present: {body}");
}

#[tokio::test]
async fn studio_exposes_recovery_and_svg_export_controls() {
    let (status, ctype, body) = get("/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(ctype.contains("html"), "ctype={ctype}");
    for marker in [
        "Download SVG",
        "Retry",
        "Preparing SVG export",
        "Copy Markdown",
        "Copy URL",
    ] {
        assert!(body.contains(marker), "studio copy missing: {marker}");
    }
    assert_eq!(studio_boot(&body), serde_json::Value::Null);
    assert!(
        !body.contains("{{BOOT}}"),
        "boot placeholder must be substituted"
    );
}

#[tokio::test]
async fn studio_boots_composer_state_from_query() {
    let (status, _, body) = get("/?form=profile&text=Ada%20Lovelace").await;
    assert_eq!(status, StatusCode::OK);
    let boot = studio_boot(&body);
    assert_eq!(boot["form"], "profile");
    assert_eq!(boot["text"], "Ada Lovelace");
}

#[tokio::test]
async fn studio_identity_query_boots_profile() {
    let (_, _, body) = get("/?form=identity&text=Ada%20Lovelace").await;
    let boot = studio_boot(&body);
    assert_eq!(boot["form"], "profile");
    assert_eq!(boot["text"], "Ada Lovelace");
}

#[tokio::test]
async fn studio_boots_from_wrapped_public_mark_url() {
    let url = "/?url=https%3A%2F%2Fmark.sylphx.com%2Fapi%2Fv1%2Fmark%2Fhero%3Ftype%3Dwave%26text%3DMark%26height%3D120";
    let (_, _, body) = get(url).await;
    let boot = studio_boot(&body);
    assert_eq!(boot["form"], "hero");
    assert_eq!(boot["type"], "wave");
    assert_eq!(boot["text"], "Mark");
    assert_eq!(boot["height"], 120);
}

#[tokio::test]
async fn studio_boots_from_badge_shorthand_url() {
    let (_, _, body) = get("/?url=%2Fbadge%2Fbuild-passing-brightgreen").await;
    let boot = studio_boot(&body);
    assert_eq!(boot["form"], "pill");
    assert_eq!(boot["pill"]["label"], "build");
    assert_eq!(boot["pill"]["message"], "passing");
    assert_eq!(boot["color"], "brightgreen");
}

#[tokio::test]
async fn studio_boot_escapes_script_breakout() {
    let (_, _, body) = get("/?form=hero&text=%3C%2Fscript%3E").await;
    let boot = studio_boot(&body);
    assert_eq!(boot["text"], "</script>");
    let start = body.find("window.__MARK_BOOT__ = ").expect("boot");
    let line = body[start..].lines().next().expect("boot line");
    assert!(
        !line.contains("</script>"),
        "boot assignment must not break the script: {line}"
    );
    assert!(
        !line.contains("<script"),
        "boot assignment must not open a script: {line}"
    );
}

#[tokio::test]
async fn mark_surface_serves_every_form() {
    for (path, needle) in [
        ("/api/v1/mark?type=aurora&text=Hi&animation=none", "Hi"),
        ("/api/v1/mark/hero?type=soft&text=Hi&animation=none", "Hi"),
        ("/api/v1/mark/pill?label=build&message=passing", "passing"),
        ("/api/v1/mark/strip?icons=rust,ts", "rust"),
        ("/api/v1/mark/profile?text=Kyle%20Tse", "Kyle Tse"),
        ("/api/v1/mark/identity?text=Ada%20Lovelace", "Ada Lovelace"),
        ("/api/v1/mark/deploy?service=mark", "Sylphx"),
        ("/badge/build-passing-brightgreen", "passing"),
    ] {
        let (status, ctype, body) = get(path).await;
        assert_eq!(status, StatusCode::OK, "mark path must serve: {path}");
        assert!(ctype.contains("svg"), "ctype={ctype} for {path}");
        assert!(body.contains(needle), "needle {needle} missing in {path}");
    }
}

#[tokio::test]
async fn badge_shorthand_accepts_grammar_query() {
    let (status, _, styled) = get("/badge/build-passing-brightgreen?style=for-the-badge").await;
    assert_eq!(status, StatusCode::OK);
    assert!(styled.contains("height=\"28\""), "for-the-badge must apply");
    assert!(styled.contains("BUILD"), "for-the-badge paints uppercase");

    let (_, _, flat) = get("/badge/build-passing-brightgreen").await;
    assert!(flat.contains("height=\"20\""), "bare shorthand stays flat");
    assert!(flat.contains("passing"));

    let (_, _, themed) = get("/badge/build-passing-brightgreen?theme=github").await;
    assert!(
        themed.contains("fill=\"#5B6CFF\""),
        "theme query must paint"
    );
    assert!(
        !themed.contains("fill=\"#44bb00\""),
        "theme query must override path color"
    );

    let (_, _, faded) = get("/badge/build-passing-brightgreen?animation=fade").await;
    assert!(faded.contains("<animate"), "animation query must compose");

    let (_, _, labeled) = get("/badge/build-passing-brightgreen?labelColor=red").await;
    assert!(
        labeled.contains("fill=\"#dd4343\""),
        "labelColor query must paint the label"
    );

    let (font_status, _, fonted) = get("/badge/build-passing-brightgreen?font=mono").await;
    assert_eq!(font_status, StatusCode::OK);
    assert!(
        fonted.contains("passing"),
        "font query must stay a valid mark"
    );

    let (credit_status, _, credited) = get("/badge/build-passing-brightgreen?credit=1").await;
    assert_eq!(credit_status, StatusCode::OK);
    assert!(
        credited.contains("passing"),
        "credit query must stay a valid mark"
    );

    // shields semantics: query `color` and `label` override the path tokens.
    let (_, _, overridden) = get("/badge/build-passing-brightgreen?color=red&label=ci").await;
    assert!(
        overridden.contains("fill=\"#dd4343\"") && !overridden.contains("fill=\"#44bb00\""),
        "query color overrides the path color, as on shields"
    );
    assert!(
        overridden.contains(">ci<"),
        "query label overrides the path label"
    );
}

#[tokio::test]
async fn svg_suffix_serves_identical_bytes() {
    for path in [
        "/badge/build-passing-brightgreen",
        "/badge/agent--ready-92%2F100-brightgreen?style=for-the-badge",
        "/api/v1/mark/hero?text=Hi",
        "/api/v1/mark/score?label=agent-ready&value=92",
        "/api/v1/mark",
        "/static/v1?label=a&message=b&color=blue",
    ] {
        let suffixed = match path.split_once('?') {
            Some((p, q)) => format!("{p}.svg?{q}"),
            None => format!("{path}.svg"),
        };
        let (s1, c1, plain) = get(path).await;
        let (s2, c2, with_suffix) = get(&suffixed).await;
        assert_eq!(s1, StatusCode::OK, "{path}");
        assert_eq!(s2, StatusCode::OK, "{suffixed}");
        assert_eq!(c1, c2);
        assert_eq!(plain, with_suffix, "{suffixed} must equal {path}");
    }
}

#[tokio::test]
async fn shields_invalid_color_paints_brightgreen_and_static_v1_defaults_lightgrey() {
    let (_, _, invalid) = get("/badge/a-b-notacolor").await;
    assert!(invalid.contains("fill=\"#44bb00\""));
    let (_, _, missing) = get("/static/v1?label=a&message=b").await;
    assert!(missing.contains("fill=\"#939393\""));
}

#[tokio::test]
async fn legacy_surfaces_are_removed() {
    for path in [
        "/api/v1/banner",
        "/api/v1/badge",
        "/api/v1/icons",
        "/api/v1/brand/sylphx",
        "/api/v1/deploy",
        "/api/v1/stats/shtse8",
        "/api/v1/org/SylphxAI",
        "/api/v1/repo/SylphxAI/mark",
        "/banner",
        "/stats/shtse8",
        "/org/SylphxAI",
        "/repo/SylphxAI/mark",
        "/brand/sylphx",
        "/deploy",
        "/api/v1/nope",
    ] {
        let (status, _, _) = get(path).await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "legacy surface must 404: {path}"
        );
    }
}

#[tokio::test]
async fn svg_responses_have_csp_and_nosniff() {
    let app = app(state());
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/mark/pill?label=x&message=y")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let csp = res
        .headers()
        .get("content-security-policy")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        csp.contains("script-src 'none'"),
        "CSP must block scripts: {csp}"
    );
    assert_eq!(
        res.headers()
            .get("x-content-type-options")
            .and_then(|v| v.to_str().ok()),
        Some("nosniff")
    );
}

#[tokio::test]
async fn studio_binds_to_the_catalog() {
    let (status, _, body) = get("/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("/api/v1/catalog"),
        "studio must load the one grammar vocabulary"
    );
}

#[tokio::test]
async fn studio_page_has_no_webfont_origin() {
    let (status, _, body) = get("/").await;
    assert_eq!(status, StatusCode::OK);
    for host in ["fonts.googleapis.com", "fonts.gstatic.com"] {
        assert!(
            !body.contains(host),
            "studio must not load webfont origin {host}"
        );
    }
    assert!(
        body.contains("--font:ui-sans-serif,system-ui,"),
        "studio page must use a system sans stack"
    );
    assert!(
        body.contains("--mono:ui-monospace,"),
        "studio page must use a system mono stack"
    );
}

#[tokio::test]
async fn studio_boots_pill_label_color_when_no_theme_pack() {
    let (_, _, body) = get("/?form=pill&label=build&message=passing&labelColor=red").await;
    let boot = studio_boot(&body);
    assert_eq!(boot["form"], "pill");
    assert_eq!(boot["pill"]["label"], "build");
    assert_eq!(boot["pill"]["message"], "passing");
    assert_eq!(boot["pill"]["labelColor"], "red");
}

#[tokio::test]
async fn studio_theme_pack_drops_pill_label_color() {
    let (_, _, body) =
        get("/?form=pill&label=build&message=passing&theme=github&labelColor=red").await;
    let boot = studio_boot(&body);
    assert_eq!(boot["theme"], "github");
    assert!(
        boot["pill"]["labelColor"].is_null(),
        "theme pack is dest paint; labelColor must not recover"
    );
}

#[tokio::test]
async fn catalog_exposes_the_one_vocabulary() {
    let (status, _, body) = get("/api/v1/catalog").await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).expect("catalog JSON");
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
}

#[tokio::test]
async fn injection_is_inert_over_http() {
    for query in [
        "color=%22%20onload=%22alert(7)",
        "text=%22%3E%20onload%3D%22alert(7)",
        "theme=%22%20onload=%22alert(7)",
        "desc=%3C%2Ftext%3E%3Cscript%3Ealert(1)%3C%2Fscript%3E",
    ] {
        let path = format!("/api/v1/mark/hero?type=soft&animation=none&{query}");
        let (status, _, body) = get(&path).await;
        assert_eq!(status, StatusCode::OK, "{query}");
        for needle in ["onload=\"", "<script", "<img", "javascript:"] {
            assert!(
                !body.contains(needle),
                "injection must not create markup: {query} -> {needle}"
            );
        }
    }
}

#[tokio::test]
async fn retired_hero_knobs_are_inert() {
    let base = "/api/v1/mark/hero?type=soft&text=probe&animation=none";
    let (_, _, plain) = get(base).await;
    for knob in [
        "fontSize=99",
        "descSize=40",
        "fontColor=%23ff0000",
        "fontAlign=10",
        "fontAlignY=10",
        "descAlign=90",
        "descAlignY=20",
        "rotate=45",
        "stroke=%2300ff00",
        "strokeWidth=8",
        "textBg=1",
        "section=footer",
        "reversal=1",
    ] {
        let (status, _, body) = get(&format!("{base}&{knob}")).await;
        assert_eq!(status, StatusCode::OK, "{knob} must still render");
        assert_eq!(
            body, plain,
            "retired predecessor knob {knob} must not reach the render"
        );
    }
}

#[tokio::test]
async fn retired_form_ids_are_unknown_forms() {
    let query = "?text=probe&animation=none";
    let (_, _, hero) = get(&format!("/api/v1/mark/hero{query}")).await;
    for id in ["badge", "icons", "iconsrow", "card", "deploymark"] {
        let (status, _, body) = get(&format!("/api/v1/mark/{id}{query}")).await;
        assert_eq!(status, StatusCode::OK, "{id} must still render (totality)");
        assert_eq!(body, hero, "retired form id {id} is unknown input");
    }
    // `identity` is the graph's `rename-to` id (MARK-IDENTITY): it stays.
    let (_, _, identity) = get(&format!("/api/v1/mark/identity{query}")).await;
    let (_, _, profile) = get(&format!("/api/v1/mark/profile{query}")).await;
    assert_eq!(
        identity, profile,
        "identity URLs still reach the profile card"
    );
}

#[tokio::test]
async fn retired_layout_and_animation_ids_map_or_default() {
    let (_, _, plain) = get("/api/v1/mark/hero?text=probe").await;
    for unknown in [
        "bg",
        "idle",
        "off",
        "static",
        "spin",
        "waving",
        "typewriter",
        "glitch",
    ] {
        let (_, _, body) = get(&format!("/api/v1/mark/hero?text=probe&animation={unknown}")).await;
        assert_eq!(body, plain, "{unknown} must render the default animation");
    }
    let (_, _, rise) = get("/api/v1/mark/hero?text=probe&animation=rise").await;
    for entry in ["bounce", "slide", "scale", "cascade"] {
        let (_, _, body) = get(&format!("/api/v1/mark/hero?text=probe&animation={entry}")).await;
        assert_eq!(body, rise, "retired entry motion {entry} renders rise");
    }
    let (_, _, default_layout) =
        get("/api/v1/mark/hero?text=probe&layout=default&animation=none").await;
    for alias in [
        "card", "mono", "cli", "center", "product", "hero", "oss", "signal",
    ] {
        let (_, _, body) = get(&format!(
            "/api/v1/mark/hero?text=probe&layout={alias}&animation=none"
        ))
        .await;
        assert_eq!(body, default_layout, "layout {alias} renders the default");
    }
    let (_, _, left) = get("/api/v1/mark/hero?text=probe&layout=left&animation=none").await;
    for alias in ["plate", "terminal"] {
        let (_, _, body) = get(&format!(
            "/api/v1/mark/hero?text=probe&layout={alias}&animation=none"
        ))
        .await;
        assert_eq!(body, left, "retired layout {alias} renders left");
    }
}

#[tokio::test]
async fn identity_form_matches_profile_over_http() {
    let query = "?text=Ada%20Lovelace&desc=First%20programmer&theme=tokyonight";
    let (_, _, identity) = get(&format!("/api/v1/mark/identity{query}")).await;
    let (_, _, profile) = get(&format!("/api/v1/mark/profile{query}")).await;
    assert_eq!(
        identity, profile,
        "identity URLs must render the profile card"
    );
    assert!(identity.contains("Ada Lovelace"));
    assert!(identity.contains(">AL<"));
}

#[tokio::test]
async fn nonfinite_paint_input_cannot_reach_svg() {
    let base = "/api/v1/mark/hero?text=probe&animation=none";
    let (_, _, plain) = get(base).await;
    for invalid_spec in [
        "NaN%3AFF0000%2C100%3A000000",
        "inf%3AFF0000",
        "-1%3AFF0000%2C100%3A000000",
    ] {
        let (status, _, body) = get(&format!("{base}&color={invalid_spec}")).await;
        assert_eq!(status, StatusCode::OK, "{invalid_spec}");
        assert_eq!(
            body, plain,
            "invalid stops must fall back to exactly the default paint: {invalid_spec}"
        );
        for invalid in ["NaN", "inf", "-inf"] {
            assert!(
                !body.contains(invalid),
                "non-finite input escaped: {invalid}"
            );
        }
    }
}

#[tokio::test]
async fn determinism_over_http() {
    let path = "/api/v1/mark/hero?type=aurora&text=Same&animation=none";
    let (_, _, a) = get(path).await;
    let (_, _, b) = get(path).await;
    assert_eq!(a, b, "same URL, same mark, forever");
}

#[tokio::test]
async fn svg_export_has_no_raw_template_markers() {
    let (status, ctype, body) = get("/api/v1/mark/hero?type=wave&text=Export&animation=none").await;
    assert_eq!(status, StatusCode::OK);
    assert!(ctype.contains("svg"), "ctype={ctype}");
    assert!(
        !body.contains('\\'),
        "SVG must not expose Rust template continuation markers"
    );
}

#[tokio::test]
async fn root_is_the_studio_unless_the_query_is_a_typing_url() {
    for path in ["/", "/?form=hero&text=Hi", "/?text=Hi"] {
        let (status, ctype, body) = get(path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(
            ctype.starts_with("text/html"),
            "{path} stays the studio: {ctype}"
        );
        assert!(
            body.contains("<html") || body.contains("<!doctype"),
            "{path}"
        );
    }
    let (status, ctype, body) = get("/?lines=Hello;World&center=true").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        ctype.starts_with("image/svg+xml"),
        "typing dialect: {ctype}"
    );
    assert!(body.contains("<textPath") && body.contains(">World<"));
    let (_, _, native) = get("/typing?lines=Hello;World&center=true").await;
    assert_eq!(body, native, "host swap and /typing are one render");
}

#[tokio::test]
async fn api_is_the_json_index_unless_the_query_is_a_capsule_url() {
    for path in ["/api", "/api?theme=radical", "/api?unknown=1"] {
        let (status, ctype, body) = get(path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(ctype.starts_with("application/json"), "{path}: {ctype}");
        let v: serde_json::Value = serde_json::from_str(&body).expect("json index");
        assert!(v["endpoints"].is_array(), "{path}");
    }
    let (status, ctype, body) = get("/api?type=waving&text=Hello&section=header").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        ctype.starts_with("image/svg+xml"),
        "capsule dialect: {ctype}"
    );
    assert!(body.contains(">Hello</tspan>"));
}

#[tokio::test]
async fn capsule_typography_keys_stay_off_the_native_grammar() {
    // The same knobs on the native hero are unknown input (ADR-0003/0005):
    // they change nothing there.
    let plain = get("/api/v1/mark/hero?type=wave&text=Hi&animation=none")
        .await
        .2;
    let knobs = get(
        "/api/v1/mark/hero?type=wave&text=Hi&animation=none&fontSize=90&fontColor=ff0000\
         &fontAlignY=10&section=footer&reversal=true&rotate=20&textBg=true",
    )
    .await
    .2;
    assert_eq!(plain, knobs);
}

#[tokio::test]
async fn studio_boots_from_wrapped_score_url() {
    let (_, _, body) =
        get("/?url=%2Fapi%2Fv1%2Fmark%2Fscore%3Flabel%3Dagent-ready%26value%3D92").await;
    let boot = studio_boot(&body);
    assert_eq!(boot["form"], "score");
    assert_eq!(boot["score"]["value"], "92");
}

#[tokio::test]
async fn unknown_pages_answer_a_real_404_page() {
    let (status, ctype, body) = get("/no-such-page").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(ctype.starts_with("text/html"), "ctype={ctype}");
    assert!(body.contains("This page does not exist."));
}

async fn get_headers(
    path: &str,
    accept: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, String) {
    let mut req = Request::builder().uri(path);
    if let Some(a) = accept {
        req = req.header(axum::http::header::ACCEPT, a);
    }
    let res = app(state())
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, headers, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn every_html_page_has_the_legal_footer() {
    for (path, want) in [
        ("/", StatusCode::OK),
        ("/docs", StatusCode::OK),
        ("/legal/privacy", StatusCode::OK),
        ("/legal/terms", StatusCode::OK),
        ("/legal/acceptable-use", StatusCode::OK),
        ("/no-such-page", StatusCode::NOT_FOUND),
    ] {
        let (status, _, body) = get(path).await;
        assert_eq!(status, want, "{path}");
        for needle in [
            "href=\"/legal/privacy\"",
            "href=\"/legal/terms\"",
            "href=\"/legal/acceptable-use\"",
            "hi@sylphx.com",
            "16438428",
            "+44 333 335 7935",
            "128 City Road",
        ] {
            assert!(body.contains(needle), "{path} lacks {needle}");
        }
        assert!(!body.contains("{{"), "{path} has an unfilled placeholder");
    }
}

#[tokio::test]
async fn docs_and_legal_pages_are_html() {
    for (path, h1) in [
        ("/docs", "<h1>Docs</h1>"),
        ("/legal/privacy", "<h1>Privacy notice</h1>"),
        ("/legal/terms", "<h1>Terms of use</h1>"),
        ("/legal/acceptable-use", "<h1>Acceptable use policy</h1>"),
    ] {
        let (status, ctype, body) = get(path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(ctype.starts_with("text/html"), "{path}: {ctype}");
        assert!(body.contains(h1), "{path} lacks {h1}");
    }
    for (from, to) in [("/privacy", "/legal/privacy"), ("/terms", "/legal/terms")] {
        let (status, headers, _) = get_headers(from, None).await;
        assert_eq!(status, StatusCode::PERMANENT_REDIRECT, "{from}");
        assert_eq!(headers.get("location").unwrap(), to);
    }
}

#[tokio::test]
async fn api_sends_browsers_to_docs() {
    let (status, headers, _) = get_headers("/api", Some("text/html,application/xhtml+xml")).await;
    assert_eq!(status, StatusCode::FOUND);
    assert_eq!(headers.get("location").unwrap(), "/docs");
    let (status, headers, body) = get_headers("/api", Some("*/*")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers["content-type"]
        .to_str()
        .unwrap()
        .starts_with("application/json"));
    assert_eq!(headers.get("vary").unwrap(), "Accept");
    assert!(body.contains("\"docs\""));
    let (status, headers, _) = get_headers("/api?username=x", Some("text/html")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers["content-type"]
        .to_str()
        .unwrap()
        .starts_with("image/svg+xml"));
}

#[tokio::test]
async fn docs_lists_every_indexed_endpoint() {
    let (_, _, index) = get("/api").await;
    let v: serde_json::Value = serde_json::from_str(&index).unwrap();
    let (_, _, docs) = get("/docs").await;
    for e in v["endpoints"].as_array().unwrap() {
        let e = e.as_str().unwrap().replace('&', "&amp;");
        assert!(docs.contains(&e), "docs lacks {e}");
    }
}

#[tokio::test]
async fn docs_examples_render() {
    let (_, _, docs) = get("/docs").await;
    let mut checked = 0;
    for part in docs.split("src=\"/").skip(1) {
        let src = format!("/{}", &part[..part.find('"').unwrap()]);
        let src = src.replace("&amp;", "&");
        let (status, ctype, _) = get(&src).await;
        assert_eq!(status, StatusCode::OK, "{src}");
        assert!(ctype.starts_with("image/svg+xml"), "{src}: {ctype}");
        checked += 1;
    }
    assert!(checked >= 15, "only {checked} examples");
}

#[tokio::test]
async fn studio_copies_edge_cacheable_svg_urls() {
    let (_, _, body) = get("/").await;
    assert!(body.contains("/api/v1/mark/hero.svg?"));
    assert!(body.contains("\"/badge/\"+path+\".svg\""));
    assert!(body.contains("/streak.svg?"));
}

#[tokio::test]
async fn stats_dialect_and_native_card_agree() {
    let q = "username=ada-dev&show_icons=true&theme=dark&hide_border=true";
    let (_, _, a) = get(&format!("/api?{q}")).await;
    let (_, _, b) = get(&format!("/api/v1/card/stats?{q}")).await;
    assert_eq!(a, b);
}

async fn get_accept(path: &str, accept: &str) -> (StatusCode, axum::http::HeaderMap, String) {
    let res = app(state())
        .oneshot(
            Request::builder()
                .uri(path)
                .header("accept", accept)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, headers) = (res.status(), res.headers().clone());
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, headers, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn api_prefers_json_whenever_it_is_requested() {
    for accept in [
        "application/json",
        "application/json, text/html;q=0.1",
        "text/html;q=0.1, application/json",
        "*/*",
    ] {
        let (status, headers, body) = get_accept("/api", accept).await;
        assert_eq!(status, StatusCode::OK, "{accept}");
        assert!(body.trim_start().starts_with('{'), "{accept}: {body}");
        assert_eq!(headers["vary"], "Accept");
    }
    let (status, headers, _) = get_accept("/api", "text/html,application/xhtml+xml").await;
    assert_eq!(status, StatusCode::FOUND);
    assert_eq!(headers["location"], "/docs");
}

#[tokio::test]
async fn api_images_are_cacheable_but_the_index_and_redirect_are_not() {
    for path in [
        "/api?username=ada-dev",
        "/api?type=wave&text=Hi",
        "/api?type=wave&text=Hi&x=1",
    ] {
        let (status, headers, body) = get_accept(path, "image/svg+xml,*/*").await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(body.contains("<svg"), "{path}");
        let cc = headers["cache-control"].to_str().unwrap();
        assert!(
            cc.contains("max-age") && cc.contains("s-maxage"),
            "{path}: {cc}"
        );
        assert!(headers.contains_key("cdn-cache-control"), "{path}");
    }
    let (_, index, _) = get_accept("/api", "application/json").await;
    assert!(!index.contains_key("cache-control"));
    let (_, redirect, _) = get_accept("/api", "text/html").await;
    assert!(!redirect.contains_key("cache-control"));
}

#[tokio::test]
async fn raw_page_templates_are_never_served_as_files() {
    for path in [
        "/docs.html",
        "/index.html",
        "/404.html",
        "/legal-terms.html",
        "/legal-privacy.html",
        "/legal-acceptable-use.html",
        "/templates/docs.html",
    ] {
        let (status, _, body) = get(path).await;
        assert!(
            !body.contains("{{FOOTER}}") && !body.contains("{{BASE}}"),
            "{path} leaked template text"
        );
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
    let (status, _, body) = get("/docs").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.contains("{{FOOTER}}") && !body.contains("{{BASE}}"));
}
