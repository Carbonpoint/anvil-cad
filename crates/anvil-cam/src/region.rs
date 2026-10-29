//! Offsetting regions (an outer loop with islands) through
//! `cavalier_contours`, which keeps loops apart where they would cross,
//! splits a region that pinches off, and rounds convex corners.

use anvil_math::DVec2;
use cavalier_contours::polyline::{PlineSource, PlineSourceMut, Polyline};
use cavalier_contours::shape_algorithms::{Shape, ShapeOffsetOptions};

/// Largest distance a flattened arc strays from the arc, in mm.
const TOL: f64 = 0.01;

fn to_pline(poly: &[DVec2], ccw: bool) -> Polyline<f64> {
    let area: f64 = (0..poly.len()).map(|i| poly[i].perp_dot(poly[(i + 1) % poly.len()])).sum();
    let mut pts: Vec<DVec2> = poly.to_vec();
    if (area > 0.0) != ccw {
        pts.reverse();
    }
    let mut pl = Polyline::new_closed();
    for p in pts {
        pl.add(p.x, p.y, 0.0);
    }
    pl
}

/// The points of a closed polyline, with arcs turned into short lines.
fn flatten(pl: &Polyline<f64>) -> Vec<DVec2> {
    let n = pl.vertex_count();
    let mut out = Vec::new();
    for i in 0..n {
        let (Some(v), Some(w)) = (pl.get(i), pl.get((i + 1) % n)) else { continue };
        let (a, b) = (DVec2::new(v.x, v.y), DVec2::new(w.x, w.y));
        out.push(a);
        if v.bulge.abs() < 1e-12 {
            continue;
        }
        // An arc from a to b turning 4 atan(bulge).
        let sweep = 4.0 * v.bulge.atan();
        let chord = (b - a).length();
        if chord < 1e-12 {
            continue;
        }
        let r = chord / (2.0 * (sweep / 2.0).sin().abs());
        let mid = (a + b) * 0.5;
        let perp = DVec2::new(-(b - a).y, (b - a).x) / chord;
        let h = r * (sweep / 2.0).cos();
        let centre = mid + perp * h * sweep.signum();
        let steps = ((sweep.abs() / (2.0 * (1.0 - TOL / r.max(TOL * 2.0)).acos())).ceil() as usize).clamp(1, 128);
        let a0 = (a - centre).y.atan2((a - centre).x);
        for k in 1..steps {
            let t = a0 + sweep * k as f64 / steps as f64;
            out.push(centre + DVec2::new(t.cos(), t.sin()) * r);
        }
    }
    out
}

/// The region `outer` less `islands`, offset by `d`: positive shrinks the
/// material region (moves every loop into it), negative grows it. Returns
/// the loops of the result; an outer loop runs counter-clockwise, an
/// island loop clockwise. Empty when the region has vanished.
pub fn offset_region(outer: &[DVec2], islands: &[Vec<DVec2>], d: f64) -> Vec<Vec<DVec2>> {
    if outer.len() < 3 {
        return Vec::new();
    }
    let plines = std::iter::once(to_pline(outer, true))
        .chain(islands.iter().filter(|h| h.len() >= 3).map(|h| to_pline(h, false)));
    let shape = Shape::from_plines(plines);
    let out = shape.parallel_offset(d, &ShapeOffsetOptions::default());
    out.ccw_plines.iter().chain(out.cw_plines.iter()).map(|ip| flatten(&ip.polyline)).filter(|l| l.len() >= 3).collect()
}

/// Signed area of a loop: positive counter-clockwise.
pub fn area(poly: &[DVec2]) -> f64 {
    0.5 * (0..poly.len()).map(|i| poly[i].perp_dot(poly[(i + 1) % poly.len()])).sum::<f64>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(s: f64) -> Vec<DVec2> {
        vec![DVec2::new(0.0, 0.0), DVec2::new(s, 0.0), DVec2::new(s, s), DVec2::new(0.0, s)]
    }

    #[test]
    fn a_positive_offset_shrinks_the_region() {
        let loops = offset_region(&square(10.0), &[], 1.0);
        assert_eq!(loops.len(), 1);
        assert!((area(&loops[0]) - 64.0).abs() < 1e-6, "{}", area(&loops[0]));
    }

    #[test]
    fn a_negative_offset_grows_with_round_corners() {
        let loops = offset_region(&square(10.0), &[], -1.0);
        let want = 100.0 + 4.0 * 10.0 + std::f64::consts::PI;
        // Arcs become chords within 0.01 mm, a little under the true area.
        assert!((area(&loops[0]) - want).abs() < 0.05, "{} vs {want}", area(&loops[0]));
    }

    #[test]
    fn an_island_grows_as_the_region_shrinks() {
        let hole: Vec<DVec2> = square(4.0).iter().map(|p| *p + DVec2::splat(8.0)).collect();
        let loops = offset_region(&square(20.0), &[hole], 1.0);
        assert_eq!(loops.len(), 2, "outer and island");
        let total: f64 = loops.iter().map(|l| area(l)).sum();
        // The 4 mm island grown by 1 mm: 16 + 4 x 4 x 1 + pi.
        let want = 18.0 * 18.0 - (16.0 + 16.0 + std::f64::consts::PI);
        assert!((total - want).abs() < 0.05, "{total} vs {want}");
    }
}
