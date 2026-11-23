//! Text shaping — the single authority for display text.
//!
//! Metrics and the ellipsis crop rule live here, so a form cannot invent its own
//! width model. Hero and profile crop painted lines to the canvas with
//! [`fit_line`]; every form measures with the same [`Metric`] table, and pill
//! width comes from [`crate::capabilities::mark::domain::pill::measure`].

/// Which painted metric the text is measured with.
///
/// `Display` is the regular-weight UI sans used by hero/pill/strip/deploy;
/// `Bold` is the heavier profile-card title, whose sidebearings run wider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Metric {
    Display,
    Bold,
}

impl Metric {
    /// Relative advance of one glyph at `font_size`.
    pub(crate) fn advance(self, ch: char, font_size: f32) -> f32 {
        let unit = match self {
            Self::Display => match class(ch) {
                Space => 0.30,
                Narrow => 0.34,
                Wide => 0.78,
                Bracket => 0.40,
                Upper => 0.58,
                Digit => 0.56,
                // Lowercase runs ~0.55em in Inter/SF and ~0.6em in DejaVu.
                NonAsciiAlnum | Other => 0.56,
            },
            // Bold name weight plus sidebearings run wider than the regular table.
            Self::Bold => match class(ch) {
                Space => 0.30,
                Narrow => 0.34,
                Wide => 0.95,
                Bracket => 0.40,
                Upper => 0.70,
                Digit => 0.58,
                NonAsciiAlnum => 1.05,
                Other => 0.60,
            },
        };
        match self {
            Self::Display => font_size * unit,
            Self::Bold => font_size * unit * 1.12,
        }
    }
}

/// One glyph classification per character, shared by every metric.
#[derive(Clone, Copy)]
enum GlyphClass {
    Space,
    Narrow,
    Wide,
    Bracket,
    Upper,
    Digit,
    NonAsciiAlnum,
    Other,
}

use GlyphClass::*;

fn class(ch: char) -> GlyphClass {
    match ch {
        ' ' | '\u{00A0}' => Space,
        'i' | 'l' | 'I' | 'j' | 't' | 'f' | 'r' | '|' | '\'' | '`' | '!' | '.' | ',' | ':'
        | ';' => Narrow,
        'm' | 'w' | 'M' | 'W' | '@' | '%' => Wide,
        '1' | '(' | ')' | '[' | ']' | '{' | '}' | '/' | '\\' => Bracket,
        c if c.is_ascii_uppercase() => Upper,
        c if c.is_ascii_digit() => Digit,
        c if !c.is_ascii() && c.is_alphanumeric() => NonAsciiAlnum,
        _ => Other,
    }
}

/// `font-family` stacks, single-sourced: banner/profile/deploy paint content
/// typography, pill-shaped marks add the shields badge stacks.
pub(crate) const FONT_MONO: &str = "ui-monospace,SFMono-Regular,Menlo,Consolas,monospace";
pub(crate) const FONT_UI_SANS: &str =
    "Inter,ui-sans-serif,system-ui,-apple-system,BlinkMacSystemFont,Segoe UI,Roboto,Helvetica,Arial,sans-serif";
/// shields' badge faces: the Verdana stack its width tables were measured in,
/// and the Helvetica stack of the social style.
pub(crate) const FONT_SHIELDS_SANS: &str = "Verdana,Geneva,DejaVu Sans,sans-serif";
pub(crate) const FONT_SHIELDS_SOCIAL: &str = "Helvetica Neue,Helvetica,Arial,sans-serif";

/// Content typography for `font=sans|mono` (hero, profile, deploy).
pub(crate) fn content_family(font: Option<&str>) -> &'static str {
    match font.map(|f| f.to_ascii_lowercase()).as_deref() {
        Some("mono") => FONT_MONO,
        _ => FONT_UI_SANS,
    }
}

/// A URL-requested `font-family` (dialects): the requested families first,
/// then a system stack. No webfont is fetched (no upstream), so the stack is
/// what renders when the viewer lacks the family: a code/pixel family falls
/// back to the monospace stack, anything else to the sans stack. Names are
/// sanitized to `[0-9A-Za-z- ]` and single-quoted, so the value is safe inside
/// a double-quoted attribute.
pub(crate) fn requested_family(raw: &str) -> String {
    const GENERIC: [&str; 6] = [
        "serif",
        "sans-serif",
        "monospace",
        "cursive",
        "fantasy",
        "system-ui",
    ];
    const MONO_HINTS: [&str; 12] = [
        "mono",
        "code",
        "consol",
        "courier",
        "menlo",
        "hack",
        "inconsolata",
        "terminal",
        "vt323",
        "press start",
        "pixel",
        "typewriter",
    ];
    let names: Vec<String> = raw
        .split(',')
        .map(|n| {
            n.chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == ' ')
                .take(60)
                .collect::<String>()
                .trim()
                .to_string()
        })
        .filter(|n| !n.is_empty())
        .take(8)
        .collect();
    let lower: Vec<String> = names.iter().map(|n| n.to_ascii_lowercase()).collect();
    match lower.first().map(String::as_str) {
        None | Some("sans") => return FONT_UI_SANS.into(),
        Some("mono") | Some("monospace") if names.len() == 1 => return FONT_MONO.into(),
        _ => {}
    }
    let mono = lower
        .iter()
        .any(|n| MONO_HINTS.iter().any(|h| n.contains(h)));
    let mut out: Vec<String> = names
        .iter()
        .zip(&lower)
        .map(|(n, l)| {
            if GENERIC.contains(&l.as_str()) {
                l.clone()
            } else {
                format!("'{n}'")
            }
        })
        .collect();
    out.push(if mono { FONT_MONO } else { FONT_UI_SANS }.into());
    out.join(",")
}

