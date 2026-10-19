//! Sylphx Mark — embeddable image API (URL → SVG).
//!
//! One concept, one grammar (ADR-0003): form × art × paint × geometry × text ×
//! motion. Every mark is a pure function of its URL — deterministic, immutable,
//! never failing. No clock, no upstream, no state.
//!
//! Public surface: [`capabilities::mark`] (kernel + `render`), the HTTP entry
//! [`app`], [`AppState`], and the [`bootstrap`] shell the `mark` binary needs.
//! Everything else is crate-internal.

pub mod bootstrap;
pub mod capabilities;
pub mod interfaces;
pub mod signals;

pub use bootstrap::AppState;
pub use interfaces::http::app;
