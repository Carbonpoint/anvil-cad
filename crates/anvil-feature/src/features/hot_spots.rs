//! Hot spots: where a casting may shrink, and whether the metal reaches it.
//!
//! A coarse model on a voxel grid, with no outside solver. Stage 5 of
//! docs/KETTLE.md; the method is in docs/research/casting_simulation.md.
//!
//! * **Fill.** From the pouring entry (the top of the sprue, or the
//!   highest metal), metal spreads through touching voxels. Metal that is
//!   not reached cannot fill.
//! * **Freezing order.** A voxel's distance to the mold wall ranks when it
//!   freezes, the inscribed sphere method (Heuvers' circles in 3D): a
//!   thick place freezes after a thin one. Chvorinov gives a time,
//!   t = C d^2. The distance equals the modulus of a plate and is larger
//!   than the modulus of a bar or a ball, so the times are an upper bound;
//!   the order is what counts.
//! * **Feeding.** Metal shrinks as it freezes, and liquid must flow in to
//!   make up for it. Going from the last voxel to freeze back to the
//!   first, liquid regions join. A region that joins a region holding a
//!   riser only at some level d was cut off from the riser while it froze
//!   from that level up to its own peak: an isolated pocket, a shrinkage
//!   risk. The whole casting with no riser is one such pocket at its end.
//!
//! The pockets come out as a voxel body, and the note lists them.

use crate::features::casting::{alloy_by_name, ALLOYS};
use crate::features::parse_index_list;
use crate::{Feature, FeatureDescriptor, FeatureOutput, ParamSpec, ParamValue, RegenContext, RegenError};
use anvil_math::DVec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HotSpotsFeature {
    /// Features whose bodies are the metal: the casting, the gating and
    /// the risers, as a list such as "15, 24, 25".
    pub metal: String,
    /// Features whose bodies are the part itself, a subset of `metal`.
    /// Only a pocket whose last point to freeze is in the part is
    /// reported: a pocket in a pouring cup does no harm. Empty: all metal.
    #[serde(default)]
    pub casting: String,
    /// Features whose bodies feed the casting: risers, and the sprue if it
    /// stays liquid long enough.
    pub risers: String,
    /// Feature whose top is where the metal is poured in. Empty: the
    /// highest metal.
    pub pour_at: String,
    pub alloy: String,
    /// Chvorinov mold constant in s/mm2; 0 takes the alloy's value.
    pub mold_constant: String,
    /// Voxel edge in mm.
    pub voxel: String,
}

impl Default for HotSpotsFeature {
    fn default() -> Self {
        HotSpotsFeature {
            metal: "1".into(),
            casting: String::new(),
            risers: String::new(),
            pour_at: String::new(),
            alloy: ALLOYS[0].name.into(),
            mold_constant: "0".into(),
            voxel: "1.5".into(),
        }
    }
}

/// Smallest pocket, or unreached region, worth a line in the note, in
/// voxels. Smaller ones are details thinner than the voxels.
const MIN_POCKET: usize = 20;

/// One isolated liquid pocket.
#[derive(Clone, Debug)]
pub struct Pocket {
    /// Where it freezes last.
    pub peak: DVec3,
    /// Distance to the mold at the peak and when it was cut off, in mm.
    pub d_peak: f64,
    pub d_cut: f64,
    /// Voxels in the pocket when it was cut off.
    pub voxels: Vec<usize>,
}

/// What the model finds.
pub struct Analysis {
    pub lo: DVec3,
    pub step: f64,
    pub n: [usize; 3],
    pub metal_voxels: usize,
    /// Unreached metal in regions of at least `MIN_POCKET` voxels, and in
    /// smaller regions.
    pub unreached: usize,
    pub unreached_specks: usize,
    pub pockets: Vec<Pocket>,
    /// Largest distance to the mold, in mm, and where.
    pub d_max: f64,
    pub at_max: DVec3,
    /// True when the last metal to freeze is in a riser.
    pub riser_last: bool,
}

/// Every triangle of a body.
fn triangles(b: &anvil_kernel::Solid) -> Vec<[DVec3; 3]> {
    let m = anvil_kernel::mesh::tessellate(b);
    m.indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| [m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]])
        .collect()
}

