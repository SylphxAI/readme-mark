//! Quality signals (owner standard `quality-signals`, cloud ADR
//! `quality-signals-to-work`): Mark reports its own problems as one structured
//! log line each, and every finished image request as a journey line.
//!
//! - `event=mark.issue.<kind>.<subject> severity=info`: the event name is the
//!   problem's fingerprint. The platform counts the line, raises one alert per
//!   fingerprint, and the work intake files one item for it (a repeat while it
//!   is open is a note; a return after it closed is a regression).
//! - `event=mark.image.ok|failed severity=info`: one line per finished image
//!   request, the events of the `mark-image-success` SLO.
//!
//! A line carries only enumerated values: the kind, a subject taken from the
//! route table, an upstream name the code names, an HTTP status and the
//! release. Never a path with its ids, a query, a body or a message.

use axum::extract::{MatchedPath, Request};
use axum::middleware::Next;
use axum::response::Response;

/// The product prefix of every event.
const PRODUCT: &str = "mark";
/// The platform counts an event name of at most this many characters.
const MAX_EVENT: usize = 64;

/// What went wrong (the ADR's kinds that a static image server can see by
/// itself).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A dependency a request needed failed: a live registry or GitHub read.
    ToolFailed,
    /// A request ended in an error the caller saw.
    TurnFailed,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::ToolFailed => "tool_failed",
            Kind::TurnFailed => "turn_failed",
        }
    }
}

/// The fingerprint of a problem: `mark.issue.<kind>.<subject>`, only
/// `[a-z0-9_.]`, at most 64 characters. Anything else in `subject` becomes
/// `_`; a subject too long keeps its head and gets a short hash of the whole,
/// so two long subjects never share a name.
pub fn fingerprint(kind: Kind, subject: &str) -> String {
    let head = format!("{PRODUCT}.issue.{}.", kind.name());
    let mut s = String::with_capacity(subject.len());
    for c in subject.chars() {
        let c = c.to_ascii_lowercase();
        let c = if c.is_ascii_lowercase() || c.is_ascii_digit() {
            c
        } else {
            '_'
        };
        if !(c == '_' && (s.is_empty() || s.ends_with('_'))) {
            s.push(c);
        }
    }
    while s.ends_with('_') {
        s.pop();
    }
    if s.is_empty() {
        s.push_str("unknown");
    }
    let room = MAX_EVENT - head.len();
    if s.len() > room {
        let hash = format!("{:08x}", fnv1a(subject.as_bytes()) as u32);
        s.truncate(room - hash.len() - 1);
        s.push('_');
        s.push_str(&hash);
    }
    head + &s
}

