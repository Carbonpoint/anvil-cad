//! 2.5D operations.

use crate::{Move, Tool, Toolpath};
use anvil_math::{DVec2, DVec3};

/// Parameters for a 2.5D contour (profile) cut around a closed polygon.
#[derive(Clone, Debug)]
pub struct ContourParams {
    /// Z of the top of stock.
    pub top_z: f64,
    /// Final depth (below top_z, positive number).
    pub depth: f64,
    /// Depth per pass.
    pub step_down: f64,
    /// Rapid clearance height above top_z.
    pub clearance: f64,
    /// `true` cuts outside the polygon, `false` cuts inside.
    pub outside: bool,
}

/// Offset a simple closed polygon by `d` (positive grows it). This is a
/// vertex-bisector offset. It is exact for convex polygons and adequate for
/// mildly concave ones. Self-intersection cleanup is future work
/// (`cavalier_contours` is the planned replacement).
pub fn offset_polygon(poly: &[DVec2], d: f64) -> Vec<DVec2> {
    let n = poly.len();
    let area: f64 = (0..n).map(|i| poly[i].perp_dot(poly[(i + 1) % n])).sum();
    let sign = if area >= 0.0 { 1.0 } else { -1.0 };
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let p0 = poly[(i + n - 1) % n];
        let p1 = poly[i];
        let p2 = poly[(i + 1) % n];
        let d1 = (p1 - p0).normalize_or_zero();
        let d2 = (p2 - p1).normalize_or_zero();
        // Outward normals for a CCW polygon point to the right of travel.
        let n1 = DVec2::new(d1.y, -d1.x) * sign;
        let n2 = DVec2::new(d2.y, -d2.x) * sign;
        let bis = (n1 + n2).normalize_or_zero();
        let cos_half = bis.dot(n1).max(0.2); // clamp to avoid spikes at sharp corners
        out.push(p1 + bis * (d / cos_half));
    }
    out
}

/// Generate a contour toolpath around `profile` (in the XY plane, mm).
pub fn contour(profile: &[DVec2], tool: &Tool, p: &ContourParams) -> Toolpath {
    let offset = if p.outside { tool.radius() } else { -tool.radius() };
    let path = offset_polygon(profile, offset);
    let mut tp = Toolpath::default();
    tp.push(Move::Comment(format!("Contour, tool {} ({} mm)", tool.name, tool.diameter)));
    tp.push(Move::ToolChange(tool.clone()));
    tp.push(Move::SpindleOn { rpm: tool.rpm, clockwise: true });
    let safe = p.top_z + p.clearance;
    let start = path[0];
    tp.push(Move::Rapid(DVec3::new(start.x, start.y, safe)));
    let passes = (p.depth / p.step_down).ceil().max(1.0) as usize;
    for k in 1..=passes {
        let z = p.top_z - (p.depth * k as f64 / passes as f64);
        tp.push(Move::Linear { to: DVec3::new(start.x, start.y, z), feed: tool.plunge });
        for q in path.iter().skip(1).chain(std::iter::once(&start)) {
            tp.push(Move::Linear { to: DVec3::new(q.x, q.y, z), feed: tool.feed });
        }
    }
    tp.push(Move::Rapid(DVec3::new(start.x, start.y, safe)));
    tp.push(Move::SpindleOff);
    tp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_square_grows_by_d() {
        let sq = [DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0), DVec2::new(10.0, 10.0), DVec2::new(0.0, 10.0)];
        let o = offset_polygon(&sq, 1.0);
        assert!((o[0] - DVec2::new(-1.0, -1.0)).length() < 1e-9, "{o:?}");
        assert!((o[2] - DVec2::new(11.0, 11.0)).length() < 1e-9, "{o:?}");
    }

    #[test]
    fn contour_has_expected_pass_count() {
        let sq = [DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0), DVec2::new(10.0, 10.0), DVec2::new(0.0, 10.0)];
        let tool =
            Tool { number: 1, name: "6mm endmill".into(), diameter: 6.0, rpm: 12000.0, feed: 800.0, plunge: 200.0 };
        let tp = contour(
            &sq,
            &tool,
            &ContourParams { top_z: 0.0, depth: 5.0, step_down: 2.0, clearance: 5.0, outside: true },
        );
        let plunges = tp.moves.iter().filter(|m| matches!(m, Move::Linear { feed, .. } if *feed == 200.0)).count();
        assert_eq!(plunges, 3);
        assert!(tp.cut_length() > 4.0 * 16.0 * 3.0);
    }
}
