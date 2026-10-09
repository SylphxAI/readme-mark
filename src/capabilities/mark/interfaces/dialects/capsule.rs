//! capsule-render dialect: `/api?type=waving&color=gradient&text=…`.
//!
//! Translates a kyechan99/capsule-render URL into the placed hero
//! (`HeroOverrides` + `render_placed`) so a README switches by changing only
//! the host. Defaults are capsule-render's (`src/model.ts`): height 120,
//! fontSize 70, descSize 20, fontAlign/fontAlignY 50, descAlign 50,
//! descAlignY 60, color `B897FF`, animation `fadeIn`.
//!
//! ADR-0005 decision 4: the typography knobs ADR-0003 retired live here and
//! only here.
//!
//! Determinism: capsule-render picks `random`/`auto`/`gradient` colors with
//! `Math.random()` and `timeAuto`/`timeGradient` from the clock. Here every
//! such choice is FNV-1a/32 of the URL's query string, so one URL always
//! renders one palette.

use std::collections::HashMap;

use super::capsule_palette::{GRADIENTS, PALETTE, THEMES};
use crate::capabilities::mark::application::render_placed;
use crate::capabilities::mark::domain::hash::fnv1a_32;
use crate::capabilities::mark::domain::shapes::capsule::{Silhouette, CAPSULE_WIDTH};
use crate::capabilities::mark::domain::svg::normalize_hex_token;
use crate::capabilities::mark::domain::text::requested_family;
use crate::capabilities::mark::domain::{HeroOverrides, MarkSpec, PlacedText};

/// Keys that mark a query as a capsule-render URL. `theme` alone is not one:
/// github-readme-stats shares it, so the `/api` dispatcher asks its other
/// dialects first.
const CLAIM_KEYS: &[&str] = &[
    "type",
    "text",
    "desc",
    "color",
    "section",
    "height",
    "fontSize",
    "fontColor",
    "fontAlign",
    "fontAlignY",
    "fontFamily",
    "descSize",
    "descAlign",
    "descAlignY",
    "animation",
    "rotate",
    "stroke",
    "strokeWidth",
    "textBg",
    "reversal",
    "customColorList",
];

const DEFAULT_FAMILY: &str = "-apple-system,BlinkMacSystemFont,Segoe UI,Helvetica,Arial,sans-serif,Apple Color Emoji,Segoe UI Emoji";

pub(crate) fn claims(pairs: &HashMap<String, String>) -> bool {
    CLAIM_KEYS.iter().any(|k| pairs.contains_key(*k))
}

pub(crate) fn render(pairs: &HashMap<String, String>, query: &str, credit: bool) -> String {
    let (spec, overrides) = translate(pairs, query, credit);
    render_placed(&spec, &overrides)
}

/// The resolved colors of one banner (capsule-render's constructor rules).
struct Colors {
    paint: Vec<(f32, String)>,
    font: String,
    desc: String,
    text_bg: String,
}

fn translate(
    pairs: &HashMap<String, String>,
    query: &str,
    credit: bool,
) -> (MarkSpec, HeroOverrides) {
    // capsule-render reads an absent or empty value as its default (`||`).
    let get = |k: &str| pairs.get(k).map(String::as_str).filter(|v| !v.is_empty());
    let num = |k: &str, default: f32| {
        get(k)
            .and_then(js_number)
            .filter(|n| *n != 0.0)
            .unwrap_or(default)
    };
    let seed = fnv1a_32(query.as_bytes());
    let colors = resolve_colors(pairs, seed);

    let stroke_color = get("stroke").unwrap_or(if get("strokeWidth").is_some() {
        "B897FF"
    } else {
        "none"
    });
    let stroke_width = num(
        "strokeWidth",
        if get("stroke") == Some("none") {
            0.0
        } else {
            1.0
        },
    );
    let stroke = (stroke_width > 0.0 && stroke_color != "none")
        .then(|| normalize_hex_token(stroke_color))
        .flatten()
        .map(|c| (c, stroke_width.min(40.0)));

    let height = num("height", 120.0).abs().round() as u32;
    let spec = MarkSpec {
        text: get("text").map(|t| t.replace("-nl-", "\n")),
        desc: get("desc").map(|t| t.replace("-nl-", "\n")),
        width: Some(CAPSULE_WIDTH),
        height: Some(height),
        credit,
        ..Default::default()
    };
    let overrides = HeroOverrides {
        title: PlacedText {
            size: num("fontSize", 70.0).abs().clamp(1.0, 400.0) as u32,
            color: colors.font,
            weight: 700,
            x: align_list(get("fontAlign")),
            y: align_list(get("fontAlignY")),
            step_em: 1.2,
        },
        desc: PlacedText {
            size: num("descSize", 20.0).abs().clamp(1.0, 400.0) as u32,
            color: colors.desc,
            weight: 500,
            x: vec![num("descAlign", 50.0)],
            y: vec![num("descAlignY", 60.0)],
            step_em: 1.0,
        },
        family: requested_family(get("fontFamily").unwrap_or(DEFAULT_FAMILY)),
        rotate: num("rotate", 0.0).clamp(-360.0, 360.0),
        stroke,
        text_bg: (get("textBg") == Some("true")).then_some(colors.text_bg),
        flip: get("section") == Some("footer"),
        mirror: get("reversal") == Some("true"),
        motion: motion(get("animation").unwrap_or("fadeIn")),
        silhouette: Silhouette::parse(get("type")),
        paint: colors.paint,
    };
    (spec, overrides)
}

