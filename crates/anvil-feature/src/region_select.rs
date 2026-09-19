//! Choosing which closed regions of a sketch a feature uses.
//!
//! Every profile bounds one region: the area inside it minus the profiles
//! directly inside it. The default regions are the ones at even nesting
//! depth, so a plate with holes stays a plate with holes; an island inside
//! a hole, or the inside of a hole, is only used when picked. Features store
//! the choice as text: empty means the default regions, otherwise region
//! indices separated by commas, for example "0,2".

use crate::features::emboss::{point_in_poly, polygon_area};
use crate::RegenError;
use anvil_math::DVec2;

/// Outer profile and the profiles directly inside it, in sketch plane
/// coordinates.
#[derive(Clone, Debug)]
pub struct Region {
    pub outer: Vec<DVec2>,
    pub holes: Vec<Vec<DVec2>>,
    /// Used when the feature picks no regions explicitly.
    pub default: bool,
}

/// The regions of a sketch's closed profiles: the default ones first, in
/// profile order, then the insides of holes.
pub fn regions(profiles: &[anvil_sketch::Profile]) -> Vec<Region> {
    let loops: Vec<&[DVec2]> = profiles.iter().map(|p| p.points.as_slice()).collect();
    let n = loops.len();
    // A loop counts as inside another when most of its sample vertices are,
    // so a hole that touches the outer loop at one vertex still nests.
    let inside = |i: usize, j: usize| -> bool {
        let li = loops[i];
        let step = (li.len() / 5).max(1);
        let samples: Vec<DVec2> = li.iter().step_by(step).take(5).copied().collect();
        let hits = samples.iter().filter(|&&p| point_in_poly(p, loops[j])).count();
        hits * 2 > samples.len()
    };
    let valid = |i: usize| loops[i].len() >= 3;
    let depth: Vec<usize> = (0..n).map(|i| (0..n).filter(|&j| j != i && valid(j) && inside(i, j)).count()).collect();
    // Parent: the smallest loop one level up that contains the loop.
    let parent: Vec<Option<usize>> = (0..n)
        .map(|i| {
            (0..n)
                .filter(|&j| j != i && valid(j) && depth[j] + 1 == depth[i] && inside(i, j))
                .min_by(|&a, &b| polygon_area(loops[a]).abs().total_cmp(&polygon_area(loops[b]).abs()))
        })
        .collect();
    let region = |i: usize| Region {
        outer: loops[i].to_vec(),
        holes: (0..n).filter(|&k| parent[k] == Some(i)).map(|k| loops[k].to_vec()).collect(),
        default: depth[i].is_multiple_of(2),
    };
    let even = (0..n).filter(|&i| depth[i].is_multiple_of(2));
    let odd = (0..n).filter(|&i| !depth[i].is_multiple_of(2) && valid(i));
    even.chain(odd).map(region).collect()
}

/// Region indices named by `spec`, or `None` for the default regions.
pub fn parse(spec: &str) -> Option<Vec<usize>> {
    let t = spec.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("all") {
        return None;
    }
    let mut v: Vec<usize> = t.split([',', ' ']).filter_map(|s| s.trim().parse().ok()).collect();
    v.sort_unstable();
    v.dedup();
    Some(v)
}

pub fn format(sel: &[usize]) -> String {
    sel.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")
}

/// True when region `i` is used under `spec`.
pub fn is_selected(regions: &[Region], spec: &str, i: usize) -> bool {
    match parse(spec) {
        None => regions.get(i).is_some_and(|r| r.default),
        Some(v) => v.contains(&i),
    }
}

/// The spec after a click on region `i`. A click on a used region drops
/// it; a click on an unused one adds it. Ending on the default regions
/// gives back the empty spec.
pub fn toggle(regions: &[Region], spec: &str, i: usize) -> String {
    let n = regions.len();
    let defaults: Vec<usize> = (0..n).filter(|&k| regions[k].default).collect();
    let mut v = parse(spec).unwrap_or_else(|| defaults.clone());
    v.retain(|&k| k < n);
    match v.iter().position(|&k| k == i) {
        Some(p) => {
            v.remove(p);
        }
        None => {
            v.push(i);
            v.sort_unstable();
        }
    }
    if v == defaults {
        String::new()
    } else {
        format(&v)
    }
}

