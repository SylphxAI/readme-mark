//! Dynamic badge faces: shields' default label, message, and color per badge.

use super::date::{age_color, parse_timestamp, relative_age};
use super::format::metric;

/// What a dynamic badge says before the user's query restyles it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Face {
    pub label: String,
    pub message: String,
    pub color: String,
}

fn face(label: &str, message: String, color: &str) -> Face {
    Face {
        label: label.into(),
        message,
        color: color.into(),
    }
}

pub(crate) fn stars(n: u64) -> Face {
    face("stars", metric(n), "blue")
}

pub(crate) fn forks(n: u64) -> Face {
    face("forks", metric(n), "blue")
}

/// Shields' license colors: permissive green, copyleft orange, public domain
/// light green, anything else grey.
pub(crate) fn license(spdx: Option<&str>) -> Face {
    let Some(id) = spdx.filter(|s| !s.is_empty() && *s != "NOASSERTION") else {
        return face("license", "not specified".into(), "lightgrey");
    };
    let upper = id.to_ascii_uppercase();
    let color = if ["CC0-1.0", "UNLICENSE", "WTFPL", "0BSD"].contains(&upper.as_str()) {
        "7cd958"
    } else if ["GPL", "AGPL", "LGPL", "MPL", "EPL", "EUPL", "OSL", "CECILL"]
        .iter()
        .any(|p| upper.starts_with(p))
    {
        "orange"
    } else if [
        "MIT", "APACHE", "BSD", "ISC", "ZLIB", "BSL", "PSF", "PYTHON", "X11", "ARTISTIC",
    ]
    .iter()
    .any(|p| upper.starts_with(p))
    {
        "green"
    } else {
        "lightgrey"
    };
    face("license", id.to_string(), color)
}

/// `v`-prefixed version, shields-style: a bare number gains `v`.
fn versioned(v: &str) -> String {
    let v = v.trim();
    if v.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("v{v}")
    } else {
        v.to_string()
    }
}

/// Pre-releases and 0.x read orange, stable releases blue.
fn version_color(v: &str) -> &'static str {
    let bare = v.trim_start_matches(['v', 'V']);
    if bare.contains('-') || bare.starts_with("0.") {
        "orange"
    } else {
        "blue"
    }
}

pub(crate) fn release(tag: Option<&str>, prerelease: bool) -> Face {
    match tag {
        Some(t) => face(
            "release",
            versioned(t),
            if prerelease {
                "orange"
            } else {
                version_color(t)
            },
        ),
        None => face("release", "no releases found".into(), "lightgrey"),
    }
}

pub(crate) fn last_commit(committed_at: Option<&str>, now_unix: i64) -> Face {
    match committed_at.and_then(parse_timestamp) {
        Some(t) => {
            let age = now_unix - t;
            face("last commit", relative_age(age), age_color(age))
        }
        None => face("last commit", "no commits".into(), "lightgrey"),
    }
}

pub(crate) fn npm_version(v: &str) -> Face {
    face("npm", versioned(v), version_color(v))
}

/// Downloads per `period` (`month`, `week`, or total when `None`).
pub(crate) fn downloads(n: u64, period: Option<&str>) -> Face {
    let message = match period {
        Some(p) => format!("{}/{p}", metric(n)),
        None => metric(n),
    };
    face(
        "downloads",
        message,
        if n > 0 { "brightgreen" } else { "red" },
    )
}

/// npm knows the package but has no download counts for it yet (npm's
/// downloads API lags new packages by a day or more).
pub(crate) fn downloads_pending() -> Face {
    face("downloads", "no data yet".into(), "lightgrey")
}

/// A registry version badge (`pub`, `packagist`, `chrome web store`).
pub(crate) fn version(label: &str, v: &str) -> Face {
    face(label, versioned(v), version_color(v))
}

pub(crate) fn likes(n: u64) -> Face {
    face("likes", metric(n), "blue")
}

/// Pub points out of the maximum, graded like a score.
pub(crate) fn pub_points(points: u64, max: u64) -> Face {
    let ratio = if max == 0 {
        0.0
    } else {
        points as f64 / max as f64
    };
    face("pub points", format!("{points}/{max}"), grade(ratio))
}