fn resolve_colors(pairs: &HashMap<String, String>, seed: u32) -> Colors {
    let get = |k: &str| pairs.get(k).map(String::as_str).filter(|v| !v.is_empty());
    let font_param = get("fontColor");
    let text_bg_param = get("textBgColor").unwrap_or("000000");
    let custom = get("customColorList").unwrap_or("");

    if let Some((_, color, text, bg)) = THEMES.iter().find(|t| Some(t.0) == get("theme")) {
        return Colors {
            paint: stops(color),
            font: hex_or_black(text),
            desc: hex_or_black(bg),
            text_bg: hex_or_black(bg),
        };
    }
    let color = get("color").unwrap_or("B897FF");
    let row = match color {
        "auto" => Some(pick(PALETTE, custom, seed)),
        "gradient" => Some(pick(GRADIENTS, custom, seed)),
        "timeAuto" => Some(PALETTE[seed as usize % PALETTE.len()]),
        "timeGradient" => Some(GRADIENTS[seed as usize % GRADIENTS.len()]),
        _ => None,
    };
    let (paint, font, text_bg) = match row {
        Some((c, text, bg)) => (stops(c), font_param.unwrap_or(text), bg),
        None if color == "random" => (
            vec![(0.0, format!("{:06X}", seed & 0x00FF_FFFF))],
            font_param.unwrap_or("000000"),
            text_bg_param,
        ),
        None => (stops(color), font_param.unwrap_or("000000"), text_bg_param),
    };
    let font = hex_or_black(font);
    Colors {
        paint,
        desc: font.clone(),
        font,
        text_bg: hex_or_black(text_bg),
    }
}

/// A row chosen by the URL hash, among `customColorList` indices when given
/// (out-of-range indices are dropped; none left means row 0, as upstream).
fn pick(
    rows: &'static [(&'static str, &'static str, &'static str)],
    custom: &str,
    seed: u32,
) -> (&'static str, &'static str, &'static str) {
    if custom.is_empty() {
        return rows[seed as usize % rows.len()];
    }
    let allowed: Vec<usize> = custom
        .split(',')
        .filter_map(|s| s.trim().parse::<usize>().ok())
        .filter(|i| *i < rows.len())
        .collect();
    let index = if allowed.is_empty() {
        0
    } else {
        allowed[seed as usize % allowed.len()]
    };
    rows[index]
}

