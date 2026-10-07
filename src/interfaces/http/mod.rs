//! HTTP composition root — wires capability interfaces into one router.
//!
//! Domain meaning is not owned here; handlers translate HTTP to capability use cases.

mod catalog;
mod client;
mod dispatch;
mod health;
mod pages;
pub(crate) mod response;
mod studio;
mod unknown;

use axum::extract::Request;
use axum::http::uri::PathAndQuery;
use axum::http::Uri;
use axum::response::Redirect;
use axum::routing::get;
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};

use crate::bootstrap::AppState;
use crate::capabilities::live::interfaces as live_http;
use crate::capabilities::mark::interfaces as mark_http;

/// Image route prefixes. Each also answers with `.svg` appended to its path
/// (ADR-0005 decision 7: Cloudflare caches by extension). A new image route
/// family adds its prefix here and gets the suffix for free.
pub(crate) const IMAGE_ROUTE_PREFIXES: &[&str] = &[
    "/api/v1/mark",
    "/badge",
    "/static/v1",
    "/icons",
    "/typing",
    // Live cards and dynamic badges (MARK-LIVE).
    "/api/v1/card",
    "/api/top-langs",
    "/api/pin",
    "/streak",
    "/trophy",
    "/github",
    "/npm",
    "/pub",
    "/packagist",
    "/bundlephobia",
    "/chrome-web-store",
    // Shields sources that complete the host swap.
    "/pypi",
    "/crates",
    "/docker",
    "/endpoint",
];

/// `/badge/a-b-c.svg` → `/badge/a-b-c`; only image routes are rewritten, so
/// real `.svg` files under `static/` keep being served as files.
fn strip_svg_suffix(path: &str) -> Option<&str> {
    let bare = path.strip_suffix(".svg")?;
    IMAGE_ROUTE_PREFIXES
        .iter()
        .any(|p| {
            bare.strip_prefix(p)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
        })
        .then_some(bare)
}

async fn rewrite_svg_suffix(mut req: Request) -> Request {
    let uri = req.uri();
    if let Some(bare) = strip_svg_suffix(uri.path()) {
        let pq = match uri.query() {
            Some(q) => format!("{bare}?{q}"),
            None => bare.to_string(),
        };
        if let Ok(pq) = PathAndQuery::try_from(pq) {
            let mut parts = uri.clone().into_parts();
            parts.path_and_query = Some(pq);
            if let Ok(rewritten) = Uri::from_parts(parts) {
                *req.uri_mut() = rewritten;
            }
        }
    }
    req
}

pub fn app(state: AppState) -> Router {
    // The suffix rewrite must run before routing, so the routed app sits
    // behind an outer router whose only job is the rewrite.
    Router::new()
        .fallback_service(routes(state))
        .layer(axum::middleware::map_request(rewrite_svg_suffix))
}

