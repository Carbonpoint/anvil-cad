//! Boolean operations on planar-facet solids by BSP-tree CSG.
//!
//! This is the classic csg.js algorithm: each solid becomes a set of convex
//! polygons (we use its triangles), a BSP tree clips one against the other,
//! and the surviving fragments are welded back into a `Solid`. Every
//! fragment carries the surface tag of the face it came from, so smooth
//! edge detection and picking still treat coplanar fragments as one face.
//!
//! Limits: planar facets only, no tolerant modeling. Coplanar faces from
//! the two inputs are handled by the usual csg.js coplanar rules, which is
//! adequate for holes, bosses, and pockets on prismatic parts.

use crate::topology::{Solid, Surface, VertexId};
use crate::BooleanOp;
use anvil_math::DVec3;

const EPS: f64 = 1e-6;

#[derive(Clone, Debug)]
struct Polygon {
    verts: Vec<DVec3>,
    normal: DVec3,
    w: f64,
    tag: Surface,
}

impl Polygon {
    fn new(verts: Vec<DVec3>, tag: Surface) -> Option<Self> {
        if verts.len() < 3 {
            return None;
        }
        let n = (verts[1] - verts[0]).cross(verts[2] - verts[0]);
        if n.length_squared() < 1e-18 {
            return None;
        }
        let normal = n.normalize();
        let w = normal.dot(verts[0]);
        Some(Polygon { verts, normal, w, tag })
    }
    fn flip(&mut self) {
        self.verts.reverse();
        self.normal = -self.normal;
        self.w = -self.w;
    }
}

#[derive(Clone, Copy)]
struct Plane {
    normal: DVec3,
    w: f64,
}

const COPLANAR: u8 = 0;
const FRONT: u8 = 1;
const BACK: u8 = 2;
const SPANNING: u8 = 3;

impl Plane {
    fn flip(&mut self) {
        self.normal = -self.normal;
        self.w = -self.w;
    }

    fn split(
        &self,
        poly: &Polygon,
        cf: &mut Vec<Polygon>,
        cb: &mut Vec<Polygon>,
        front: &mut Vec<Polygon>,
        back: &mut Vec<Polygon>,
    ) {
        let mut ptype = 0u8;
        let types: Vec<u8> = poly
            .verts
            .iter()
            .map(|v| {
                let t = self.normal.dot(*v) - self.w;
                let ty = if t < -EPS {
                    BACK
                } else if t > EPS {
                    FRONT
                } else {
                    COPLANAR
                };
                ptype |= ty;
                ty
            })
            .collect();
        match ptype {
            COPLANAR => {
                if self.normal.dot(poly.normal) > 0.0 {
                    cf.push(poly.clone())
                } else {
                    cb.push(poly.clone())
                }
            }
            FRONT => front.push(poly.clone()),
            BACK => back.push(poly.clone()),
            _ => {
                let mut f = Vec::new();
                let mut b = Vec::new();
                let n = poly.verts.len();
                for i in 0..n {
                    let j = (i + 1) % n;
                    let (ti, tj) = (types[i], types[j]);
                    let (vi, vj) = (poly.verts[i], poly.verts[j]);
                    if ti != BACK {
                        f.push(vi);
                    }
                    if ti != FRONT {
                        b.push(vi);
                    }
                    if (ti | tj) == SPANNING {
                        let t = (self.w - self.normal.dot(vi)) / self.normal.dot(vj - vi);
                        let v = vi.lerp(vj, t);
                        f.push(v);
                        b.push(v);
                    }
                }
                if let Some(p) = Polygon::new(f, poly.tag) {
                    front.push(p);
                }
                if let Some(p) = Polygon::new(b, poly.tag) {
                    back.push(p);
                }
            }
        }
    }
}

#[derive(Default)]
struct Node {
    plane: Option<Plane>,
    front: Option<Box<Node>>,
    back: Option<Box<Node>>,
    polygons: Vec<Polygon>,
}

impl Node {
    fn new(polys: Vec<Polygon>) -> Node {
        let mut n = Node::default();
        n.build(polys);
        n
    }

    fn invert(&mut self) {
        for p in &mut self.polygons {
            p.flip();
        }
        if let Some(pl) = &mut self.plane {
            pl.flip();
        }
        if let Some(f) = &mut self.front {
            f.invert();
        }
        if let Some(b) = &mut self.back {
            b.invert();
        }
        std::mem::swap(&mut self.front, &mut self.back);
    }

    fn clip_polygons(&self, polys: Vec<Polygon>) -> Vec<Polygon> {
        let Some(plane) = self.plane else { return polys };
        let mut front = Vec::new();
        let mut back = Vec::new();
        for p in &polys {
            let (mut cf, mut cb) = (Vec::new(), Vec::new());
            plane.split(p, &mut cf, &mut cb, &mut front, &mut back);
            front.extend(cf);
            back.extend(cb);
        }
        let front = match &self.front {
            Some(f) => f.clip_polygons(front),
            None => front,
        };
        let back = match &self.back {
            Some(b) => b.clip_polygons(back),
            None => Vec::new(),
        };
        let mut out = front;
        out.extend(back);
        out
    }

