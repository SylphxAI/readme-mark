//! Typing application: TypingSpec → animated typing SVG.
//!
//! The technique is readme-typing-svg's, kept byte-for-byte in spirit so a
//! host swap renders the same thing: each line is a `<textPath>` on a path
//! whose `d` is SMIL-animated from zero length to the full width. Glyphs past
//! the end of the path are not painted, so the line appears to be typed. SMIL
//! runs inside GitHub's `<img>` proxy; no script, no CSS animation, no font
//! fetch.

use crate::capabilities::mark::domain::svg::{esc, svg_doc_with};
use crate::capabilities::mark::domain::text::requested_family;
use crate::capabilities::mark::domain::{MarkSpec, TypingSpec};

/// Blinking caret glyph appended to a typed line. It sits in the same text run,
/// so the typing reveal uncovers it exactly at the end of the text, whatever
/// font the viewer resolves.
const CARET: &str = "\u{258F}";

/// A viewer who asks for reduced motion sees this poster frame instead of the
/// animation: the same text, fully typed, no movement. SMIL cannot read the
/// media query, so both layers are drawn and CSS picks one.
const MOTION_STYLE: &str = "<style>.mk-still{display:none}\
     @media (prefers-reduced-motion:reduce){.mk-live{display:none}.mk-still{display:inline}}</style>";

pub fn render(spec: &MarkSpec) -> String {
    let t = &spec.typing;
    let family = requested_family(&t.font);
    let last = t.lines.len().saturating_sub(1);
    let mut live = String::new();
    let mut still = String::new();
    let transparent = t.background.len() == 9 && t.background.ends_with("00");
    let mut body = String::from(MOTION_STYLE);
    if !transparent {
        body.push_str(&format!(
            "<rect width=\"100%\" height=\"100%\" fill=\"{}\"/>",
            t.background
        ));
    }
    for (i, line) in t.lines.iter().enumerate() {
        let animate = if t.multiline {
            multiline_animate(t, i, last)
        } else {
            single_line_animate(t, i, last)
        };
        let attrs = format!(
            "font-family=\"{family}\" fill=\"{color}\" font-size=\"{size}\" font-weight=\"{weight}\" \
             dominant-baseline=\"{baseline}\" text-anchor=\"{anchor}\" letter-spacing=\"{spacing}\"",
            color = t.color,
            size = t.size,
            weight = t.weight,
            baseline = if t.v_center { "middle" } else { "auto" },
            anchor = if t.center { "middle" } else { "start" },
            spacing = t.letter_spacing,
        );
        let x = if t.center { "50%" } else { "0%" };
        // The caret follows the line being typed: every line on one baseline,
        // only the last line when the lines are stacked.
        let caret = if t.caret && (!t.multiline || i == last) {
            format!(
                "<tspan fill-opacity=\"1\" dx=\"2\">{CARET}<animate attributeName=\"fill-opacity\" \
                 values=\"1;0\" dur=\"1.1s\" begin=\"0s\" repeatCount=\"indefinite\" calcMode=\"discrete\"/></tspan>"
            )
        } else {
            String::new()
        };
        live.push_str(&format!(
            "<path id=\"path{i}\">{animate}</path>\
             <text {attrs} x=\"{x}\"><textPath href=\"#path{i}\" xlink:href=\"#path{i}\">{text}{caret}</textPath></text>",
            text = esc(line),
        ));
        if !t.multiline && i > 0 {
            continue;
        }
        let y = if t.multiline {
            f64::from(t.size + 5) * (i + 1) as f64
        } else {
            f64::from(t.height) / 2.0
        };
        still.push_str(&format!(
            "<text {attrs} x=\"{x}\" y=\"{y}\">{}</text>",
            esc(line)
        ));
    }
    body.push_str(&format!(
        "<g class=\"mk-live\">{live}</g><g class=\"mk-still\">{still}</g>"
    ));
    svg_doc_with(
        t.width,
        t.height,
        " xmlns:xlink=\"http://www.w3.org/1999/xlink\"",
        &body,
    )
}

/// Retype on one baseline: line `i` starts when line `i-1` ends; the first
/// line also restarts after the last when `repeat` is on. Typing takes 80% of
/// `duration`, then the line holds for the rest plus `pause`, then clears
/// (the last line stays when `repeat` is off).
fn single_line_animate(t: &TypingSpec, i: usize, last: usize) -> String {
    let begin = match (i, t.repeat) {
        (0, true) => format!("0s;d{last}.end"),
        (0, false) => "0s".into(),
        _ => format!("d{}.end", i - 1),
    };
    let freeze = !t.repeat && i == last;
    let y = f64::from(t.height) / 2.0;
    let empty = format!("m0,{y} h0");
    let full = format!("m0,{y} h{}", t.width);
    let (duration, pause) = (f64::from(t.duration), f64::from(t.pause));
    let total = duration + pause;
    format!(
        "<animate id=\"d{i}\" attributeName=\"d\" begin=\"{begin}\" dur=\"{total}ms\" fill=\"{fill}\" \
         calcMode=\"spline\" values=\"{empty} ; {full} ; {full} ; {end}\" keyTimes=\"0;{k1};{k2};1\" \
         keySplines=\"0 0 1 1;0 0 1 1;0.55 0 1 0.45\"/>",
        fill = if freeze { "freeze" } else { "remove" },
        end = if freeze { &full } else { &empty },
        k1 = 0.8 * duration / total,
        k2 = (0.8 * duration + pause) / total,
    )
}