/// Run the model on the metal bodies, with the riser bodies and the pour
/// entry body marked. `step` is the voxel edge in mm.
pub fn analyse(
    metal: &[&anvil_kernel::Solid],
    casting: &[&anvil_kernel::Solid],
    risers: &[&anvil_kernel::Solid],
    pour: Option<&[&anvil_kernel::Solid]>,
    step: f64,
) -> Result<Analysis, String> {
    if step.is_nan() || step <= 0.0 {
        return Err("the voxel size must be above 0".into());
    }
    let mut lo = DVec3::splat(f64::INFINITY);
    let mut hi = DVec3::splat(f64::NEG_INFINITY);
    for b in metal {
        let bb = b.bounds();
        lo = lo.min(bb.min);
        hi = hi.max(bb.max);
    }
    if !lo.is_finite() {
        return Err("no metal body".into());
    }
    // One voxel of mold all round, so the wall distance is right.
    lo -= DVec3::splat(step);
    hi += DVec3::splat(step);
    let n = [
        ((hi.x - lo.x) / step).ceil() as usize,
        ((hi.y - lo.y) / step).ceil() as usize,
        ((hi.z - lo.z) / step).ceil() as usize,
    ];
    let cells = n[0] * n[1] * n[2];
    if cells > 40_000_000 {
        return Err(format!("{cells} voxels is too many; use a larger voxel size"));
    }
    let grid = |bodies: &[&anvil_kernel::Solid]| {
        let mut g = vec![false; cells];
        for b in bodies {
            for (q, inside) in anvil_implicit::sampled::inside_grid(&triangles(b), lo, step, n).into_iter().enumerate()
            {
                g[q] |= inside;
            }
        }
        g
    };
    let inside = grid(metal);
    let feeder = grid(risers);
    let part = if casting.is_empty() { inside.clone() } else { grid(casting) };
    let metal_voxels = inside.iter().filter(|&&b| b).count();
    if metal_voxels == 0 {
        return Err("the metal is thinner than one voxel; use a smaller voxel size".into());
    }
    let d2 = anvil_implicit::sampled::distance_squared(&inside, n, false);
    let d = |q: usize| (d2[q].sqrt() - 0.5).max(0.0) * step;
    let centre = |q: usize| {
        let (i, r) = (q / (n[1] * n[2]), q % (n[1] * n[2]));
        lo + DVec3::new(i as f64 + 0.5, (r / n[2]) as f64 + 0.5, (r % n[2]) as f64 + 0.5) * step
    };
    let neighbours = |q: usize| {
        let (i, r) = (q / (n[1] * n[2]), q % (n[1] * n[2]));
        let (j, k) = (r / n[2], r % n[2]);
        let mut out = [usize::MAX; 6];
        if i > 0 {
            out[0] = q - n[1] * n[2];
        }
        if i + 1 < n[0] {
            out[1] = q + n[1] * n[2];
        }
        if j > 0 {
            out[2] = q - n[2];
        }
        if j + 1 < n[1] {
            out[3] = q + n[2];
        }
        if k > 0 {
            out[4] = q - 1;
        }
        if k + 1 < n[2] {
            out[5] = q + 1;
        }
        out
    };

    // Fill: flood from the entry through touching metal.
    let entry: Vec<usize> = match pour {
        Some(bodies) => {
            let g = grid(bodies);
            let top = (0..cells).filter(|&q| g[q] && inside[q]).map(|q| q % n[2]).max();
            match top {
                Some(kt) => (0..cells).filter(|&q| g[q] && inside[q] && q % n[2] + 1 >= kt).collect(),
                None => Vec::new(),
            }
        }
        None => {
            let kt = (0..cells).filter(|&q| inside[q]).map(|q| q % n[2]).max().unwrap_or(0);
            (0..cells).filter(|&q| inside[q] && q % n[2] == kt).collect()
        }
    };
    let mut reached = vec![false; cells];
    let mut stack = entry.clone();
    for &q in &entry {
        reached[q] = true;
    }
    while let Some(q) = stack.pop() {
        for m in neighbours(q) {
            if m != usize::MAX && inside[m] && !reached[m] {
                reached[m] = true;
                stack.push(m);
            }
        }
    }
    // Unreached metal, region by region: a tiny region is a detail
    // thinner than the voxels, not metal that cannot fill.
    let (mut unreached, mut unreached_specks) = (0, 0);
    let mut seen = reached.clone();
    for q0 in 0..cells {
        if !inside[q0] || seen[q0] {
            continue;
        }
        let mut region = 0;
        let mut stack = vec![q0];
        seen[q0] = true;
        while let Some(q) = stack.pop() {
            region += 1;
            for m in neighbours(q) {
                if m != usize::MAX && inside[m] && !seen[m] {
                    seen[m] = true;
                    stack.push(m);
                }
            }
        }
        if region >= MIN_POCKET {
            unreached += region;
        } else {
            unreached_specks += region;
        }
    }

    // Feeding: join liquid regions from the last voxel to freeze down.
    let mut order: Vec<usize> = (0..cells).filter(|&q| inside[q]).collect();
    order.sort_by(|&a, &b| d2[b].total_cmp(&d2[a]).then(a.cmp(&b)));
    let mut parent: Vec<usize> = (0..cells).collect();
    let mut done = vec![false; cells];
    // Per root: peak voxel, riser flag, members.
    let mut peak: Vec<usize> = (0..cells).collect();
    let mut fed = feeder.clone();
    let mut members: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    fn find(parent: &mut [usize], mut q: usize) -> usize {
        while parent[q] != q {
            parent[q] = parent[parent[q]];
            q = parent[q];
        }
        q
    }
    let mut pockets = Vec::new();
    for &q in &order {
        done[q] = true;
        members.insert(q, vec![q]);
        for m in neighbours(q) {
            if m == usize::MAX || !done[m] {
                continue;
            }
            let (a, b) = (find(&mut parent, q), find(&mut parent, m));
            if a == b {
                continue;
            }
            // The winner keeps the region's identity: a fed region wins,
            // then the higher peak.
            let (win, lose) = match (fed[a], fed[b]) {
                (true, false) => (a, b),
                (false, true) => (b, a),
                _ if d2[peak[a]] >= d2[peak[b]] => (a, b),
                _ => (b, a),
            };
            if !fed[lose] && part[peak[lose]] {
                let lost = members.get(&lose).cloned().unwrap_or_default();
                if lost.len() >= MIN_POCKET && d(peak[lose]) - d(q) >= 0.5 * step {
                    pockets.push(Pocket { peak: centre(peak[lose]), d_peak: d(peak[lose]), d_cut: d(q), voxels: lost });
                }
            }
            parent[lose] = win;
            fed[win] |= fed[lose];
            if d2[peak[lose]] > d2[peak[win]] {
                peak[win] = peak[lose];
            }
            // Move the smaller list into the larger one.
            let mut small = members.remove(&lose).unwrap_or_default();
            let mut big = members.remove(&win).unwrap_or_default();
            if small.len() > big.len() {
                std::mem::swap(&mut small, &mut big);
            }
            big.extend(small);
            members.insert(win, big);
        }
    }
    // A region that never reached a riser is cut off until it freezes.
    let mut roots: Vec<usize> = members.keys().copied().collect();
    roots.sort();
    for r in roots {
        if !fed[r] && part[peak[r]] {
            let v = members[&r].clone();
            if v.len() >= MIN_POCKET {
                pockets.push(Pocket { peak: centre(peak[r]), d_peak: d(peak[r]), d_cut: 0.0, voxels: v });
            }
        }
    }
    pockets.sort_by_key(|p| std::cmp::Reverse(p.voxels.len()));
    let top = order[0];
    Ok(Analysis {
        lo,
        step,
        n,
        metal_voxels,
        unreached,
        unreached_specks,
        pockets,
        d_max: d(top),
        at_max: centre(top),
        riser_last: feeder[top],
    })
}

