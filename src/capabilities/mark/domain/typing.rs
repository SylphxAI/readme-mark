//! Typing text: the readme-typing-svg parameter set, normalized.
//!
//! Every parameter of DenverCoder1/readme-typing-svg is honored with its
//! upstream default (`lines`, `separator`, `font`, `weight`, `size`, `color`,
//! `background`, `center`, `vCenter`, `multiline`, `width`, `height`,
//! `duration`, `pause`, `repeat`, `random`, `letterSpacing`). Where upstream
//! answers an error page, Mark normalizes instead: rendering never fails.
//!
//! Determinism: upstream `random=true` shuffles with PHP's RNG on every
//! request. Here the order is a Fisher–Yates shuffle driven by an LCG seeded
//! with FNV-1a/32 of the URL's query string, so one URL always renders one
//! order (and a different URL may render another). No clock, no RNG.

use std::collections::HashMap;

use crate::capabilities::mark::domain::color::resolve_paint;
use crate::capabilities::mark::domain::hash::fnv1a_32;
use crate::capabilities::mark::domain::svg::normalize_hex_token;
use crate::capabilities::mark::domain::text::cap_text;
use crate::capabilities::mark::domain::MAX_TEXT_CHARS;

/// Upper bound on typed lines (bounded input).
pub(crate) const MAX_TYPING_LINES: usize = 32;

/// Shown when a URL names no line at all (upstream answers an error).
const DEFAULT_LINE: &str = "Hello, world!";

/// A normalized typing mark.
#[derive(Debug, Clone, PartialEq)]
pub struct TypingSpec {
    pub lines: Vec<String>,
    /// Requested family, sanitized to `[0-9A-Za-z- ]` (upstream's rule).
    pub font: String,
    pub weight: u32,
    pub size: u32,
    /// Canonical `#hex` token.
    pub color: String,
    /// Canonical `#hex` token (`#00000000` is transparent).
    pub background: String,
    pub center: bool,
    pub v_center: bool,
    pub multiline: bool,
    pub width: u32,
    pub height: u32,
    /// Typing time per line, ms.
    pub duration: u32,
    /// Hold after a line is typed, ms.
    pub pause: u32,
    pub repeat: bool,
    /// A soft caret after the typed line (`caret=false` turns it off).
    pub caret: bool,
    /// A CSS keyword or `<number><unit>`, validated.
    pub letter_spacing: String,
}

impl Default for TypingSpec {
    fn default() -> Self {
        Self {
            lines: vec![DEFAULT_LINE.into()],
            font: "monospace".into(),
            weight: 400,
            size: 20,
            color: "#36BCF7".into(),
            background: "#00000000".into(),
            center: false,
            v_center: false,
            multiline: false,
            width: 400,
            height: 50,
            duration: 5000,
            pause: 0,
            repeat: true,
            caret: true,
            letter_spacing: "normal".into(),
        }
    }
}

impl TypingSpec {
    /// Build from decoded query pairs. `query` is the raw query string, the
    /// seed of the deterministic `random=true` order. `text` (the native
    /// grammar's content key) stands in for `lines` when `lines` is absent.
    pub(crate) fn from_pairs(pairs: &HashMap<String, String>, query: &str) -> Self {
        let d = Self::default();
        let get = |k: &str| pairs.get(k).map(String::as_str);
        let separator = get("separator").filter(|s| !s.is_empty()).unwrap_or(";");
        let mut lines = match (get("lines"), get("text")) {
            (Some(raw), _) if !raw.is_empty() => split_lines(raw, separator),
            (_, Some(text)) if !text.is_empty() => split_lines(&text.replace("-nl-", "\n"), "\n"),
            _ => d.lines.clone(),
        };
        if lines.is_empty() {
            lines = d.lines.clone();
        }
        if flag(get("random"), false) {
            shuffle(&mut lines, fnv1a_32(query.as_bytes()));
        }
        Self {
            lines,
            font: get("font")
                .map(sanitize_font)
                .filter(|f| !f.trim().is_empty())
                .unwrap_or(d.font),
            weight: positive(get("weight"), d.weight).clamp(100, 1000),
            size: positive(get("size"), d.size).min(400),
            color: color(get("color")).unwrap_or(d.color),
            background: color(get("background")).unwrap_or(d.background),
            center: flag(get("center"), d.center),
            v_center: flag(get("vCenter"), d.v_center),
            multiline: flag(get("multiline"), d.multiline),
            width: positive(get("width"), d.width).min(4000),
            height: positive(get("height"), d.height).min(4000),
            duration: positive(get("duration"), d.duration).min(600_000),
            pause: non_negative(get("pause"), d.pause).min(600_000),
            repeat: flag(get("repeat"), d.repeat),
            caret: flag(get("caret"), d.caret),
            letter_spacing: get("letterSpacing")
                .filter(|s| valid_letter_spacing(s))
                .map(str::to_string)
                .unwrap_or(d.letter_spacing),
        }
    }
}

/// Upstream trims one trailing separator (when it is one character), then
/// splits. Each line is capped like any other mark text.
fn split_lines(raw: &str, separator: &str) -> Vec<String> {
    let mut raw = raw;
    if separator.chars().count() == 1 {
        raw = raw.trim_end_matches(separator);
    }
    raw.split(separator)
        .take(MAX_TYPING_LINES)
        .map(|l| cap_text(l, MAX_TEXT_CHARS))
        .collect()
}

fn shuffle(lines: &mut [String], seed: u32) {
    let mut state = seed;
    for i in (1..lines.len()).rev() {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let j = (state >> 8) as usize % (i + 1);
        lines.swap(i, j);
    }
}