/// Total advance of a painted line.
pub(crate) fn line_advance(line: &str, font_size: f32, metric: Metric) -> f32 {
    line.chars().map(|c| metric.advance(c, font_size)).sum()
}

/// Crop a line to `max_px`, marking the cut with `…`.
pub(crate) fn fit_line(line: &str, max_px: f32, font_size: f32, metric: Metric) -> String {
    if line_advance(line, font_size, metric) <= max_px {
        return line.to_string();
    }
    let ell = '\u{2026}';
    let budget = (max_px - metric.advance(ell, font_size)).max(0.0);
    let mut out = String::new();
    let mut used = 0.0;
    for ch in line.chars() {
        let adv = metric.advance(ch, font_size);
        if used + adv > budget {
            break;
        }
        out.push(ch);
        used += adv;
    }
    out.push(ell);
    out
}

/// Cap a display string at `max` chars, marking truncation with `…`.
/// Total length never exceeds `max`.
///
/// This is the public bounded-input rule (`docs/capabilities.md`, limits):
/// contract tests assert it directly.
pub fn cap_text(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
}

/// Two-letter monogram from a display name (profile tile + hero plate).
///
/// Letters come from the supplied name. The `MK` fallback is only for empty
/// or punctuation-only input — never a substitute for non-Latin letters.
pub(crate) fn monogram(text: &str) -> String {
    let parts: Vec<&str> = text
        .split(|c: char| c.is_whitespace() || c == '-' || c == '_')
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() >= 2 {
        match (first_letter(parts[0]), first_letter(parts[1])) {
            (Some(a), Some(b)) => format!("{a}{b}"),
            (Some(a), None) => format!("{a}{a}"),
            (None, Some(b)) => format!("{b}{b}"),
            (None, None) => "MK".into(),
        }
    } else {
        let mut chars = text.chars().filter(|c| c.is_alphanumeric()).map(upcase);
        match (chars.next(), chars.next()) {
            (None, _) => "MK".into(),
            (Some(a), None) => format!("{a}{a}"),
            (Some(a), Some(b)) => format!("{a}{b}"),
        }
    }
}

fn first_letter(s: &str) -> Option<char> {
    s.chars().find(|c| c.is_alphanumeric()).map(upcase)
}

fn upcase(c: char) -> char {
    c.to_uppercase().next().unwrap_or(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requested_family_puts_the_request_first_with_a_safe_fallback() {
        assert_eq!(requested_family("monospace"), FONT_MONO);
        assert_eq!(requested_family("sans"), FONT_UI_SANS);
        assert_eq!(requested_family(""), FONT_UI_SANS);
        assert_eq!(
            requested_family("Fira Code"),
            format!("'Fira Code',{FONT_MONO}")
        );
        assert_eq!(
            requested_family("Lobster"),
            format!("'Lobster',{FONT_UI_SANS}")
        );
        assert_eq!(
            requested_family("Roboto,sans-serif"),
            format!("'Roboto',sans-serif,{FONT_UI_SANS}")
        );
        assert_eq!(requested_family("x\"'><y"), format!("'xy',{FONT_UI_SANS}"));
    }

    #[test]
    fn monogram_latin_and_empty_fallback() {
        assert_eq!(monogram("PDF Reader MCP"), "PR");
        assert_eq!(monogram("coderag"), "CO");
        assert_eq!(monogram("Alex Doe"), "AD");
        assert_eq!(monogram(""), "MK");
        assert_eq!(monogram("---"), "MK");
        assert_eq!(monogram("Jo"), "JO");
    }

    #[test]
    fn monogram_uses_supplied_non_latin_letters() {
        assert_eq!(monogram("日本語"), "日本");
        assert_eq!(monogram("李小龙"), "李小");
        assert_eq!(monogram("山田 太郎"), "山太");
        assert_eq!(monogram("Владимир"), "ВЛ");
        assert_eq!(monogram("Émile Zola"), "ÉZ");
    }

    #[test]
    fn cap_text_marks_truncation_and_stays_within_budget() {
        assert_eq!(cap_text("short", 10), "short");
        let capped = cap_text(&"x".repeat(500), 100);
        assert_eq!(capped.chars().count(), 100);
        assert!(capped.ends_with('…'));
    }

    #[test]
    fn fit_line_keeps_short_text_and_marks_the_cut() {
        assert_eq!(fit_line("Ship it", 400.0, 16.0, Metric::Display), "Ship it");
        let long = "A Very Long Display Name That Should Not Escape The Canvas";
        let fitted = fit_line(long, 120.0, 16.0, Metric::Display);
        assert!(fitted.ends_with('…'), "cut is marked: {fitted}");
        assert!(
            line_advance(&fitted, 16.0, Metric::Display) <= 120.0 + 0.001,
            "fitted line stays inside the budget"
        );
        assert!(fitted.chars().count() < long.chars().count());
    }

    #[test]
    fn bold_metric_is_wider_than_display() {
        let wide = "WWWWMMMM";
        assert!(
            line_advance(wide, 24.0, Metric::Bold) > line_advance(wide, 24.0, Metric::Display),
            "profile titles measure wider than hero text"
        );
    }
}
