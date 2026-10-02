//! Shared entry paths that several dialects answer (ADR-0005 decision 4).
//!
//! `/` and `/api` are where other tools serve their images, so a host swap
//! lands here. Each dispatcher asks the dialects that claim the query in
//! order and falls back to the page the path had before: the studio at `/`,
//! the JSON index at `/api`. Adding a dialect is one more arm.
//!
//! - `/`: readme-typing-svg (`?lines=`), github-readme-streak-stats (`?user=`),
//!   github-profile-trophy (`?username=`).
//! - `/api`: github-readme-stats (`?username=`), capsule-render (`?type=`).

use std::collections::HashMap;

use axum::extract::{Query, RawQuery, State};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};

use super::response::{if_none_match, parse_bool, svg_response_conditional};
use super::{catalog, studio};
use crate::bootstrap::AppState;
use crate::capabilities::live::interfaces::{stats_card, streak_card, trophy_card, CardQuery};
use crate::capabilities::mark::interfaces::dialects::{capsule, typing};

/// `GET /`: readme-typing-svg (`?lines=`), streak-stats (`?user=`),
/// profile-trophy (`?username=`), else
/// the studio.
pub(crate) async fn root(
    state: State<AppState>,
    Query(pairs): Query<HashMap<String, String>>,
    Query(card): Query<CardQuery>,
    RawQuery(raw): RawQuery,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let query = raw.as_deref().unwrap_or("");
    if typing::claims(&pairs) {
        return svg_response_conditional(&typing::svg(&pairs, query), if_none_match(&headers));
    }
    if card.user.is_some() {
        return streak_card(&state, &card, &headers).await;
    }
    if card.username.is_some() {
        return trophy_card(&state, &card, &headers).await;
    }
    studio::index_page(state, uri).await
}

/// `GET /api`: github-readme-stats (`?username=`), capsule-render (`?type=`,
/// `?text=`, …), else the JSON index. `username` claims first: both dialects
/// accept `theme`, only github-readme-stats names a user.
pub(crate) async fn api(
    State(st): State<AppState>,
    Query(pairs): Query<HashMap<String, String>>,
    Query(card): Query<CardQuery>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> Response {
    if card.username.is_some() {
        return stats_card(&st, &card, &headers).await;
    }
    let query = raw.as_deref().unwrap_or("");
    if capsule::claims(&pairs) {
        let credit = parse_bool(pairs.get("credit").map(String::as_str), st.default_credit);
        let svg = capsule::render(&pairs, query, credit);
        return svg_response_conditional(&svg, if_none_match(&headers));
    }
    let wants_html = prefers_html(
        headers
            .get(axum::http::header::ACCEPT)
            .and_then(|v| v.to_str().ok()),
    );
    if wants_html {
        return (
            StatusCode::FOUND,
            [
                (axum::http::header::LOCATION, "/docs"),
                (axum::http::header::VARY, "Accept"),
            ],
        )
            .into_response();
    }
    let mut res = catalog::api_index(State(st)).await.into_response();
    res.headers_mut().insert(
        axum::http::header::VARY,
        axum::http::HeaderValue::from_static("Accept"),
    );
    res
}

/// Whether an `Accept` header asks for the HTML docs instead of the JSON
/// index: `text/html` is accepted and `application/json` is not. JSON wins
/// whenever it is requested (`application/json, text/html;q=0.1`).
fn prefers_html(accept: Option<&str>) -> bool {
    let (mut html, mut json) = (false, false);
    for part in accept.unwrap_or("").split(',') {
        let mut fields = part.split(';');
        let kind = fields.next().unwrap_or("").trim().to_ascii_lowercase();
        let rejected = fields.any(|f| {
            f.trim()
                .strip_prefix("q=")
                .and_then(|q| q.trim().parse::<f32>().ok())
                .is_some_and(|q| q <= 0.0)
        });
        if rejected {
            continue;
        }
        match kind.as_str() {
            "text/html" => html = true,
            "application/json" => json = true,
            _ => {}
        }
    }
    html && !json
}

#[cfg(test)]
mod tests {
    use super::prefers_html;

    #[test]
    fn json_wins_whenever_it_is_requested() {
        assert!(prefers_html(Some("text/html,application/xhtml+xml")));
        assert!(prefers_html(Some("text/html")));
        assert!(!prefers_html(Some("application/json")));
        assert!(!prefers_html(Some("application/json, text/html;q=0.1")));
        assert!(!prefers_html(Some("text/html, application/json")));
        assert!(!prefers_html(Some("*/*")));
        assert!(!prefers_html(None));
        assert!(
            prefers_html(Some("text/html, application/json;q=0")),
            "q=0 means not acceptable"
        );
    }
}