fn routes(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        // `/api` is also capsule-render's image path (ADR-0005 dialects).
        .route("/api", get(dispatch::api))
        .route("/api/v1", get(catalog::api_index))
        .route("/api/v1/catalog", get(catalog::catalog))
        // One surface (ADR-0003): /api/v1/mark/{form} is the whole grammar,
        // plus the shields static badge dialect (/badge/…, /static/v1).
        // Every legacy capability route is deleted (banner, badge, icons,
        // brand, deploy, stats, org, repo).
        .route("/api/v1/mark", get(mark_http::mark_default_handler))
        .route("/api/v1/mark/{form}", get(mark_http::mark_handler))
        .route("/badge/{*tail}", get(mark_http::badge_path))
        .route("/icons", get(mark_http::icons_handler))
        .route("/static/v1", get(mark_http::static_v1))
        .route("/typing", get(mark_http::typing_handler))
        // Live data (ADR-0005, MARK-LIVE): hour-scale cache, never a broken
        // image. github-readme-stats / streak-stats / shields paths swap hosts.
        .route("/api/v1/card/{kind}", get(live_http::card_handler))
        .route("/api/top-langs", get(live_http::top_langs_handler))
        .route("/api/pin", get(live_http::pin_handler))
        .route("/streak", get(live_http::streak_handler))
        .route("/trophy", get(live_http::trophy_handler))
        .route(
            "/github/v/release/{owner}/{repo}",
            get(live_http::release_badge),
        )
        .route(
            "/github/last-commit/{owner}/{repo}/{branch}",
            get(live_http::last_commit_branch),
        )
        .route(
            "/github/{kind}/{owner}/{repo}",
            get(live_http::github_badge),
        )
        .route("/npm/{kind}/{*package}", get(live_http::npm_badge))
        .route(
            "/github/actions/workflow/status/{owner}/{repo}/{file}",
            get(live_http::workflow_badge),
        )
        .route("/pub/{kind}/{package}", get(live_http::pub_badge))
        .route(
            "/packagist/{kind}/{vendor}/{package}",
            get(live_http::packagist_badge),
        )
        .route(
            "/bundlephobia/{kind}/{*package}",
            get(live_http::bundlephobia_badge),
        )
        .route(
            "/chrome-web-store/{kind}/{id}",
            get(live_http::chrome_badge),
        )
        .route("/pypi/{kind}/{*package}", get(live_http::pypi_badge))
        .route("/crates/{kind}/{name}", get(live_http::crates_badge))
        .route(
            "/docker/{kind}/{user}/{image}",
            get(live_http::docker_badge),
        )
        .route("/endpoint", get(live_http::endpoint_badge))
        .route("/svg", get(live_http::star_history_handler))
        // `/` is the studio, readme-typing-svg's image path (`?lines=`), and
        // github-readme-streak-stats' (`?user=`).
        .route("/", get(dispatch::root))
        .route("/docs", get(|s| pages::page(s, "docs.html")))
        .route(
            "/legal/privacy",
            get(|s| pages::page(s, "legal-privacy.html")),
        )
        .route("/legal/terms", get(|s| pages::page(s, "legal-terms.html")))
        .route(
            "/legal/acceptable-use",
            get(|s| pages::page(s, "legal-acceptable-use.html")),
        )
        .route(
            "/privacy",
            get(|| async { Redirect::permanent("/legal/privacy") }),
        )
        .route(
            "/terms",
            get(|| async { Redirect::permanent("/legal/terms") }),
        )
        // Anything else is a static file, an `unsupported` SVG badge for a
        // badge-like path (`unknown`), or the Mark-styled 404 page with a
        // real 404 status.
        .fallback_service(
            ServeDir::new("static").fallback(get(unknown::not_found).with_state(state.clone())),
        )
        // Public SVG GET is origin-independent (`ACAO: *`, no credentials).
        // Default CorsLayer Vary includes Origin, which splits the CDN cache
        // key without changing bytes. Empty vary is correct: allowed
        // origin/methods/headers are constants, not mirrored.
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
                .vary(Vec::<axum::http::HeaderName>::new()),
        )
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO))
                .on_response(DefaultOnResponse::new().level(tracing::Level::INFO)),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            client::scope,
        ))
        // Quality signals sit outermost so they see the final status: a 5xx
        // reports `mark.issue.turn_failed.<route>` and every image request
        // writes its `mark.image.ok|failed` journey line.
        .layer(axum::middleware::from_fn(crate::signals::observe))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::strip_svg_suffix;

    #[test]
    fn only_image_routes_lose_the_suffix() {
        assert_eq!(strip_svg_suffix("/badge/a-b-c.svg"), Some("/badge/a-b-c"));
        assert_eq!(strip_svg_suffix("/api/v1/mark.svg"), Some("/api/v1/mark"));
        assert_eq!(
            strip_svg_suffix("/api/v1/mark/hero.svg"),
            Some("/api/v1/mark/hero")
        );
        assert_eq!(strip_svg_suffix("/static/v1.svg"), Some("/static/v1"));
        assert_eq!(strip_svg_suffix("/icons.svg"), Some("/icons"));
        assert_eq!(strip_svg_suffix("/logo.svg"), None);
        assert_eq!(strip_svg_suffix("/api/v1/markx.svg"), None);
        assert_eq!(strip_svg_suffix("/badge/a-b-c"), None);
    }
}
