//! Generator UI surface.

use axum::extract::State;
use axum::http::Uri;
use axum::response::{Html, IntoResponse, Response};
use serde::Serialize;

use super::pages;
use crate::bootstrap::AppState;
use crate::capabilities::mark::domain::recovery::parse_public_mark_url;

pub(crate) async fn index_page(State(st): State<AppState>, uri: Uri) -> Response {
    let locator = match uri.query() {
        Some(q) => format!("{}?{q}", uri.path()),
        None => uri.path().to_string(),
    };
    let boot_json = match parse_public_mark_url(&locator) {
        Some(boot) => json_for_script(&boot),
        None => "null".to_string(),
    };
    let html = pages::render("index.html", &st.public_base).replace("{{BOOT}}", &boot_json);
    Html(html).into_response()
}

fn json_for_script(value: &impl Serialize) -> String {
    let json = serde_json::to_string(value).unwrap_or_else(|_| "null".into());
    json.replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}