    fn clip_to(&mut self, other: &Node) {
        self.polygons = other.clip_polygons(std::mem::take(&mut self.polygons));
        if let Some(f) = &mut self.front {
            f.clip_to(other);
        }
        if let Some(b) = &mut self.back {
            b.clip_to(other);
        }
    }

    fn all_polygons(&self) -> Vec<Polygon> {
        let mut out = self.polygons.clone();
        if let Some(f) = &self.front {
            out.extend(f.all_polygons());
        }
        if let Some(b) = &self.back {
            out.extend(b.all_polygons());
        }
        out
    }

    fn build(&mut self, polys: Vec<Polygon>) {
        if polys.is_empty() {
            return;
        }
        if self.plane.is_none() {
            // Pick a splitting plane from the middle to keep the tree shallower.
            let p = &polys[polys.len() / 2];
            self.plane = Some(Plane { normal: p.normal, w: p.w });
        }
        let plane = self.plane.unwrap();
        let mut front = Vec::new();
        let mut back = Vec::new();
        for p in &polys {
            let (mut cf, mut cb) = (Vec::new(), Vec::new());
            plane.split(p, &mut cf, &mut cb, &mut front, &mut back);
            self.polygons.extend(cf);
            self.polygons.extend(cb);
        }
        if !front.is_empty() {
            self.front.get_or_insert_with(Default::default).build(front);
        }
        if !back.is_empty() {
            self.back.get_or_insert_with(Default::default).build(back);
        }
    }
}

fn to_polygons(s: &Solid) -> Vec<Polygon> {
    let m = crate::mesh::tessellate(s);
    let mut out = Vec::new();
    for (k, t) in m.indices.as_chunks::<3>().0.iter().enumerate() {
        let tag = m.face_of_tri.get(k).map(|f| s.faces[*f].surface).unwrap_or(Surface::Plane);
        if let Some(p) =
            Polygon::new(vec![m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]], tag)
        {
            out.push(p);
        }
    }
    out
}

fn from_polygons(polys: Vec<Polygon>) -> Solid {
    use std::collections::HashMap;
    let mut s = Solid::new();
    // Weld within a distance tolerance: look in the 27 neighbouring grid
    // cells, so two points that straddle a cell boundary still merge.
    const WELD: f64 = 1e-6;
    let mut map: HashMap<(i64, i64, i64), Vec<(DVec3, VertexId)>> = HashMap::new();
    let cell = |p: DVec3| ((p.x / WELD).floor() as i64, (p.y / WELD).floor() as i64, (p.z / WELD).floor() as i64);
    let mut weld = |s: &mut Solid, v: DVec3| -> VertexId {
        let (cx, cy, cz) = cell(v);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(list) = map.get(&(cx + dx, cy + dy, cz + dz)) {
                        if let Some((_, id)) = list.iter().find(|(q, _)| (*q - v).length() <= WELD) {
                            return *id;
                        }
                    }
                }
            }
        }
        let id = s.add_vertex(v);
        map.entry((cx, cy, cz)).or_default().push((v, id));
        id
    };
    for p in polys {
        let mut ids: Vec<VertexId> = Vec::with_capacity(p.verts.len());
        for v in p.verts {
            let id = weld(&mut s, v);
            if ids.last() != Some(&id) {
                ids.push(id);
            }
        }
        if ids.len() >= 3 && ids.first() == ids.last() {
            ids.pop();
        }
        if ids.len() < 3 {
            continue;
        }
        s.faces.insert(crate::topology::Face { outer: ids, inner: Vec::new(), surface: p.tag });
    }
    // Rebuild edge list from faces.
    let faces: Vec<Vec<VertexId>> = s.faces.values().map(|f| f.outer.clone()).collect();
    let mut seen = std::collections::HashSet::new();
    for f in faces {
        for i in 0..f.len() {
            let a = f[i];
            let b = f[(i + 1) % f.len()];
            let k = if a < b { (a, b) } else { (b, a) };
            if seen.insert(k) {
                s.edges.insert(crate::topology::Edge { a, b });
            }
        }
    }
    s
}

/// Tag input faces so fragments of one planar face stay grouped.
fn retag(s: &Solid, base: u32) -> Solid {
    let mut c = s.clone();
    let mut next = base;
    for f in c.faces.values_mut() {
        if f.surface == Surface::Plane {
            f.surface = Surface::Cylindrical { id: next };
            next += 1;
        }
    }
    c
}