fn grade(ratio: f64) -> &'static str {
    match ratio {
        r if r >= 0.9 => "brightgreen",
        r if r >= 0.75 => "green",
        r if r >= 0.5 => "yellow",
        r if r >= 0.25 => "orange",
        _ => "red",
    }
}

/// Bundle size in shields' decimal units (`7.8 kB`).
pub(crate) fn size(label: &str, bytes: u64) -> Face {
    let message = match bytes {
        b if b < 1000 => format!("{b} B"),
        b if b < 1_000_000 => format!("{:.1} kB", b as f64 / 1e3),
        b => format!("{:.2} MB", b as f64 / 1e6),
    };
    face(label, message, "blue")
}

pub(crate) fn users(n: u64) -> Face {
    face("users", metric(n), "blue")
}

/// A 0–5 rating as `4.5/5`, or as stars (`★★★★½`).
pub(crate) fn rating(value: f64, stars: bool) -> Face {
    let message = if stars {
        let halves = (value * 2.0).round() as usize;
        let full = halves / 2;
        let half = if halves % 2 == 1 { "½" } else { "" };
        let empty = 5usize.saturating_sub(full + half.len().min(1));
        format!("{}{half}{}", "★".repeat(full), "☆".repeat(empty))
    } else {
        format!("{:.1}/5", value)
    };
    face("rating", message, grade(value / 5.0))
}

pub(crate) fn rating_count(n: u64) -> Face {
    face("rating count", metric(n), "blue")
}

/// A workflow's newest completed run, shields' words and colours.
pub(crate) fn workflow(name: &str, conclusion: Option<&str>) -> Face {
    let (message, color) = match conclusion {
        Some("success") => ("passing", "brightgreen"),
        Some("failure") => ("failing", "red"),
        Some("timed_out") => ("timed out", "red"),
        Some("startup_failure") => ("startup failure", "red"),
        Some("cancelled") => ("cancelled", "lightgrey"),
        Some("skipped") => ("skipped", "lightgrey"),
        Some("action_required") => ("action required", "yellow"),
        Some("neutral") => ("neutral", "lightgrey"),
        _ => ("no status", "lightgrey"),
    };
    face(name, message.into(), color)
}

/// A badge whose subject does not exist upstream.
pub(crate) fn not_found(label: &str, what: &str) -> Face {
    face(label, format!("{what} not found"), "lightgrey")
}

/// A shields path Mark knows the family of but has no source for yet: a calm
/// badge in place of a broken image on someone else's page.
pub(crate) fn unsupported(label: &str) -> Face {
    face(label, "unsupported".into(), "lightgrey")
}

/// Upstream failed and nothing is cached: still a calm, readable badge.
pub(crate) fn unavailable(label: &str) -> Face {
    face(label, "unavailable".into(), "lightgrey")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_follow_shields_defaults() {
        assert_eq!(stars(12_345).message, "12k");
        assert_eq!(license(Some("MIT")).color, "green");
        assert_eq!(license(Some("GPL-3.0")).color, "orange");
        assert_eq!(license(None).message, "not specified");
        assert_eq!(release(Some("1.2.0"), false).message, "v1.2.0");
        assert_eq!(release(Some("v2.0.0-rc.1"), false).color, "orange");
        assert_eq!(npm_version("0.3.1").color, "orange");
        assert_eq!(downloads(1_234_567, Some("month")).message, "1.2M/month");
        assert_eq!(pub_points(140, 160).message, "140/160");
        assert_eq!(size("minzipped size", 3058).message, "3.1 kB");
        assert_eq!(rating(4.47, true).message, "★★★★½");
        assert_eq!(rating(3.0, true).message, "★★★☆☆");
        assert_eq!(rating(4.47, false).message, "4.5/5");
        assert_eq!(workflow("CI", Some("success")).color, "brightgreen");
        assert_eq!(workflow("CI", None).message, "no status");
        let lc = last_commit(Some("2026-09-20T00:00:00Z"), 1_790_294_400);
        assert!(lc.message.ends_with("ago"));
    }
}