/// Upstream booleans: only `true` (any case) is true.
fn flag(v: Option<&str>, default: bool) -> bool {
    v.map(|s| s.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(default)
}

/// Upstream numbers: keep digits and `-`, read the integer; `20px` is `20`.
fn integer(v: &str) -> Option<i64> {
    let kept: String = v
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    let end = kept
        .char_indices()
        .find(|(i, c)| !(c.is_ascii_digit() || (*i == 0 && *c == '-')))
        .map_or(kept.len(), |(i, _)| i);
    kept[..end]
        .parse::<i64>()
        .ok()
        .map(|n| n.min(i64::from(u32::MAX)))
}

fn positive(v: Option<&str>, default: u32) -> u32 {
    v.and_then(integer)
        .filter(|n| *n > 0)
        .map_or(default, |n| n as u32)
}

fn non_negative(v: Option<&str>, default: u32) -> u32 {
    v.and_then(integer)
        .filter(|n| *n >= 0)
        .map_or(default, |n| n as u32)
}

/// Upstream strips non-hex characters and takes 3/4/6/8 digits; Mark also
/// accepts its named colors. Anything else is the default.
fn color(v: Option<&str>) -> Option<String> {
    let v = v?.trim();
    let hex: String = v.chars().filter(char::is_ascii_hexdigit).collect();
    let hex = if hex.len() == 4 {
        hex.chars().flat_map(|c| [c, c]).collect()
    } else {
        hex
    };
    normalize_hex_token(&hex).or_else(|| {
        let named = resolve_paint(Some(v), "");
        (!named.is_empty()).then_some(named)
    })
}

fn sanitize_font(v: &str) -> String {
    v.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == ' ')
        .take(80)
        .collect()
}

fn valid_letter_spacing(v: &str) -> bool {
    const KEYWORDS: [&str; 6] = [
        "normal",
        "inherit",
        "initial",
        "revert",
        "revert-layer",
        "unset",
    ];
    // Longest first, so `rem` is not read as `em` nor `vmin` as `in`.
    const UNITS: [&str; 15] = [
        "vmin", "vmax", "rem", "px", "em", "pt", "pc", "in", "cm", "mm", "ex", "ch", "vh", "vw",
        "%",
    ];
    if KEYWORDS.contains(&v) {
        return true;
    }
    let Some(unit) = UNITS.iter().find(|u| v.ends_with(**u)) else {
        return false;
    };
    let number = &v[..v.len() - unit.len()];
    let digits = number.strip_prefix('-').unwrap_or(number);
    let mut parts = digits.splitn(2, '.');
    let whole = parts.next().unwrap_or("");
    let frac = parts.next();
    !whole.is_empty()
        && whole.chars().all(|c| c.is_ascii_digit())
        && frac.is_none_or(|f| !f.is_empty() && f.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(q: &str) -> TypingSpec {
        let pairs: HashMap<String, String> = q
            .split('&')
            .filter_map(|p| p.split_once('='))
            .map(|(k, v)| (k.to_string(), v.replace('+', " ")))
            .collect();
        TypingSpec::from_pairs(&pairs, q)
    }

    #[test]
    fn upstream_defaults() {
        let s = spec("lines=Hi");
        assert_eq!(s.lines, vec!["Hi"]);
        assert_eq!((s.width, s.height, s.size, s.weight), (400, 50, 20, 400));
        assert_eq!((s.duration, s.pause), (5000, 0));
        assert_eq!(s.color, "#36BCF7");
        assert_eq!(s.background, "#00000000");
        assert!(s.repeat && !s.center && !s.v_center && !s.multiline);
        assert_eq!(s.font, "monospace");
        assert_eq!(s.letter_spacing, "normal");
    }

    #[test]
    fn lines_split_and_trim_the_trailing_separator() {
        assert_eq!(spec("lines=a;b;c;").lines, vec!["a", "b", "c"]);
        assert_eq!(spec("lines=a|b&separator=|").lines, vec!["a", "b"]);
        assert_eq!(spec("lines=a;;b").lines, vec!["a", "", "b"]);
        assert_eq!(spec("").lines, vec![DEFAULT_LINE]);
        assert_eq!(spec("text=one-nl-two").lines, vec!["one", "two"]);
    }

    #[test]
    fn numbers_colors_and_spacing_follow_upstream_sanitizing() {
        let s = spec("lines=x&size=28px&width=-5&pause=1000&color=f0f&background=00000080");
        assert_eq!((s.size, s.width, s.pause), (28, 400, 1000));
        assert_eq!(s.color, "#ff00ff");
        assert_eq!(s.background, "#00000080");
        assert_eq!(spec("lines=x&color=zz").color, "#36BCF7");
        assert_eq!(spec("lines=x&color=abcd").color, "#aabbccdd");
        assert_eq!(spec("lines=x&letterSpacing=0.5em").letter_spacing, "0.5em");
        assert_eq!(spec("lines=x&letterSpacing=-2px").letter_spacing, "-2px");
        assert_eq!(spec("lines=x&letterSpacing=1e").letter_spacing, "normal");
        assert_eq!(spec("lines=x&letterSpacing=\"x").letter_spacing, "normal");
        assert_eq!(spec("lines=x&font=Fira+Code\"<").font, "Fira Code");
    }

    #[test]
    fn random_order_is_a_pure_function_of_the_url() {
        let q = "lines=a;b;c;d;e;f&random=true";
        let first = spec(q).lines;
        assert_eq!(first, spec(q).lines, "same URL, same order");
        let mut sorted = first.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["a", "b", "c", "d", "e", "f"]);
        assert_ne!(first, spec("lines=a;b;c;d;e;f").lines, "random reorders");
    }
}
