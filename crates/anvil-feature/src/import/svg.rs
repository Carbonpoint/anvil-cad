//! SVG import into a sketch.
//!
//! Reads `path`, `rect` (with rounded corners), `circle`, `ellipse`,
//! `line`, `polyline` and `polygon`, with `transform` on shapes and on
//! groups. Curves and arcs become short lines within about 0.05 mm; a
//! circle that its transform keeps round stays a circle. Sizes come from
//! the root `width` and `viewBox` (px are 1/96 inch). SVG's y axis points
//! down, so y is flipped: the drawing is upright in the sketch. Shapes in
//! `defs`, `clipPath`, `mask`, `symbol` and `pattern` are not drawn.

use super::xml::Tags;
use anvil_math::DVec2;
use anvil_sketch::Sketch;

/// Largest distance a flattened curve may stray from the curve, in mm.
const TOL: f64 = 0.05;

/// A 2D affine transform `[a, b, c, d, e, f]`: x' = a x + c y + e,
/// y' = b x + d y + f, as SVG writes it.
type Xf = [f64; 6];

const IDENTITY: Xf = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

fn apply(m: &Xf, p: DVec2) -> DVec2 {
    DVec2::new(m[0] * p.x + m[2] * p.y + m[4], m[1] * p.x + m[3] * p.y + m[5])
}

/// `inner` first, then `outer`.
fn compose(outer: &Xf, inner: &Xf) -> Xf {
    [
        outer[0] * inner[0] + outer[2] * inner[1],
        outer[1] * inner[0] + outer[3] * inner[1],
        outer[0] * inner[2] + outer[2] * inner[3],
        outer[1] * inner[2] + outer[3] * inner[3],
        outer[0] * inner[4] + outer[2] * inner[5] + outer[4],
        outer[1] * inner[4] + outer[3] * inner[5] + outer[5],
    ]
}

/// Numbers in an SVG attribute: "1.5-2e3,.5.5" is 1.5, -2000, 0.5, 0.5.
fn numbers(s: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_digit() || c == b'.' || c == b'-' || c == b'+' {
            let start = i;
            i += 1;
            let mut dot = c == b'.';
            while i < b.len() {
                let d = b[i];
                if d.is_ascii_digit() {
                    i += 1;
                } else if d == b'.' && !dot {
                    dot = true;
                    i += 1;
                } else if (d == b'e' || d == b'E') && i + 1 < b.len() {
                    i += 1;
                    if b[i] == b'-' || b[i] == b'+' {
                        i += 1;
                    }
                } else {
                    break;
                }
            }
            if let Ok(x) = s[start..i].parse::<f64>() {
                out.push(x);
            }
        } else {
            i += 1;
        }
    }
    out
}

fn parse_transform(s: &str) -> Xf {
    let mut m = IDENTITY;
    let mut rest = s;
    while let Some(open) = rest.find('(') {
        let name = rest[..open].trim().trim_start_matches(',').trim();
        let Some(close) = rest[open..].find(')') else { break };
        let v = numbers(&rest[open + 1..open + close]);
        let g = |k: usize, d: f64| v.get(k).copied().unwrap_or(d);
        let t: Xf = match name {
            "matrix" if v.len() == 6 => [v[0], v[1], v[2], v[3], v[4], v[5]],
            "translate" => [1.0, 0.0, 0.0, 1.0, g(0, 0.0), g(1, 0.0)],
            "scale" => {
                let sx = g(0, 1.0);
                [sx, 0.0, 0.0, g(1, sx), 0.0, 0.0]
            }
            "rotate" => {
                let (s, c) = g(0, 0.0).to_radians().sin_cos();
                let (cx, cy) = (g(1, 0.0), g(2, 0.0));
                // Turn about (cx, cy).
                [c, s, -s, c, cx - c * cx + s * cy, cy - s * cx - c * cy]
            }
            "skewX" => [1.0, 0.0, g(0, 0.0).to_radians().tan(), 1.0, 0.0, 0.0],
            "skewY" => [1.0, g(0, 0.0).to_radians().tan(), 0.0, 1.0, 0.0, 0.0],
            _ => IDENTITY,
        };
        m = compose(&m, &t);
        rest = &rest[open + close + 1..];
    }
    m
}