/// The bodies of every feature in a list such as "15, 24, 25".
fn bodies_in<'c>(ctx: &'c RegenContext, list: &str) -> Result<Vec<&'c anvil_kernel::Solid>, RegenError> {
    let mut out = Vec::new();
    for i in parse_index_list(list) {
        out.extend(ctx.bodies_of(i)?.iter());
    }
    Ok(out)
}

/// A closed body made of the voxels, for showing the pockets.
fn voxel_body(a: &Analysis, voxels: &[usize]) -> Option<anvil_kernel::Solid> {
    let n = a.n;
    let set: std::collections::HashSet<usize> = voxels.iter().copied().collect();
    let mut tris: Vec<[DVec3; 3]> = Vec::new();
    for &q in voxels {
        let (i, r) = (q / (n[1] * n[2]), q % (n[1] * n[2]));
        let (j, k) = (r / n[2], r % n[2]);
        let p = a.lo + DVec3::new(i as f64, j as f64, k as f64) * a.step;
        let s = a.step;
        // (axis, side, neighbour offset in voxels)
        for (axis, side) in [(0, 0), (0, 1), (1, 0), (1, 1), (2, 0), (2, 1)] {
            let idx = [i as isize, j as isize, k as isize];
            let mut o = idx;
            o[axis] += if side == 1 { 1 } else { -1 };
            let inside = o.iter().zip(n.iter()).all(|(&c, &m)| c >= 0 && (c as usize) < m)
                && set.contains(&(((o[0] as usize) * n[1] + o[1] as usize) * n[2] + o[2] as usize));
            if inside {
                continue;
            }
            // The four corners of this face, counter-clockwise from outside.
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            let corner = |du: f64, dv: f64| {
                let mut c = [0.0; 3];
                c[axis] = side as f64 * s;
                c[u] = du * s;
                c[v] = dv * s;
                p + DVec3::new(c[0], c[1], c[2])
            };
            let (c0, c1, c2, c3) = (corner(0.0, 0.0), corner(1.0, 0.0), corner(1.0, 1.0), corner(0.0, 1.0));
            if side == 1 {
                tris.push([c0, c1, c2]);
                tris.push([c0, c2, c3]);
            } else {
                tris.push([c0, c2, c1]);
                tris.push([c0, c3, c2]);
            }
        }
    }
    (!tris.is_empty()).then(|| anvil_kernel::ops::from_triangles(&tris, a.step * 1e-6))
}

