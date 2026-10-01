//! Star history card: stars over time as one smooth line with a soft area.
//!
//! Samples come from the stargazer pages (see the live service); the curve is
//! a monotone cubic through them, so it never dips between samples.

use super::card::{frame, n2, CardStyle, PAD_X};
use super::date::{civil_from_days, MONTHS};
use super::format::{card_number, metric};
use super::model::StarHistory;
use crate::capabilities::mark::domain::svg::esc;

pub(crate) const WIDTH: u32 = 720;
pub(crate) const HEIGHT: u32 = 360;

/// Plot rectangle: left, top, right, bottom.
const PLOT: (f32, f32, f32, f32) = (
    PAD_X + 40.0,
    72.0,
    WIDTH as f32 - PAD_X,
    HEIGHT as f32 - 42.0,
);

pub(crate) fn render(h: &StarHistory, style: &CardStyle) -> String {
    let p = &style.palette;
    let title = style.title_or(format!("{}/{}", h.owner, h.repo));
    let total = h.points.last().map_or(0, |p| p.1);
    if h.points.len() < 2 {
        return without_timeline(style, &title, total);
    }
    let pts = h.points.clone();
    let (t0, t1) = (pts[0].0, pts[pts.len() - 1].0.max(pts[0].0 + 1));
    let top = nice_ceiling(pts.iter().map(|p| p.1).max().unwrap_or(0));
    let (l, t, r, b) = PLOT;
    let x = |ts: i64| l + (ts - t0) as f32 / (t1 - t0) as f32 * (r - l);
    let y = |n: u64| b - n as f32 / top as f32 * (b - t);
    let xy: Vec<(f32, f32)> = pts.iter().map(|&(ts, n)| (x(ts), y(n))).collect();

    let mut body = String::new();
    // Total, right-aligned on the title row.
    body.push_str(&format!(
        "<text x=\"{}\" y=\"35\" text-anchor=\"end\" fill=\"{}\" font-size=\"14\" font-weight=\"600\">★ {}</text>",
        n2(WIDTH as f32 - PAD_X),
        p.icon,
        card_number(total)
    ));
    // Gridlines and y labels.
    for i in 0..=4 {
        let v = top * i / 4;
        let gy = y(v);
        body.push_str(&format!(
            "<line x1=\"{}\" y1=\"{gy}\" x2=\"{}\" y2=\"{gy}\" stroke=\"{}\" stroke-opacity=\"{}\"/>\
             <text x=\"{}\" y=\"{}\" text-anchor=\"end\" fill=\"{}\" fill-opacity=\"0.7\" font-size=\"11\">{}</text>",
            n2(l),
            n2(r),
            p.text,
            if i == 0 { "0.28" } else { "0.1" },
            n2(l - 10.0),
            n2(gy + 4.0),
            p.text,
            metric(v),
            gy = n2(gy),
        ));
    }
    // X labels: up to five evenly spaced dates.
    let span_days = (t1 - t0) / 86_400;
    for i in 0..5 {
        let ts = t0 + (t1 - t0) * i / 4;
        let (year, month, _) = civil_from_days(ts.div_euclid(86_400));
        let label = if span_days > 3 * 365 {
            year.to_string()
        } else {
            format!(
                "{} {year}",
                MONTHS[(month as usize).saturating_sub(1).min(11)]
            )
        };
        let anchor = match i {
            0 => "start",
            4 => "end",
            _ => "middle",
        };
        body.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"{anchor}\" fill=\"{}\" fill-opacity=\"0.7\" font-size=\"11\">{}</text>",
            n2(x(ts)),
            n2(b + 22.0),
            p.text,
            esc(&label)
        ));
    }
    let line = monotone_path(&xy);
    let (lx, ly) = xy[xy.len() - 1];
    body.push_str(&format!(
        "<defs><linearGradient id=\"sa\" x1=\"0\" y1=\"0\" x2=\"0\" y2=\"1\">\
         <stop offset=\"0\" stop-color=\"{c}\" stop-opacity=\"0.28\"/>\
         <stop offset=\"1\" stop-color=\"{c}\" stop-opacity=\"0\"/></linearGradient></defs>\
         <path d=\"{line} L{} {} L{} {} Z\" fill=\"url(#sa)\"/>\
         <path d=\"{line}\" fill=\"none\" stroke=\"{c}\" stroke-width=\"2.5\" stroke-linejoin=\"round\" stroke-linecap=\"round\"/>\
         <circle cx=\"{}\" cy=\"{}\" r=\"7\" fill=\"{c}\" fill-opacity=\"0.2\"/>\
         <circle cx=\"{}\" cy=\"{}\" r=\"3.5\" fill=\"{c}\"/>",
        n2(lx),
        n2(b),
        n2(xy[0].0),
        n2(b),
        n2(lx),
        n2(ly),
        n2(lx),
        n2(ly),
        c = p.icon,
    ));
    frame(style, WIDTH, HEIGHT, &title, &body)
}