/// A length with its unit, in mm. Plain numbers and px are 1/96 inch.
fn length_mm(s: &str) -> Option<f64> {
    let s = s.trim();
    let split = s.find(|c: char| c.is_ascii_alphabetic() || c == '%').unwrap_or(s.len());
    let v: f64 = s[..split].trim().parse().ok()?;
    let k = match s[split..].trim() {
        "" | "px" => 25.4 / 96.0,
        "mm" => 1.0,
        "cm" => 10.0,
        "in" => 25.4,
        "pt" => 25.4 / 72.0,
        "pc" => 25.4 / 6.0,
        _ => return None,
    };
    Some(v * k)
}

/// Collects polylines in sketch coordinates.
struct Out<'a> {
    sk: &'a mut Sketch,
    count: usize,
}

impl Out<'_> {
    /// One polyline; a closed one ends on its first point.
    fn polyline(&mut self, pts: &[DVec2], closed: bool) {
        let mut pts: Vec<DVec2> = pts.to_vec();
        pts.dedup_by(|a, b| (*a - *b).length() < 1e-9);
        let closed = closed || (pts.len() > 2 && (pts[0] - pts[pts.len() - 1]).length() < 1e-6);
        if closed && pts.len() > 1 && (pts[0] - pts[pts.len() - 1]).length() < 1e-6 {
            pts.pop();
        }
        if pts.len() < 2 {
            return;
        }
        let ids: Vec<_> = pts.iter().map(|p| self.sk.add_point(p.x, p.y)).collect();
        for w in ids.windows(2) {
            self.sk.add_line(w[0], w[1]);
        }
        if closed && ids.len() > 2 {
            self.sk.add_line(ids[ids.len() - 1], ids[0]);
        }
        self.count += 1;
    }
}

/// Segments for a curve of about `len` mm.
fn steps(len: f64) -> usize {
    ((len / (8.0 * TOL).sqrt()).ceil() as usize).clamp(2, 256)
}

/// Points along an elliptical arc in SVG endpoint form, after `from`.
#[allow(clippy::too_many_arguments)]
fn arc(from: DVec2, rx: f64, ry: f64, rot_deg: f64, large: bool, sweep: bool, to: DVec2, scale: f64) -> Vec<DVec2> {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 1e-12 || ry < 1e-12 || (from - to).length() < 1e-12 {
        return vec![to];
    }
    // SVG 1.1 appendix F.6.5: endpoint to centre form.
    let (s, c) = rot_deg.to_radians().sin_cos();
    let d = (from - to) * 0.5;
    let p = DVec2::new(c * d.x + s * d.y, -s * d.x + c * d.y);
    let lambda = (p.x * p.x) / (rx * rx) + (p.y * p.y) / (ry * ry);
    if lambda > 1.0 {
        rx *= lambda.sqrt();
        ry *= lambda.sqrt();
    }
    let num = (rx * rx * ry * ry - rx * rx * p.y * p.y - ry * ry * p.x * p.x).max(0.0);
    let den = rx * rx * p.y * p.y + ry * ry * p.x * p.x;
    let mut k = if den > 0.0 { (num / den).sqrt() } else { 0.0 };
    if large == sweep {
        k = -k;
    }
    let cp = DVec2::new(k * rx * p.y / ry, -k * ry * p.x / rx);
    let mid = (from + to) * 0.5;
    let centre = DVec2::new(c * cp.x - s * cp.y + mid.x, s * cp.x + c * cp.y + mid.y);
    let angle = |u: DVec2, v: DVec2| u.x.mul_add(v.y, -u.y * v.x).atan2(u.dot(v));
    let u = DVec2::new((p.x - cp.x) / rx, (p.y - cp.y) / ry);
    let v = DVec2::new((-p.x - cp.x) / rx, (-p.y - cp.y) / ry);
    let t0 = angle(DVec2::X, u);
    let mut dt = angle(u, v);
    if !sweep && dt > 0.0 {
        dt -= std::f64::consts::TAU;
    } else if sweep && dt < 0.0 {
        dt += std::f64::consts::TAU;
    }
    let n = steps(rx.max(ry) * scale * dt.abs());
    (1..=n)
        .map(|i| {
            if i == n {
                return to;
            }
            let t = t0 + dt * i as f64 / n as f64;
            let q = DVec2::new(rx * t.cos(), ry * t.sin());
            centre + DVec2::new(c * q.x - s * q.y, s * q.x + c * q.y)
        })
        .collect()
}

