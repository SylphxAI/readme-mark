//! Server-rendered HTML pages that share one footer partial.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;

use crate::bootstrap::AppState;

const FOOTER: &str = include_str!("footer.html");

/// Reads `static/{file}` and fills `{{FOOTER}}` and `{{BASE}}`.
pub(crate) fn render(file: &str, public_base: &str) -> String {
    match std::fs::read_to_string(format!("static/{file}")) {
        Ok(html) => html
            .replace("{{FOOTER}}", FOOTER)
            .replace("{{BASE}}", public_base),
        Err(_) => format!(
            r##"<!doctype html><meta charset=utf-8><title>Mark</title>
        <body style="font-family:system-ui;background:#0d1117;color:#e6edf3;padding:2rem">
        <h1>Mark</h1>
        <p>Beautiful README images from one URL.</p>
        <p>Base: <code>{public_base}</code></p>
        <p><a href="/docs" style="color:#58a6ff">Docs</a> · <a href="/health" style="color:#58a6ff">Health</a></p>
        </body>"##
        ),
    }
}

pub(crate) async fn page(State(st): State<AppState>, file: &'static str) -> Html<String> {
    Html(render(file, &st.public_base))
}

pub(crate) async fn not_found(State(st): State<AppState>) -> (StatusCode, Html<String>) {
    (
        StatusCode::NOT_FOUND,
        Html(render("404.html", &st.public_base)),
    )
}