fn fnv1a(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325, |h, &x| {
        (h ^ x as u64).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The subject of a route: its template with the parameters left out, so
/// `/api/v1/card/{kind}` is `api_v1_card`. Only the route table's own text
/// enters it.
pub fn route_subject(template: &str) -> String {
    template
        .split('/')
        .filter(|seg| !seg.is_empty() && !seg.starts_with('{') && !seg.starts_with(':'))
        .collect::<Vec<_>>()
        .join("_")
}

/// The release this process runs (the platform's revision name), or `-`.
fn release() -> &'static str {
    static R: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    R.get_or_init(|| std::env::var("K_REVISION").unwrap_or_else(|_| "-".into()))
}

/// Report one occurrence of a problem. `code` is an enumerated detail (an
/// upstream name, an HTTP status, a fixed reason name), never free text.
pub fn issue(kind: Kind, subject: &str, code: &str) {
    let event = fingerprint(kind, subject);
    tracing::info!(event = %event, severity = "info", code, release = release(), "issue");
}

/// The journey line of one finished image request.
fn journey(ok: bool, status: u16) {
    let event = if ok {
        "mark.image.ok"
    } else {
        "mark.image.failed"
    };
    tracing::info!(
        event,
        severity = "info",
        status,
        release = release(),
        "journey"
    );
}

/// Route middleware (on matched routes): an image request writes its journey
/// line, and a 5xx reports `turn_failed` for the route's template -- never the
/// path's own ids, which the matched template already drops.
pub async fn observe(req: Request, next: Next) -> Response {
    let subject = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| route_subject(p.as_str()))
        .unwrap_or_default();
    let r = next.run(req).await;
    let status = r.status().as_u16();
    if status >= 500 {
        issue(Kind::TurnFailed, &subject, &status.to_string());
    }
    journey(status < 500, status);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid(e: &str) -> bool {
        !e.is_empty()
            && e.len() <= MAX_EVENT
            && e.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.')
    }

    #[test]
    fn fingerprint_is_bounded_and_stable() {
        assert_eq!(
            fingerprint(Kind::TurnFailed, &route_subject("/api/v1/card/{kind}")),
            "mark.issue.turn_failed.api_v1_card"
        );
        assert_eq!(
            fingerprint(Kind::ToolFailed, "github"),
            "mark.issue.tool_failed.github"
        );
        assert_eq!(
            fingerprint(Kind::TurnFailed, ""),
            "mark.issue.turn_failed.unknown"
        );
        // free text never survives as-is: case, spaces and punctuation fold
        let f = fingerprint(Kind::ToolFailed, "Bob's Email <a@b.c>!");
        assert!(valid(&f), "{f}");
        assert!(!f.contains('@') && !f.contains(' '), "{f}");
    }

    #[test]
    fn long_subjects_keep_distinct_names_within_the_limit() {
        let a = "x".repeat(100) + "a";
        let b = "x".repeat(100) + "b";
        let (fa, fb) = (
            fingerprint(Kind::ToolFailed, &a),
            fingerprint(Kind::ToolFailed, &b),
        );
        assert!(valid(&fa) && valid(&fb), "{fa} {fb}");
        assert_eq!(fa.len(), MAX_EVENT);
        assert_ne!(fa, fb);
    }

    /// The lines the middleware writes, through the same text formatter the
    /// server uses (the platform's counter reads `event=` and `severity=`).
    #[derive(Clone, Default)]
    struct Capture(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
    impl std::io::Write for Capture {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn events(out: &str) -> Vec<String> {
        out.lines()
            .filter_map(|l| l.split("event=").nth(1))
            .map(|r| {
                r.split_whitespace()
                    .next()
                    .unwrap()
                    .trim_matches('"')
                    .to_string()
            })
            .collect()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn a_failed_request_reports_one_fingerprint_and_every_image_its_journey() {
        use axum::http::StatusCode;
        use axum::routing::get;
        use tower::ServiceExt;

        let cap = Capture::default();
        let w = cap.clone();
        let sub = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_writer(move || w.clone())
            .finish();
        let _g = tracing::subscriber::set_default(sub);

        let app = axum::Router::new()
            .route("/badge/{*tail}", get(|| async { "ok" }))
            .route(
                "/api/v1/card/{kind}",
                get(
                    |axum::extract::Path(k): axum::extract::Path<String>| async move {
                        if k == "boom" {
                            StatusCode::INTERNAL_SERVER_ERROR
                        } else {
                            StatusCode::OK
                        }
                    },
                ),
            )
            .route_layer(axum::middleware::from_fn(observe));
        let call = |p: &str| {
            axum::http::Request::builder()
                .uri(p)
                .body(axum::body::Body::empty())
                .unwrap()
        };
        // the same induced failure twice: one fingerprint, two occurrences
        for _ in 0..2 {
            app.clone()
                .oneshot(call("/api/v1/card/boom"))
                .await
                .unwrap();
        }
        app.clone().oneshot(call("/badge/a-b-c")).await.unwrap();

        let out = String::from_utf8(cap.0.lock().unwrap().clone()).unwrap();
        assert_eq!(
            events(&out),
            [
                "mark.issue.turn_failed.api_v1_card",
                "mark.image.failed",
                "mark.issue.turn_failed.api_v1_card",
                "mark.image.failed",
                "mark.image.ok",
            ],
            "{out}"
        );
        assert!(
            out.lines().all(|l| l.contains("severity=\"info\"")),
            "{out}"
        );
        // a parameter never reaches a line: the route template is the subject
        assert!(!out.contains("boom"), "{out}");
    }
}
