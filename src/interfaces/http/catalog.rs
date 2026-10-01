//! Catalog surface — composition of capability catalogs for the studio/API index.

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

use crate::bootstrap::AppState;
use crate::capabilities::mark::domain::catalog::vocabulary;

pub(crate) async fn api_index(State(st): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "name": "Sylphx Mark",
        "tagline": "Beautiful README images from one URL.",
        "base": st.public_base,
        "docs": format!("{}/docs", st.public_base),
        "endpoints": [
            "/api/v1/mark",
            "/api/v1/mark/{form}",
            "/badge/{label}-{message}-{color}",
            "/icons?i={ids}",
            "/typing?lines={a};{b}",
            "/?lines={a};{b}",
            "/api?type={art}&text={text}",
            "/static/v1",
            "/api/v1/card/{stats|langs|streak|repo|stars}?username=",
            "/api?username=",
            "/api/top-langs?username=",
            "/api/pin?username=&repo=",
            "/streak?user=",
            "/trophy?username=",
            "/github/{stars|forks|license|last-commit}/{owner}/{repo}",
            "/github/v/release/{owner}/{repo}",
            "/npm/{v|dm|dw|dt|l}/{package}",
            "/github/actions/workflow/status/{owner}/{repo}/{file}",
            "/pub/{v|likes|points|dm}/{package}",
            "/packagist/{v|dm|dd|dt}/{vendor}/{package}",
            "/bundlephobia/{min|minzip}/{package}",
            "/chrome-web-store/{v|users|rating|stars|rating-count}/{id}",
            "/svg?repos={owner}/{repo}",
            "/api/v1/catalog",
            "/health"
        ]
    }))
}

pub(crate) async fn catalog() -> impl IntoResponse {
    let mut v = vocabulary();
    if let Some(obj) = v.as_object_mut() {
        obj.insert(
            "limits".into(),
            json!({
                "text": 500,
                "desc": 240,
                "lines": 8,
                "pill_label": 80,
                "pill_message": 120,
                "strip_icons": 60,
                "tile_icons": 300,
                "tile_perline": 50,
                "deploy_service": 40,
                "typing_lines": 32
            }),
        );
        obj.insert(
            "notes".into(),
            json!({
                "grammar": "mark = form × art (type) × paint (theme/color, pill labelColor) × content (text/desc/font) × geometry (width/height, hero layout) × motion (animation)",
                "themes": "neutral design themes — no personal or company names",
                "determinism": "static marks: same URL, same mark, forever — no clock, no upstream, no state",
                "live_data": "cards (/api/v1/card/*) and dynamic badges (/github/*, /npm/*) read public GitHub/npm data with no user token; cached for hours, stale on upstream error, never a broken image",
                "animation_type": "true per-character typewriter with cursor (SMIL)",
                "icons": "any Simple Icons slug or title, plus every skill-icons id (/icons?i=). Simple Icons path data is CC0-1.0; brand names and logos are trademarks of their owners and their use does not imply endorsement — see https://github.com/simple-icons/simple-icons/blob/develop/DISCLAIMER.md"
            }),
        );
    }
    Json(v)
}