/// Subpaths of an SVG path `d`, as point lists in user units, each with
/// a flag for closed.
fn path(d: &str, scale: f64) -> Vec<(Vec<DVec2>, bool)> {
    let mut out: Vec<(Vec<DVec2>, bool)> = Vec::new();
    let mut cur: Vec<DVec2> = Vec::new();
    let mut pos = DVec2::ZERO;
    let mut start = DVec2::ZERO;
    let mut last_ctrl: Option<(char, DVec2)> = None;
    // Split into commands with their numbers.
    let mut cmds: Vec<(char, Vec<f64>)> = Vec::new();
    let mut from = 0;
    for (i, ch) in d.char_indices() {
        if ch.is_ascii_alphabetic() && ch != 'e' && ch != 'E' {
            if let Some(last) = cmds.last_mut() {
                last.1 = numbers(&d[from..i]);
            }
            cmds.push((ch, Vec::new()));
            from = i + ch.len_utf8();
        }
    }
    if let Some(last) = cmds.last_mut() {
        last.1 = numbers(&d[from..]);
    }
    let flush = |cur: &mut Vec<DVec2>, out: &mut Vec<(Vec<DVec2>, bool)>, closed: bool| {
        if cur.len() > 1 {
            out.push((std::mem::take(cur), closed));
        } else {
            cur.clear();
        }
    };
    for (cmd, v) in cmds {
        let rel = cmd.is_ascii_lowercase();
        let base = |pos: DVec2| if rel { pos } else { DVec2::ZERO };
        let upper = cmd.to_ascii_uppercase();
        let arity = match upper {
            'M' | 'L' | 'T' => 2,
            'H' | 'V' => 1,
            'C' => 6,
            'S' | 'Q' => 4,
            'A' => 7,
            _ => 0,
        };
        if upper == 'Z' {
            flush(&mut cur, &mut out, true);
            pos = start;
            last_ctrl = None;
            continue;
        }
        if arity == 0 || v.len() < arity {
            continue;
        }
        for (k, g) in v.chunks_exact(arity).enumerate() {
            let b = base(pos);
            let p2 = |i: usize| b + DVec2::new(g[i], g[i + 1]);
            match upper {
                'M' if k == 0 => {
                    flush(&mut cur, &mut out, false);
                    pos = p2(0);
                    start = pos;
                    cur.push(pos);
                    last_ctrl = None;
                }
                // Pairs after a move are lines.
                'M' | 'L' => {
                    pos = p2(0);
                    cur.push(pos);
                    last_ctrl = None;
                }
                'H' => {
                    pos = DVec2::new(if rel { pos.x + g[0] } else { g[0] }, pos.y);
                    cur.push(pos);
                    last_ctrl = None;
                }
                'V' => {
                    pos = DVec2::new(pos.x, if rel { pos.y + g[0] } else { g[0] });
                    cur.push(pos);
                    last_ctrl = None;
                }
                'C' | 'S' => {
                    let (c1, c2, end) = if upper == 'C' {
                        (p2(0), p2(2), p2(4))
                    } else {
                        let c1 = match last_ctrl {
                            Some(('C', c)) => pos * 2.0 - c,
                            _ => pos,
                        };
                        (c1, p2(0), p2(2))
                    };
                    if cur.is_empty() {
                        cur.push(pos);
                    }
                    let len = (c1 - pos).length() + (c2 - c1).length() + (end - c2).length();
                    let n = steps(len * scale);
                    for i in 1..=n {
                        let t = i as f64 / n as f64;
                        let u = 1.0 - t;
                        cur.push(
                            pos * (u * u * u) + c1 * (3.0 * u * u * t) + c2 * (3.0 * u * t * t) + end * (t * t * t),
                        );
                    }
                    last_ctrl = Some(('C', c2));
                    pos = end;
                }
                'Q' | 'T' => {
                    let (c1, end) = if upper == 'Q' {
                        (p2(0), p2(2))
                    } else {
                        let c1 = match last_ctrl {
                            Some(('Q', c)) => pos * 2.0 - c,
                            _ => pos,
                        };
                        (c1, p2(0))
                    };
                    if cur.is_empty() {
                        cur.push(pos);
                    }
                    let n = steps(((c1 - pos).length() + (end - c1).length()) * scale);
                    for i in 1..=n {
                        let t = i as f64 / n as f64;
                        let u = 1.0 - t;
                        cur.push(pos * (u * u) + c1 * (2.0 * u * t) + end * (t * t));
                    }
                    last_ctrl = Some(('Q', c1));
                    pos = end;
                }
                'A' => {
                    let end = b + DVec2::new(g[5], g[6]);
                    if cur.is_empty() {
                        cur.push(pos);
                    }
                    cur.extend(arc(pos, g[0], g[1], g[2], g[3] != 0.0, g[4] != 0.0, end, scale));
                    pos = end;
                    last_ctrl = None;
                }
                _ => {}
            }
        }
    }
    flush(&mut cur, &mut out, false);
    out
}

