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
    walk(s).0
}

/// Open polylines (chains that do not close), for sweep paths.
pub fn extract_open(s: &Sketch) -> Vec<Vec<DVec2>> {
    walk(s).1
}

fn walk(s: &Sketch) -> (Vec<Profile>, Vec<Vec<DVec2>>) {
    let mut profiles = Vec::new();
    let mut open = Vec::new();

    // Full circles, ellipses, and closed splines are profiles on their own.
    for (id, e) in &s.entities {
        let own = matches!(e, Entity::Circle { .. } | Entity::Ellipse { .. } | Entity::Spline { closed: true, .. });
        if own {
            let mut points = s.sample(id);
            points.pop();
            let mut p = Profile { points };
            p.make_ccw();
            profiles.push(p);
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
    for (eid, e) in &s.entities {
        match e {
            Entity::Line { a, b, construction: false } => {
                let na = node_for(*a, s);
                let nb = node_for(*b, s);
                edges.push((na, nb, Vec::new()));
            }
            Entity::Arc { start, end, .. } => {
                let mut pts = s.sample(eid);
                pts.pop();
                pts.remove(0);
                let na = node_for(*start, s);
                let nb = node_for(*end, s);
                edges.push((na, nb, pts));
            }
            Entity::Spline { points, closed: false } if points.len() >= 2 => {
                let mut pts = s.sample(eid);
                pts.pop();
                pts.remove(0);
                let na = node_for(points[0], s);
                let nb = node_for(*points.last().unwrap(), s);
                edges.push((na, nb, pts));
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
        } else if !closed && loop_pts.len() >= 2 {
            open.push(loop_pts);
        }
    }
    (profiles, open)
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