#[typetag::serde(name = "hot_spots")]
impl Feature for HotSpotsFeature {
    fn kind(&self) -> &'static str {
        "hot_spots"
    }
    fn name(&self) -> String {
        format!("Hot spots ({})", self.metal)
    }
    fn params(&self) -> Vec<ParamSpec> {
        let names: Vec<&'static str> = ALLOYS.iter().map(|a| a.name).collect();
        let text = |name, label, v: &String| ParamSpec {
            name,
            label,
            kind: crate::param::ParamKind::Text,
            value: ParamValue::Expr(v.clone()),
        };
        vec![
            text("metal", "Metal features (list)", &self.metal),
            text("casting", "Casting features (list, empty = all)", &self.casting),
            text("risers", "Riser features (list)", &self.risers),
            text("pour_at", "Pour at feature (empty = top)", &self.pour_at),
            ParamSpec::choice("alloy", "Alloy", names, &self.alloy),
            ParamSpec::length("mold_constant", "Mold constant s/mm2 (0 = alloy)", &self.mold_constant),
            ParamSpec::length("voxel", "Voxel size", &self.voxel),
        ]
    }
    fn set_param(&mut self, name: &str, value: ParamValue) -> Result<(), String> {
        match (name, value) {
            ("metal", ParamValue::Expr(s)) => self.metal = s,
            ("casting", ParamValue::Expr(s)) => self.casting = s,
            ("risers", ParamValue::Expr(s)) => self.risers = s,
            ("pour_at", ParamValue::Expr(s)) => self.pour_at = s,
            ("alloy", ParamValue::Choice(s)) => self.alloy = s,
            ("mold_constant", ParamValue::Expr(s)) => self.mold_constant = s,
            ("voxel", ParamValue::Expr(s)) => self.voxel = s,
            (n, _) => return Err(format!("unknown parameter {n}")),
        }
        Ok(())
    }
    fn remap_refs(&mut self, map: &dyn Fn(usize) -> Option<usize>) -> Vec<&'static str> {
        let mut broken = Vec::new();
        for (name, list) in [
            ("metal", &mut self.metal),
            ("casting", &mut self.casting),
            ("risers", &mut self.risers),
            ("pour_at", &mut self.pour_at),
        ] {
            let old = parse_index_list(list);
            let new: Vec<usize> = old.iter().filter_map(|&i| map(i)).collect();
            if new.len() != old.len() {
                broken.push(name);
            }
            if new != old {
                *list = new.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(", ");
            }
        }
        broken
    }
    fn regenerate(&self, ctx: &mut RegenContext) -> Result<FeatureOutput, RegenError> {
        let alloy = alloy_by_name(&self.alloy);
        let c_user = ctx.eval(&self.mold_constant)?;
        let c = if c_user > 0.0 { c_user } else { alloy.mold_constant };
        let step = ctx.eval(&self.voxel)?;
        let ctx = &*ctx;
        let metal = bodies_in(ctx, &self.metal)?;
        if metal.is_empty() {
            return Err(RegenError::Other("list the metal features, for example 15, 24, 25".into()));
        }
        let casting = bodies_in(ctx, &self.casting)?;
        let risers = bodies_in(ctx, &self.risers)?;
        let pour = if self.pour_at.trim().is_empty() { None } else { Some(bodies_in(ctx, &self.pour_at)?) };
        let a = analyse(&metal, &casting, &risers, pour.as_deref(), step).map_err(RegenError::Other)?;
        let t = |d: f64| c * d * d;
        let vox_cm3 = a.step.powi(3) * 1e-3;
        let mut parts = vec![format!(
            "{} voxels of {} mm ({:.0} cm3); the last metal to freeze is {:.1} mm from the mold at ({:.0}, {:.0}, {:.0}), {}, after at most {:.0} s",
            a.metal_voxels,
            a.step,
            a.metal_voxels as f64 * vox_cm3,
            a.d_max,
            a.at_max.x,
            a.at_max.y,
            a.at_max.z,
            if a.riser_last { "in a riser, as it should be" } else { "NOT in a riser" },
            t(a.d_max)
        )];
        parts.push(if a.unreached == 0 {
            "fill: all the metal is reached from the pour".to_string()
        } else {
            format!(
                "fill: {} voxels ({:.1} cm3) are NOT reached from the pour",
                a.unreached,
                a.unreached as f64 * vox_cm3
            )
        });
        if a.pockets.is_empty() {
            parts.push("feeding: no isolated pocket; every region stays joined to a riser while it freezes".into());
        } else {
            parts.push(format!("feeding: {} isolated pockets, shrinkage risk", a.pockets.len()));
            for (k, p) in a.pockets.iter().take(8).enumerate() {
                parts.push(format!(
                    "pocket {} at ({:.0}, {:.0}, {:.0}), {:.1} cm3, cut off from {:.0} s to {:.0} s",
                    k + 1,
                    p.peak.x,
                    p.peak.y,
                    p.peak.z,
                    p.voxels.len() as f64 * vox_cm3,
                    t(p.d_cut),
                    t(p.d_peak)
                ));
            }
        }
        parts.push(format!("times from Chvorinov with C = {c} s/mm2, an upper bound; calibrate C from one pour"));
        let all: Vec<usize> = a.pockets.iter().flat_map(|p| p.voxels.iter().copied()).collect();
        let bodies = voxel_body(&a, &all).into_iter().collect();
        Ok(FeatureOutput { bodies, note: Some(parts.join("; ")), ..Default::default() })
    }
    fn clone_box(&self) -> Box<dyn Feature> {
        Box::new(self.clone())
    }
}

