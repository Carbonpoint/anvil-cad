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

/// Offset a closed polygon by `d` (positive grows it). Loops that would
/// cross are kept apart and convex corners are rounded; when a shrinking
/// polygon splits, the largest piece is returned. Empty when it vanishes.
pub fn offset_polygon(poly: &[DVec2], d: f64) -> Vec<DVec2> {
    crate::region::offset_region(poly, &[], -d)
        .into_iter()
        .filter(|l| crate::region::area(l) > 0.0)
        .max_by(|a, b| crate::region::area(a).total_cmp(&crate::region::area(b)))
        .unwrap_or_default()
}

/// Generate a contour toolpath around `profile` (in the XY plane, mm).
pub fn contour(profile: &[DVec2], tool: &Tool, p: &ContourParams) -> Toolpath {
    let offset = if p.outside { tool.radius() } else { -tool.radius() };
    let path = offset_polygon(profile, offset);
    let mut tp = Toolpath::default();
    if path.is_empty() {
        tp.push(Move::Comment("Contour: the tool does not fit inside the profile".into()));
        return tp;
    }
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
        // Grown by 1 with round corners: 144 - 4 + pi.
        let a = crate::region::area(&o);
        assert!((a - (140.0 + std::f64::consts::PI)).abs() < 0.05, "{a}");
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
        // Three passes round the square 3 mm out, corners rounded, plus
        // the plunges: 5 mm from the clearance to the first pass depth of
        // 5 / 3, then 5 / 3 twice more.
        let lap = 40.0 + std::f64::consts::TAU * 3.0;
        assert!((tp.cut_length() - (3.0 * lap + 10.0)).abs() < 0.2, "{}", tp.cut_length());
    }
}

/// Parameters for a pocket: clear the inside of a closed loop, leaving
/// islands standing.
#[derive(Clone, Debug)]
pub struct PocketParams {
    pub top_z: f64,
    /// Final depth below `top_z` (positive).
    pub depth: f64,
    pub step_down: f64,
    /// Distance between neighbouring rings, as a fraction of the tool
    /// diameter (0.4 is common).
    pub stepover: f64,
    pub clearance: f64,
}

/// Contour-parallel pocket: rings offset inward from the wall by the tool
/// radius, then by the stepover, until nothing is left. Each depth pass
/// cuts from the innermost ring outward, so every ring starts in cleared
/// space; the tool lifts to the clearance height between rings, which is
/// safe over islands.
pub fn pocket(outer: &[DVec2], islands: &[Vec<DVec2>], tool: &Tool, p: &PocketParams) -> Toolpath {
    let r = tool.radius();
    let step = (p.stepover.clamp(0.05, 0.95) * tool.diameter).max(1e-3);
    // Rings, from the wall inward.
    let mut rings: Vec<Vec<DVec2>> = Vec::new();
    let mut d = r;
    for _ in 0..10_000 {
        let loops = crate::region::offset_region(outer, islands, d);
        if loops.is_empty() {
            break;
        }
        rings.extend(loops);
        d += step;
    }
    let mut tp = Toolpath::default();
    tp.push(Move::Comment(format!("Pocket, tool {} ({} mm), {} rings", tool.name, tool.diameter, rings.len())));
    if rings.is_empty() {
        tp.push(Move::Comment("Pocket: the tool does not fit inside the pocket".into()));
        return tp;
    }
    tp.push(Move::ToolChange(tool.clone()));
    tp.push(Move::SpindleOn { rpm: tool.rpm, clockwise: true });
    let safe = p.top_z + p.clearance;
    let passes = (p.depth / p.step_down).ceil().max(1.0) as usize;
    for k in 1..=passes {
        let z = p.top_z - p.depth * k as f64 / passes as f64;
        for ring in rings.iter().rev() {
            let start = ring[0];
            tp.push(Move::Rapid(DVec3::new(start.x, start.y, safe)));
            tp.push(Move::Linear { to: DVec3::new(start.x, start.y, z), feed: tool.plunge });
            for q in ring.iter().skip(1).chain(std::iter::once(&start)) {
                tp.push(Move::Linear { to: DVec3::new(q.x, q.y, z), feed: tool.feed });
            }
            tp.push(Move::Rapid(DVec3::new(start.x, start.y, safe)));
        }
    }
    tp.push(Move::SpindleOff);
    tp
}