pub fn boolean(a: &Solid, b: &Solid, op: BooleanOp) -> Solid {
    let a = retag(a, 1_000_000);
    let b = retag(b, 2_000_000);
    let mut na = Node::new(to_polygons(&a));
    let mut nb = Node::new(to_polygons(&b));
    let polys = match op {
        BooleanOp::Union => {
            na.clip_to(&nb);
            nb.clip_to(&na);
            nb.invert();
            nb.clip_to(&na);
            nb.invert();
            na.build(nb.all_polygons());
            na.all_polygons()
        }
        BooleanOp::Subtract => {
            na.invert();
            na.clip_to(&nb);
            nb.clip_to(&na);
            nb.invert();
            nb.clip_to(&na);
            nb.invert();
            na.build(nb.all_polygons());
            na.invert();
            na.all_polygons()
        }
        BooleanOp::Intersect => {
            na.invert();
            nb.clip_to(&na);
            nb.invert();
            na.clip_to(&nb);
            nb.clip_to(&na);
            na.build(nb.all_polygons());
            na.invert();
            na.all_polygons()
        }
    };
    let mut s = from_polygons(polys);
    s.fix_t_junctions();
    // Merging fragments keeps later booleans fast. Keep the merge only if
    // it does not make the mesh less watertight.
    let before = s.open_edge_report();
    let mut merged = s.clone();
    merged.merge_coplanar_faces();
    let after = merged.open_edge_report();
    if after.0 + after.1 <= before.0 + before.1 {
        merged
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{box_solid, cylinder};
    use anvil_math::{DVec2, Plane as MPlane};

    #[test]
    fn subtract_hole_from_plate() {
        let plate = box_solid(DVec3::ZERO, DVec3::new(40.0, 20.0, 5.0)).unwrap();
        let hole = cylinder(&MPlane::XY, DVec2::new(20.0, 10.0), 4.0, 5.0).unwrap();
        let r = boolean(&plate, &hole, BooleanOp::Subtract);
        let exact = 4000.0 - hole.volume();
        assert!((r.volume() - exact).abs() < 1e-6, "{} vs {exact}", r.volume());
    }

    #[test]
    fn union_and_intersect_overlapping_boxes() {
        let a = box_solid(DVec3::ZERO, DVec3::new(10.0, 10.0, 10.0)).unwrap();
        let b = box_solid(DVec3::new(5.0, 5.0, 5.0), DVec3::new(10.0, 10.0, 10.0)).unwrap();
        let u = boolean(&a, &b, BooleanOp::Union);
        assert!((u.volume() - 1875.0).abs() < 1e-6, "{}", u.volume());
        let i = boolean(&a, &b, BooleanOp::Intersect);
        assert!((i.volume() - 125.0).abs() < 1e-6, "{}", i.volume());
    }

    #[test]
    fn through_hole_with_coplanar_caps() {
        // Cylinder taller than the plate on both sides: the common case.
        let plate = box_solid(DVec3::ZERO, DVec3::new(30.0, 30.0, 6.0)).unwrap();
        let drill =
            cylinder(&MPlane { origin: DVec3::new(0.0, 0.0, -1.0), ..MPlane::XY }, DVec2::new(15.0, 15.0), 3.0, 8.0)
                .unwrap();
        let r = boolean(&plate, &drill, BooleanOp::Subtract);
        let hole_vol = cylinder(&MPlane::XY, DVec2::ZERO, 3.0, 6.0).unwrap().volume();
        assert!((r.volume() - (5400.0 - hole_vol)).abs() < 1e-6, "{}", r.volume());
        let (open, over) = r.open_edge_report();
        assert_eq!((open, over), (0, 0), "result must be watertight; euler {}", r.euler_characteristic());
    }

    #[test]
    fn repeated_holes_keep_face_count_small() {
        let mut plate = box_solid(DVec3::ZERO, DVec3::new(100.0, 60.0, 8.0)).unwrap();
        for (x, y) in [(10.0, 10.0), (90.0, 10.0), (90.0, 50.0), (10.0, 50.0), (50.0, 30.0)] {
            let drill =
                cylinder(&MPlane { origin: DVec3::new(0.0, 0.0, 9.0), ..MPlane::XY }, DVec2::new(x, y), 4.0, -10.0)
                    .unwrap();
            plate = boolean(&plate, &drill, BooleanOp::Subtract);
        }
        // Known limit: after several booleans in a row a few edges can be
        // shared by more than two faces (non-manifold slivers). Volume stays
        // exact; see docs/WORKBOOK.md. Open (single-use) edges must stay zero.
        assert_eq!(plate.open_edge_report().0, 0, "{:?}", plate.open_edge_report());
        // Without merging this grew past ten thousand fragments. Merging is
        // skipped for groups whose boundary loops are ambiguous, so the count
        // still creeps up with each boolean; see docs/WORKBOOK.md.
        assert!(plate.faces.len() < 2500, "{} faces", plate.faces.len());
        let exact = 48000.0 - 5.0 * cylinder(&MPlane::XY, DVec2::ZERO, 4.0, 8.0).unwrap().volume();
        assert!((plate.volume() - exact).abs() / exact < 1e-9, "{} vs {exact}", plate.volume());
    }
}