/// One baseline per line (`multiline=true`): line `i` waits for the lines
/// before it, types over `duration`, and stays.
fn multiline_animate(t: &TypingSpec, i: usize, last: usize) -> String {
    let next = (i + 1) as f64;
    let y = next * f64::from(t.size + 5);
    let line_duration = f64::from(t.duration + t.pause) * next;
    let empty = format!("m0,{y} h0");
    let full = format!("m0,{y} h{}", t.width);
    let repeat = if t.repeat {
        format!(";d{last}.end")
    } else {
        String::new()
    };
    format!(
        "<animate id=\"d{i}\" attributeName=\"d\" begin=\"0s{repeat}\" dur=\"{line_duration}ms\" fill=\"freeze\" \
         values=\"{empty} ; {empty} ; {full} ; {full}\" keyTimes=\"0;{k1};{k2};1\"/>",
        k1 = i as f64 / next,
        k2 = i as f64 / next + f64::from(t.duration) / line_duration,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::mark::domain::MarkForm;

    fn typing(t: TypingSpec) -> String {
        render(&MarkSpec {
            form: MarkForm::Typing,
            typing: t,
            ..Default::default()
        })
    }

    #[test]
    fn single_line_chain_matches_upstream_timing() {
        let svg = typing(TypingSpec {
            lines: vec!["a".into(), "b".into()],
            pause: 1000,
            ..Default::default()
        });
        assert!(svg.contains("id=\"d0\" attributeName=\"d\" begin=\"0s;d1.end\" dur=\"6000ms\""));
        assert!(svg.contains("id=\"d1\" attributeName=\"d\" begin=\"d0.end\""));
        assert!(svg.contains("keyTimes=\"0;0.6666666666666666;0.8333333333333334;1\""));
        assert!(svg.contains("values=\"m0,25 h0 ; m0,25 h400 ; m0,25 h400 ; m0,25 h0\""));
        assert!(
            !svg.contains("<rect"),
            "transparent background paints nothing"
        );
    }

    #[test]
    fn no_repeat_freezes_the_last_line() {
        let svg = typing(TypingSpec {
            lines: vec!["a".into(), "b".into()],
            repeat: false,
            ..Default::default()
        });
        assert!(svg.contains("begin=\"0s\" dur=\"5000ms\" fill=\"remove\""));
        assert!(svg.contains("values=\"m0,25 h0 ; m0,25 h400 ; m0,25 h400 ; m0,25 h400\""));
    }

    #[test]
    fn multiline_stacks_baselines() {
        let svg = typing(TypingSpec {
            lines: vec!["a".into(), "b".into()],
            multiline: true,
            ..Default::default()
        });
        assert!(svg.contains("id=\"d1\" attributeName=\"d\" begin=\"0s;d1.end\" dur=\"10000ms\""));
        assert!(svg.contains("m0,50 h0 ; m0,50 h0 ; m0,50 h400 ; m0,50 h400"));
        assert!(svg.contains("keyTimes=\"0;0.5;1;1\""));
    }

    #[test]
    fn text_is_escaped_and_background_painted() {
        let svg = typing(TypingSpec {
            lines: vec!["<b>&".into()],
            background: "#112233".into(),
            center: true,
            v_center: true,
            ..Default::default()
        });
        assert!(svg.contains("&lt;b&gt;&amp;"));
        assert!(svg.contains("<rect width=\"100%\" height=\"100%\" fill=\"#112233\"/>"));
        assert!(svg.contains("x=\"50%\" text-anchor=\"middle\""));
        assert!(svg.contains("dominant-baseline=\"middle\""));
    }

    #[test]
    fn caret_is_on_by_default_and_reduced_motion_gets_a_still_frame() {
        let svg = typing(TypingSpec {
            lines: vec!["a".into(), "b".into()],
            ..Default::default()
        });
        assert_eq!(svg.matches(CARET).count(), 2, "one caret per line");
        assert!(svg.contains("prefers-reduced-motion:reduce"));
        assert!(svg.contains("class=\"mk-still\"><text"));
        assert_eq!(
            svg.matches("<text").count(),
            3,
            "two live lines and one still"
        );
        let off = typing(TypingSpec {
            caret: false,
            ..Default::default()
        });
        assert!(!off.contains(CARET));
    }

    #[test]
    fn stacked_lines_keep_one_caret_and_a_still_line_each() {
        let svg = typing(TypingSpec {
            lines: vec!["a".into(), "b".into()],
            multiline: true,
            ..Default::default()
        });
        assert_eq!(svg.matches(CARET).count(), 1);
        assert_eq!(svg.matches("<text").count(), 4);
    }
}
