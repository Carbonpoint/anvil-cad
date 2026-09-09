//! Closed-loop extraction.
//!
//! A profile is a closed polyline in sketch coordinates, counter-clockwise.
//! Circles and arcs are discretised. Lines marked `construction` are ignored.

use crate::{Entity, EntityId, Sketch};
use anvil_math::DVec2;
use std::collections::HashMap;

/// Number of segments used to discretise a full circle.
pub const CIRCLE_SEGMENTS: usize = 64;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Profile {
    /// Closed polyline; the last point connects back to the first.
    pub points: Vec<DVec2>,
}

impl Profile {
    pub fn signed_area(&self) -> f64 {
        let n = self.points.len();
        let mut a = 0.0;
        for i in 0..n {
            let p = self.points[i];
            let q = self.points[(i + 1) % n];
            a += p.x * q.y - q.x * p.y;
        }
        0.5 * a
    }
    pub fn make_ccw(&mut self) {
        if self.signed_area() < 0.0 {
            self.points.reverse();
        }
    }
}

pub fn extract(s: &Sketch) -> Vec<Profile> {
    let mut profiles = Vec::new();

    // Full circles are profiles on their own.
    for e in s.entities.values() {
        if let Entity::Circle { center, radius } = e {
            let c = s.point(*center);
            let points = (0..CIRCLE_SEGMENTS)
                .map(|i| {
                    let t = i as f64 / CIRCLE_SEGMENTS as f64 * std::f64::consts::TAU;
                    c + DVec2::new(t.cos(), t.sin()) * *radius
                })
                .collect();
            profiles.push(Profile { points });
        }
    }

    // Lines and arcs: walk edge chains by shared point ids (or coincident locations).
    // Endpoints are merged by location so that unconstrained coincidence still closes.
    let mut key_of: HashMap<EntityId, usize> = HashMap::new();
    let mut nodes: Vec<DVec2> = Vec::new();
    let mut node_for = |id: EntityId, s: &Sketch| -> usize {
        if let Some(k) = key_of.get(&id) {
            return *k;
        }
        let p = s.point(id);
        let k = nodes.iter().position(|q| (*q - p).length() < 1e-6).unwrap_or_else(|| {
            nodes.push(p);
            nodes.len() - 1
        });
        key_of.insert(id, k);
        k
    };

    // Each edge: (node_a, node_b, intermediate points from a to b exclusive)
    let mut edges: Vec<(usize, usize, Vec<DVec2>)> = Vec::new();
    for e in s.entities.values() {
        match e {
            Entity::Line { a, b, construction: false } => {
                let na = node_for(*a, s);
                let nb = node_for(*b, s);
                edges.push((na, nb, Vec::new()));
            }
            Entity::Arc { center, start, end } => {
                let c = s.point(*center);
                let ps = s.point(*start);
                let pe = s.point(*end);
                let r = (ps - c).length();
                let a0 = (ps - c).y.atan2((ps - c).x);
                let mut a1 = (pe - c).y.atan2((pe - c).x);
                if a1 <= a0 {
                    a1 += std::f64::consts::TAU;
                }
                let n = ((a1 - a0) / std::f64::consts::TAU * CIRCLE_SEGMENTS as f64).ceil().max(2.0) as usize;
                let mid: Vec<DVec2> = (1..n)
                    .map(|i| {
                        let t = a0 + (a1 - a0) * i as f64 / n as f64;
                        c + DVec2::new(t.cos(), t.sin()) * r
                    })
                    .collect();
                let na = node_for(*start, s);
                let nb = node_for(*end, s);
                edges.push((na, nb, mid));
            }
            _ => {}
        }
    }

    let mut used = vec![false; edges.len()];
    for start in 0..edges.len() {
        if used[start] {
            continue;
        }
        let mut loop_pts = vec![nodes[edges[start].0]];
        let origin = edges[start].0;
        let mut cur = start;
        let mut forward = true;
        let mut closed = false;
        loop {
            used[cur] = true;
            let (a, b, mid) = &edges[cur];
            let (next_node, mids): (usize, Vec<DVec2>) =
                if forward { (*b, mid.clone()) } else { (*a, mid.iter().rev().cloned().collect()) };
            loop_pts.extend(mids);
            if next_node == origin {
                closed = true;
                break;
            }
            loop_pts.push(nodes[next_node]);
            let mut found = None;
            for (i, (ea, eb, _)) in edges.iter().enumerate() {
                if used[i] {
                    continue;
                }
                if *ea == next_node {
                    found = Some((i, true));
                    break;
                }
                if *eb == next_node {
                    found = Some((i, false));
                    break;
                }
            }
            match found {
                Some((i, f)) => {
                    cur = i;
                    forward = f;
                }
                None => break,
            }
        }
        if closed && loop_pts.len() >= 3 {
            let mut p = Profile { points: loop_pts };
            p.make_ccw();
            profiles.push(p);
        }
    }
    profiles
}

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_math::Plane;

    #[test]
    fn rectangle_gives_one_ccw_profile() {
        let mut s = Sketch::new(Plane::XY);
        s.add_rectangle(0.0, 0.0, 4.0, 2.0);
        let p = s.profiles();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].points.len(), 4);
        assert!((p[0].signed_area() - 8.0).abs() < 1e-12);
    }
}