inventory::submit! { FeatureDescriptor { id: "hot_spots", label: "Hot spots", tab: "Solid", group: "Casting", tooltip: "Voxel fill check and shrinkage pockets (inscribed sphere method)", order: 21, create: || Box::new(HotSpotsFeature::default()) } }

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_kernel::ops;
    use anvil_math::Plane;

    fn block(x0: f64, y0: f64, z0: f64, dx: f64, dy: f64, dz: f64) -> anvil_kernel::Solid {
        let pts = [
            anvil_math::DVec2::new(x0, y0),
            anvil_math::DVec2::new(x0 + dx, y0),
            anvil_math::DVec2::new(x0 + dx, y0 + dy),
            anvil_math::DVec2::new(x0, y0 + dy),
        ];
        let plane = Plane { origin: DVec3::new(0.0, 0.0, z0), ..Plane::XY };
        ops::extrude_with_holes(&plane, &pts, &[], dz).unwrap()
    }

    #[test]
    fn a_pocket_outside_the_casting_is_not_reported() {
        // The same dumbbell, but only the plate and the first cube are the
        // part: the far cube is a pouring cup, and its pocket is harmless.
        let parts = dumbbell();
        let metal: Vec<&anvil_kernel::Solid> = parts.iter().collect();
        let a = analyse(&metal, &[&parts[0], &parts[1]], &[&parts[0]], None, 1.0).unwrap();
        assert!(a.pockets.is_empty(), "{:?}", a.pockets.iter().map(|p| p.peak).collect::<Vec<_>>());
    }

    /// Two 20 mm cubes joined by a 4 mm plate: a dumbbell.
    fn dumbbell() -> Vec<anvil_kernel::Solid> {
        vec![
            block(0.0, 0.0, 0.0, 20.0, 20.0, 20.0),
            block(20.0, 5.0, 8.0, 40.0, 10.0, 4.0),
            block(60.0, 0.0, 0.0, 20.0, 20.0, 20.0),
        ]
    }

    #[test]
    fn without_a_riser_the_thickest_place_is_a_pocket() {
        let parts = dumbbell();
        let metal: Vec<&anvil_kernel::Solid> = parts.iter().collect();
        let a = analyse(&metal, &[], &[], None, 1.0).unwrap();
        assert_eq!(a.unreached, 0);
        assert!(!a.riser_last);
        // Each cube is a pocket: one cut off by the thin plate, the other
        // never fed at all.
        assert_eq!(a.pockets.len(), 2, "{:?}", a.pockets.iter().map(|p| (p.peak, p.d_cut)).collect::<Vec<_>>());
        for p in &a.pockets {
            assert!((p.d_peak - 10.0).abs() < 1.0, "peak {}", p.d_peak);
        }
    }

    #[test]
    fn a_riser_on_one_cube_leaves_the_other_cut_off() {
        let parts = dumbbell();
        let metal: Vec<&anvil_kernel::Solid> = parts.iter().collect();
        let a = analyse(&metal, &[], &[&parts[0]], None, 1.0).unwrap();
        assert!(a.riser_last);
        assert_eq!(a.pockets.len(), 1);
        let p = &a.pockets[0];
        assert!(p.peak.x > 60.0, "the far cube is the pocket: {:?}", p.peak);
        // It was cut off where the plate froze, 2 mm from the mold.
        assert!((p.d_cut - 2.0).abs() < 1.0, "cut at {}", p.d_cut);
        let body = voxel_body(&a, &p.voxels).unwrap();
        assert!(body.open_edges().is_empty());
        assert!((body.volume() - p.voxels.len() as f64).abs() < 1e-6);
    }

    #[test]
    fn risers_on_both_cubes_feed_everything() {
        let parts = dumbbell();
        let metal: Vec<&anvil_kernel::Solid> = parts.iter().collect();
        let a = analyse(&metal, &[], &[&parts[0], &parts[2]], None, 1.0).unwrap();
        assert!(a.pockets.is_empty(), "{} pockets", a.pockets.len());
    }

    #[test]
    fn metal_the_pour_cannot_reach_is_counted() {
        let parts = [block(0.0, 0.0, 0.0, 10.0, 10.0, 10.0), block(30.0, 0.0, 0.0, 10.0, 10.0, 5.0)];
        let metal: Vec<&anvil_kernel::Solid> = parts.iter().collect();
        let a = analyse(&metal, &[], &[], Some(&[&parts[0]]), 1.0).unwrap();
        assert_eq!(a.unreached, 500, "the second block is 10 x 10 x 5 voxels");
    }
}