/// `"0:EEFF00,100:a82da8"` (a gradient, sorted by offset) or one hex.
fn stops(color: &str) -> Vec<(f32, String)> {
    if !color.contains(',') {
        return vec![(0.0, color.to_string())];
    }
    let mut out: Vec<(f32, String)> = color
        .split(',')
        .filter_map(|part| {
            let (offset, hex) = part.split_once(':')?;
            Some((offset.trim().parse::<f32>().ok()?, hex.trim().to_string()))
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

fn hex_or_black(v: &str) -> String {
    normalize_hex_token(v).unwrap_or_else(|| "#000000".into())
}

/// JavaScript `Number(v)` for the plain decimal inputs capsule URLs carry
/// (`NaN` is `None`; an all-space string is 0).
fn js_number(v: &str) -> Option<f32> {
    let t = v.trim();
    if t.is_empty() {
        return Some(0.0);
    }
    t.parse::<f32>().ok().filter(|n| n.is_finite())
}

/// capsule-render `parseToNumberArr`: comma list, non-numbers dropped,
/// nothing left means `[50]`.
fn align_list(v: Option<&str>) -> Vec<f32> {
    let list: Vec<f32> = v
        .unwrap_or("")
        .split(',')
        .filter_map(js_number)
        .take(8)
        .collect();
    if v.is_none() || list.is_empty() {
        vec![50.0]
    } else {
        list
    }
}

/// capsule-render animations onto hero text motion (SMIL, not CSS, so they
/// run inside `<img>`). An unknown name is capsule-render's no-op.
fn motion(name: &str) -> &'static str {
    match name {
        "fadeIn" => "fade",
        "scaleIn" => "rise",
        "blink" => "blink",
        "blinking" => "blinking",
        "twinkling" => "twinkling",
        _ => "none",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(q: &str) -> (MarkSpec, HeroOverrides) {
        let pairs: HashMap<String, String> = q
            .split('&')
            .filter_map(|p| p.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        translate(&pairs, q, false)
    }

    #[test]
    fn defaults_are_capsule_renders() {
        let (spec, ov) = parse("type=wave");
        assert_eq!((spec.width, spec.height), (Some(854), Some(120)));
        assert_eq!((ov.title.size, ov.desc.size), (70, 20));
        assert_eq!(
            (ov.title.x.clone(), ov.title.y.clone()),
            (vec![50.0], vec![50.0])
        );
        assert_eq!(
            (ov.desc.x.clone(), ov.desc.y.clone()),
            (vec![50.0], vec![60.0])
        );
        assert_eq!(ov.paint, vec![(0.0, "B897FF".to_string())]);
        assert_eq!(ov.title.color, "#000000");
        assert_eq!(ov.motion, "fade");
        assert!(ov.stroke.is_none() && ov.text_bg.is_none() && !ov.flip && !ov.mirror);
    }

    #[test]
    fn typography_and_placement_overrides() {
        let (spec, ov) = parse(
            "type=egg&text=a-nl-b&fontSize=90&fontColor=fff&fontAlign=30,70&fontAlignY=38&descSize=14\
             &descAlign=20&descAlignY=80&rotate=-10&stroke=000&strokeWidth=2&section=footer&reversal=true\
             &textBg=true&animation=twinkling&height=300",
        );
        assert_eq!(spec.text.as_deref(), Some("a\nb"));
        assert_eq!(spec.height, Some(300));
        assert_eq!(ov.silhouette, Silhouette::Egg);
        assert_eq!((ov.title.size, ov.title.color.as_str()), (90, "#ffffff"));
        assert_eq!(ov.title.x, vec![30.0, 70.0]);
        assert_eq!(ov.title.y, vec![38.0]);
        assert_eq!((ov.desc.size, ov.desc.x[0], ov.desc.y[0]), (14, 20.0, 80.0));
        assert_eq!(ov.rotate, -10.0);
        assert_eq!(ov.stroke, Some(("#000000".to_string(), 2.0)));
        assert!(ov.flip && ov.mirror);
        assert_eq!(ov.text_bg.as_deref(), Some("#000000"));
        assert_eq!(ov.motion, "twinkling");
    }

    #[test]
    fn colors_follow_capsule_presets_deterministically() {
        let (_, ov) = parse("color=0:EEFF00,100:a82da8");
        assert_eq!(
            ov.paint,
            vec![(0.0, "EEFF00".into()), (100.0, "a82da8".into())]
        );
        let (_, ov) = parse("theme=radical");
        assert_eq!(ov.paint, vec![(0.0, "141321".to_string())]);
        assert_eq!(
            (ov.title.color.as_str(), ov.desc.color.as_str()),
            ("#d83a7c", "#a9fef7")
        );
        let (_, a) = parse("color=gradient&text=x");
        let (_, b) = parse("color=gradient&text=x");
        assert_eq!(a.paint, b.paint, "same URL, same palette");
        assert_eq!(a.title.color, "#f7f5f5");
        let (_, only) = parse("color=gradient&customColorList=2");
        assert_eq!(only.paint, stops("0:43cea2,100:185a9d"));
        let (_, fallback) = parse("color=auto&customColorList=99");
        assert_eq!(fallback.paint, stops(PALETTE[0].0));
        let (_, r) = parse("color=random");
        assert_eq!(r.paint[0].1.len(), 6);
    }

    #[test]
    fn stroke_rules_match_upstream() {
        assert!(parse("text=a").1.stroke.is_none());
        assert_eq!(
            parse("strokeWidth=3").1.stroke,
            Some(("#B897FF".into(), 3.0))
        );
        assert_eq!(parse("stroke=fff").1.stroke, Some(("#ffffff".into(), 1.0)));
        assert!(parse("stroke=none&strokeWidth=4").1.stroke.is_none());
        assert!(
            parse("stroke=%22x").1.stroke.is_none(),
            "invalid color never reaches SVG"
        );
    }

    #[test]
    fn claims_capsule_keys_only() {
        let q = |s: &str| -> HashMap<String, String> {
            s.split('&')
                .filter_map(|p| p.split_once('='))
                .map(|(k, v)| (k.into(), v.into()))
                .collect()
        };
        assert!(claims(&q("type=waving")));
        assert!(claims(&q("text=hi")));
        assert!(!claims(&q("theme=radical")));
        assert!(!claims(&q("")));
    }
}