/// Parameters for drilling.
#[derive(Clone, Debug)]
pub struct DrillParams {
    pub top_z: f64,
    /// Hole depth below `top_z` (positive).
    pub depth: f64,
    /// Peck depth; 0 drills in one go.
    pub peck: f64,
    /// Height above `top_z` the drill retracts to between pecks and holes.
    pub retract: f64,
    pub clearance: f64,
}

/// Drill every point: one canned cycle per hole, which a post writes as
/// G81 or G83 or spells out as moves for a control without cycles.
pub fn drill(points: &[DVec2], tool: &Tool, p: &DrillParams) -> Toolpath {
    let mut tp = Toolpath::default();
    tp.push(Move::Comment(format!("Drill {} holes, tool {} ({} mm)", points.len(), tool.name, tool.diameter)));
    if points.is_empty() {
        return tp;
    }
    tp.push(Move::ToolChange(tool.clone()));
    tp.push(Move::SpindleOn { rpm: tool.rpm, clockwise: true });
    let safe = p.top_z + p.clearance;
    tp.push(Move::Rapid(DVec3::new(points[0].x, points[0].y, safe)));
    for q in points {
        tp.push(Move::Drill {
            at: *q,
            bottom_z: p.top_z - p.depth,
            retract_z: p.top_z + p.retract,
            peck: p.peck.max(0.0),
            feed: tool.plunge,
        });
    }
    tp.push(Move::Rapid(DVec3::new(points[points.len() - 1].x, points[points.len() - 1].y, safe)));
    tp.push(Move::SpindleOff);
    tp
}

#[cfg(test)]
mod pocket_tests {
    use super::*;

    fn tool() -> Tool {
        Tool { number: 2, name: "6mm endmill".into(), diameter: 6.0, rpm: 12000.0, feed: 800.0, plunge: 200.0 }
    }

    #[test]
    fn a_square_pocket_clears_to_the_wall_and_leaves_the_island() {
        let sq = [DVec2::new(0.0, 0.0), DVec2::new(40.0, 0.0), DVec2::new(40.0, 40.0), DVec2::new(0.0, 40.0)];
        let island =
            vec![DVec2::new(15.0, 15.0), DVec2::new(25.0, 15.0), DVec2::new(25.0, 25.0), DVec2::new(15.0, 25.0)];
        let p = PocketParams { top_z: 0.0, depth: 4.0, step_down: 2.0, stepover: 0.4, clearance: 5.0 };
        let tp = pocket(&sq, &[island], &tool(), &p);
        let cuts: Vec<DVec3> = tp
            .moves
            .iter()
            .filter_map(|m| match m {
                Move::Linear { to, .. } => Some(*to),
                _ => None,
            })
            .collect();
        assert!(!cuts.is_empty());
        for c in &cuts {
            // The tool centre stays a radius from every wall and island.
            let inside_wall = c.x >= 3.0 - 1e-6 && c.x <= 37.0 + 1e-6 && c.y >= 3.0 - 1e-6 && c.y <= 37.0 + 1e-6;
            let dx = (15.0 - c.x).max(c.x - 25.0).max(0.0);
            let dy = (15.0 - c.y).max(c.y - 25.0).max(0.0);
            assert!(inside_wall, "the tool cuts the wall at {c}");
            assert!((dx * dx + dy * dy).sqrt() >= 3.0 - 1e-3, "the tool cuts the island at {c}");
        }
        // Two depth passes reach the floor.
        assert!(cuts.iter().any(|c| (c.z + 4.0).abs() < 1e-9));
        // Neighbouring rings are within one stepover plus a little: no
        // ridge of material is left between them.
        let rings = tp.moves.iter().filter(|m| matches!(m, Move::Linear { feed, .. } if *feed == 200.0)).count();
        assert!(rings >= 2 * 3, "{rings} plunges");
    }

    #[test]
    fn a_pocket_smaller_than_the_tool_is_refused() {
        let sq = [DVec2::new(0.0, 0.0), DVec2::new(4.0, 0.0), DVec2::new(4.0, 4.0), DVec2::new(0.0, 4.0)];
        let p = PocketParams { top_z: 0.0, depth: 1.0, step_down: 1.0, stepover: 0.4, clearance: 5.0 };
        let tp = pocket(&sq, &[], &tool(), &p);
        assert!(tp.moves.iter().all(|m| !matches!(m, Move::Linear { .. })));
    }
}