/// Add the drawing in `text` to the sketch. Returns the number of shapes
/// added.
pub fn import(text: &str, sk: &mut Sketch) -> Result<usize, String> {
    let mut out = Out { sk, count: 0 };
    let mut stack: Vec<(Xf, bool)> = Vec::new();
    let mut root: Option<Xf> = None;
    for tag in Tags::new(text) {
        let hidden = stack.iter().any(|(_, h)| *h);
        let top = stack.last().map(|(m, _)| *m).unwrap_or(IDENTITY);
        let own = tag.attr("transform").map(parse_transform).unwrap_or(IDENTITY);
        let m = compose(&top, &own);
        let num = |k: &str| tag.attr(k).and_then(|v| numbers(v).first().copied()).unwrap_or(0.0);
        match (tag.name, tag.closing) {
            ("svg", false) if root.is_none() => {
                // User units to mm, and y flipped.
                let vb = tag.attr("viewBox").map(numbers).filter(|v| v.len() == 4);
                let w = tag.attr("width").and_then(length_mm);
                let h = tag.attr("height").and_then(length_mm);
                let (sx, sy, ox, oy) = match (vb, w, h) {
                    (Some(v), Some(w), Some(h)) if v[2] > 0.0 && v[3] > 0.0 => (w / v[2], h / v[3], v[0], v[1]),
                    (Some(v), Some(w), None) if v[2] > 0.0 => (w / v[2], w / v[2], v[0], v[1]),
                    (Some(v), _, _) => (25.4 / 96.0, 25.4 / 96.0, v[0], v[1]),
                    _ => (25.4 / 96.0, 25.4 / 96.0, 0.0, 0.0),
                };
                let r = [sx, 0.0, 0.0, -sy, -ox * sx, oy * sy];
                root = Some(r);
                if !tag.empty {
                    stack.push((compose(&r, &own), false));
                }
            }
            ("g" | "a" | "defs" | "clipPath" | "mask" | "symbol" | "pattern" | "marker" | "svg", false) => {
                if !tag.empty {
                    let hides = matches!(tag.name, "defs" | "clipPath" | "mask" | "symbol" | "pattern" | "marker");
                    stack.push((m, hides));
                }
            }
            ("g" | "a" | "defs" | "clipPath" | "mask" | "symbol" | "pattern" | "marker" | "svg", true) => {
                stack.pop();
            }
            (_, true) => {}
            _ if hidden => {}
            ("path", false) => {
                let scale = (m[0] * m[3] - m[1] * m[2]).abs().sqrt();
                for (pts, closed) in path(tag.attr("d").unwrap_or(""), scale) {
                    let pts: Vec<DVec2> = pts.iter().map(|p| apply(&m, *p)).collect();
                    out.polyline(&pts, closed);
                }
            }
            ("rect", false) => {
                let (x, y, w, h) = (num("x"), num("y"), num("width"), num("height"));
                if w <= 0.0 || h <= 0.0 {
                    continue;
                }
                let rx = tag.attr("rx").and_then(|v| numbers(v).first().copied());
                let ry = tag.attr("ry").and_then(|v| numbers(v).first().copied());
                let (rx, ry) = match (rx, ry) {
                    (Some(a), Some(b)) => (a, b),
                    (Some(a), None) | (None, Some(a)) => (a, a),
                    _ => (0.0, 0.0),
                };
                let (rx, ry) = (rx.clamp(0.0, w / 2.0), ry.clamp(0.0, h / 2.0));
                let d = if rx > 0.0 && ry > 0.0 {
                    format!(
                        "M{} {} H{} A{rx} {ry} 0 0 1 {} {} V{} A{rx} {ry} 0 0 1 {} {} H{} A{rx} {ry} 0 0 1 {} {} V{} A{rx} {ry} 0 0 1 {} {} Z",
                        x + rx, y, x + w - rx, x + w, y + ry, y + h - ry, x + w - rx, y + h, x + rx, x, y + h - ry, y + ry, x + rx, y
                    )
                } else {
                    format!("M{x} {y} H{} V{} H{x} Z", x + w, y + h)
                };
                let scale = (m[0] * m[3] - m[1] * m[2]).abs().sqrt();
                for (pts, closed) in path(&d, scale) {
                    let pts: Vec<DVec2> = pts.iter().map(|p| apply(&m, *p)).collect();
                    out.polyline(&pts, closed);
                }
            }
            ("circle" | "ellipse", false) => {
                let (cx, cy) = (num("cx"), num("cy"));
                let (rx, ry) = if tag.name == "circle" { (num("r"), num("r")) } else { (num("rx"), num("ry")) };
                if rx <= 0.0 || ry <= 0.0 {
                    continue;
                }
                // A circle that stays round: the linear part is a turn
                // and a uniform scale.
                let (a, b, c, d) = (m[0], m[1], m[2], m[3]);
                let round = (a - d).abs() < 1e-9 * a.abs().max(1.0) && (b + c).abs() < 1e-9
                    || (a + d).abs() < 1e-9 * a.abs().max(1.0) && (b - c).abs() < 1e-9;
                if (rx - ry).abs() < 1e-12 && round {
                    let centre = apply(&m, DVec2::new(cx, cy));
                    let k = (a * a + b * b).sqrt();
                    let id = out.sk.add_point(centre.x, centre.y);
                    out.sk.add_circle(id, rx * k);
                    out.count += 1;
                } else {
                    let outline = format!(
                        "M{} {cy} A{rx} {ry} 0 1 1 {} {cy} A{rx} {ry} 0 1 1 {} {cy} Z",
                        cx + rx,
                        cx - rx,
                        cx + rx
                    );
                    let scale = (a * d - b * c).abs().sqrt().max(1e-9);
                    for (pts, closed) in path(&outline, scale) {
                        let pts: Vec<DVec2> = pts.iter().map(|p| apply(&m, *p)).collect();
                        out.polyline(&pts, closed);
                    }
                }
            }
            ("line", false) => {
                let pts = [DVec2::new(num("x1"), num("y1")), DVec2::new(num("x2"), num("y2"))];
                let pts: Vec<DVec2> = pts.iter().map(|p| apply(&m, *p)).collect();
                out.polyline(&pts, false);
            }
            ("polyline" | "polygon", false) => {
                let v = numbers(tag.attr("points").unwrap_or(""));
                let pts: Vec<DVec2> = v.as_chunks::<2>().0.iter().map(|p| apply(&m, DVec2::new(p[0], p[1]))).collect();
                out.polyline(&pts, tag.name == "polygon");
            }
            _ => {}
        }
    }
    if root.is_none() {
        return Err("not an SVG file: there is no <svg> element".into());
    }
    Ok(out.count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sketch(svg: &str) -> Sketch {
        let mut sk = Sketch::new(anvil_math::Plane::XY);
        import(svg, &mut sk).unwrap();
        sk
    }

    fn profile_areas(sk: &Sketch) -> Vec<f64> {
        let mut a: Vec<f64> = sk.profiles().iter().map(|p| p.signed_area().abs()).collect();
        a.sort_by(|x, y| y.total_cmp(x));
        a
    }

    #[test]
    fn a_rect_in_millimetres_is_a_closed_profile() {
        let sk = sketch(
            r#"<svg width="100mm" height="50mm" viewBox="0 0 100 50"><rect x="10" y="5" width="30" height="20"/></svg>"#,
        );
        let a = profile_areas(&sk);
        assert_eq!(a.len(), 1, "{a:?}");
        assert!((a[0] - 600.0).abs() < 1e-9);
    }

    #[test]
    fn pixels_are_a_96th_of_an_inch_and_y_is_flipped() {
        let sk = sketch(r#"<svg><polygon points="0,0 96,0 96,96 0,96"/></svg>"#);
        let a = profile_areas(&sk);
        assert!((a[0] - 25.4 * 25.4).abs() < 1e-6, "{a:?}");
        let ys: Vec<f64> = sk
            .entities
            .values()
            .filter_map(|e| match e {
                anvil_sketch::Entity::Point { pos, .. } => Some(pos.y),
                _ => None,
            })
            .collect();
        assert!(ys.iter().all(|y| *y <= 1e-9), "y must be flipped: {ys:?}");
    }

    #[test]
    fn a_path_with_an_arc_and_a_hole_gives_two_loops() {
        // A 40 x 40 square with a round hole of radius 10 drawn by arcs.
        let sk = sketch(
            r#"<svg width="40mm" height="40mm" viewBox="0 0 40 40">
            <g transform="translate(0,0)">
            <path d="M0 0 H40 V40 H0 Z M30 20 A10 10 0 0 1 10 20 A10 10 0 0 1 30 20 Z"/>
            </g></svg>"#,
        );
        let a = profile_areas(&sk);
        assert_eq!(a.len(), 2, "{a:?}");
        assert!((a[0] - 1600.0).abs() < 1e-6);
        let hole = std::f64::consts::PI * 100.0;
        assert!((a[1] - hole).abs() < 0.01 * hole, "hole {} vs {hole}", a[1]);
    }

    #[test]
    fn a_circle_stays_a_circle_and_curves_are_smooth() {
        let sk = sketch(
            r#"<svg width="100mm" height="100mm" viewBox="0 0 100 100">
            <circle cx="50" cy="50" r="10" transform="rotate(30 50 50)"/>
            <path d="M0 0 C 0 50, 100 50, 100 0 Z"/>
            </svg>"#,
        );
        let a = profile_areas(&sk);
        assert_eq!(a.len(), 2);
        // The circle is kept as a circle; its profile is a fine polygon.
        let circles = sk.entities.values().filter(|e| matches!(e, anvil_sketch::Entity::Circle { .. })).count();
        assert_eq!(circles, 1);
        assert!(a.iter().any(|x| (x - std::f64::consts::PI * 100.0).abs() < 0.01 * 314.2), "{a:?}");
        // The cubic: y = 150 t (1 - t), x = 100 (3 t^2 - 2 t^3), so the
        // area is 90000 times the integral of t^2 (1 - t)^2, which is 3000.
        assert!(a.iter().any(|x| (x - 3000.0).abs() < 0.5), "{a:?}");
    }

    #[test]
    fn shapes_in_defs_are_not_drawn() {
        let sk =
            sketch(r#"<svg><defs><rect width="10" height="10"/></defs><rect x="20" width="10" height="10"/></svg>"#);
        assert_eq!(profile_areas(&sk).len(), 1);
    }

    #[test]
    fn a_file_without_svg_is_an_error() {
        let mut sk = Sketch::new(anvil_math::Plane::XY);
        assert!(import("<html></html>", &mut sk).is_err());
    }
}