/// An outer profile and its holes, as the kernel takes them.
pub type OuterAndHoles = (Vec<DVec2>, Vec<Vec<DVec2>>);

/// The regions that `spec` picks from a sketch's profiles.
pub fn select(profiles: &[anvil_sketch::Profile], spec: &str) -> Result<Vec<OuterAndHoles>, RegenError> {
    let all = regions(profiles);
    let picked: Vec<Region> = match parse(spec) {
        None => all.into_iter().filter(|r| r.default).collect(),
        Some(sel) => {
            if sel.is_empty() {
                return Err(RegenError::Other("no region selected; click a region of the sketch".into()));
            }
            if let Some(bad) = sel.iter().find(|&&i| i >= all.len()) {
                return Err(RegenError::Other(format!(
                    "region {bad} no longer exists (the sketch has {} regions); pick the regions again",
                    all.len()
                )));
            }
            all.into_iter().enumerate().filter(|(i, _)| sel.contains(i)).map(|(_, r)| r).collect()
        }
    };
    Ok(picked.into_iter().map(|r| (r.outer, r.holes)).collect())
}

/// The smallest region that contains `p` (inside the outer profile and
/// outside its holes).
pub fn region_at(regions: &[Region], p: DVec2) -> Option<usize> {
    regions
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            r.outer.len() >= 3 && point_in_poly(p, &r.outer) && !r.holes.iter().any(|l| point_in_poly(p, l))
        })
        .min_by(|a, b| polygon_area(&a.1.outer).abs().total_cmp(&polygon_area(&b.1.outer).abs()))
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_sketch::Profile;

    fn square(c: DVec2, h: f64) -> Profile {
        Profile {
            points: vec![c + DVec2::new(-h, -h), c + DVec2::new(h, -h), c + DVec2::new(h, h), c + DVec2::new(-h, h)],
        }
    }

    #[test]
    fn toggle_from_default_and_back() {
        let r =
            regions(&[square(DVec2::ZERO, 1.0), square(DVec2::new(5.0, 0.0), 1.0), square(DVec2::new(9.0, 0.0), 1.0)]);
        assert_eq!(toggle(&r, "", 1), "0,2");
        assert_eq!(toggle(&r, "0,2", 1), "");
        assert_eq!(toggle(&r, "0", 0), "");
        assert!(is_selected(&r, "", 2));
        assert!(!is_selected(&r, "0,2", 1));
    }

    #[test]
    fn select_and_pick_nested_regions() {
        // Big square with a hole, an island in the hole, and a separate square.
        let profs = vec![
            square(DVec2::ZERO, 10.0),
            square(DVec2::ZERO, 5.0),
            square(DVec2::ZERO, 2.0),
            square(DVec2::new(30.0, 0.0), 3.0),
        ];
        let r = regions(&profs);
        assert_eq!(r.len(), 4);
        assert_eq!(r.iter().filter(|x| x.default).count(), 3);
        let ring = region_at(&r, DVec2::new(7.0, 0.0)).unwrap();
        let island = region_at(&r, DVec2::ZERO).unwrap();
        let side = region_at(&r, DVec2::new(30.0, 0.0)).unwrap();
        assert_eq!(r[ring].holes.len(), 1);
        assert_ne!(ring, island);
        // Between the hole and the island: the inside of the hole, not a default region.
        let gap = region_at(&r, DVec2::new(3.5, 0.0)).unwrap();
        assert!(!r[gap].default);
        assert_eq!(r[gap].holes.len(), 1);
        let picked = select(&profs, &format(&[side])).unwrap();
        assert_eq!(picked.len(), 1);
        assert!(select(&profs, "7").is_err());
        assert_eq!(select(&profs, " ").unwrap().len(), 3);
    }

    #[test]
    fn circle_inside_a_face_outline_can_be_picked_alone() {
        let profs = vec![square(DVec2::ZERO, 10.0), square(DVec2::new(2.0, 2.0), 1.0)];
        let r = regions(&profs);
        let boss = region_at(&r, DVec2::new(2.0, 2.0)).unwrap();
        let picked = select(&profs, &format(&[boss])).unwrap();
        assert_eq!(picked.len(), 1);
        assert!(picked[0].1.is_empty());
        assert!((polygon_area(&picked[0].0).abs() - 4.0).abs() < 1e-9);
    }
}