/// GitHub shares stargazer dates only with readers it allows: without them
/// the card shows today's total and says why there is no curve.
fn without_timeline(style: &CardStyle, title: &str, total: u64) -> String {
    let p = &style.palette;
    let cx = WIDTH as f32 / 2.0;
    let body = format!(
        "<text x=\"{cx}\" y=\"190\" text-anchor=\"middle\" fill=\"{}\" font-size=\"44\" font-weight=\"700\">★ {}</text>\
         <text x=\"{cx}\" y=\"226\" text-anchor=\"middle\" fill=\"{}\" fill-opacity=\"0.7\" font-size=\"13\">\
         Stars on GitHub</text>",
        p.icon,
        card_number(total),
        p.text,
    );
    frame(style, WIDTH, HEIGHT, title, &body)
}

/// The axis top: four gridline steps of a round size (1, 2, 2.5, 5 × 10ⁿ,
/// whole numbers only) that reach `n`.
fn nice_ceiling(n: u64) -> u64 {
    let mut unit = 1u64;
    loop {
        for step in [unit, unit * 2, unit * 5 / 2, unit * 5] {
            if step * 4 >= n.max(1) && (step * 10) % 10 == 0 && step > 0 {
                return step * 4;
            }
        }
        unit *= 10;
    }
}

/// A monotone cubic (Fritsch–Carlson) through points sorted by x.
fn monotone_path(pts: &[(f32, f32)]) -> String {
    let n = pts.len();
    let mut d = format!("M{} {}", n2(pts[0].0), n2(pts[0].1));
    if n < 3 {
        for p in &pts[1..] {
            d.push_str(&format!(" L{} {}", n2(p.0), n2(p.1)));
        }
        return d;
    }
    let slope = |i: usize| {
        let dx = pts[i + 1].0 - pts[i].0;
        if dx.abs() < 1e-6 {
            0.0
        } else {
            (pts[i + 1].1 - pts[i].1) / dx
        }
    };
    let secants: Vec<f32> = (0..n - 1).map(slope).collect();
    let mut m = vec![0.0f32; n];
    m[0] = secants[0];
    m[n - 1] = secants[n - 2];
    for i in 1..n - 1 {
        m[i] = if secants[i - 1] * secants[i] <= 0.0 {
            0.0
        } else {
            (secants[i - 1] + secants[i]) / 2.0
        };
    }
    for i in 0..n - 1 {
        if secants[i] == 0.0 {
            m[i] = 0.0;
            m[i + 1] = 0.0;
            continue;
        }
        let (a, b) = (m[i] / secants[i], m[i + 1] / secants[i]);
        let s = a * a + b * b;
        if s > 9.0 {
            let tau = 3.0 / s.sqrt();
            m[i] = tau * a * secants[i];
            m[i + 1] = tau * b * secants[i];
        }
    }
    for i in 0..n - 1 {
        let dx = (pts[i + 1].0 - pts[i].0) / 3.0;
        d.push_str(&format!(
            " C{} {} {} {} {} {}",
            n2(pts[i].0 + dx),
            n2(pts[i].1 + m[i] * dx),
            n2(pts[i + 1].0 - dx),
            n2(pts[i + 1].1 - m[i + 1] * dx),
            n2(pts[i + 1].0),
            n2(pts[i + 1].1)
        ));
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::live::domain::palette::ColorOverrides;

    #[test]
    fn axis_ceiling_is_round() {
        assert_eq!(nice_ceiling(0), 4);
        assert_eq!(nice_ceiling(2), 4);
        assert_eq!(nice_ceiling(15), 20);
        assert_eq!(nice_ceiling(87), 100);
        assert_eq!(nice_ceiling(1523), 2000);
        assert_eq!(nice_ceiling(19_915), 20_000);
    }

    #[test]
    fn chart_draws_the_curve_and_total() {
        let h = StarHistory {
            owner: "SylphxAI".into(),
            repo: "mark".into(),
            points: vec![
                (1_700_000_000, 1),
                (1_720_000_000, 800),
                (1_790_000_000, 1523),
            ],
        };
        let svg = render(&h, &CardStyle::new(None, &ColorOverrides::default()));
        assert!(svg.contains("SylphxAI/mark"));
        assert!(svg.contains("★ 1,523"));
        assert!(svg.contains(" C"), "smooth curve");
    }
}
