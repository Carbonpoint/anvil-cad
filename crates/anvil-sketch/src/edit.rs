//! Editing operations on groups of entities: copy, mirror, pattern, move,
//! scale, fillet, trim, extend, offset, and projection of 3D segments.

use crate::{dist_to_segment, Constraint, Entity, EntityId, Sketch};
use anvil_math::{DVec2, DVec3};
use std::collections::HashMap;

impl Sketch {
    /// Copy entities (and the points they use) through a point transform.
    /// Returns the new entity ids in the same order as `ids`.
    pub fn duplicate(&mut self, ids: &[EntityId], f: impl Fn(DVec2) -> DVec2) -> Vec<EntityId> {
        let mut pmap: HashMap<EntityId, EntityId> = HashMap::new();
        let mut needed: Vec<EntityId> = Vec::new();
        for &id in ids {
            if let Some(e) = self.entities.get(id) {
                if matches!(e, Entity::Point { .. }) {
                    needed.push(id);
                }
                needed.extend(e.point_refs());
            }
        }
        for pid in needed {
            if pmap.contains_key(&pid) {
                continue;
            }
            let p = f(self.point(pid));
            let np = self.add_point(p.x, p.y);
            pmap.insert(pid, np);
        }
        let mut out = Vec::new();
        for &id in ids {
            let Some(e) = self.entities.get(id).cloned() else { continue };
            let m = |i: EntityId| pmap[&i];
            let ne = match e {
                Entity::Point { .. } => {
                    out.push(pmap[&id]);
                    continue;
                }
                Entity::Line { a, b, construction } => Entity::Line { a: m(a), b: m(b), construction },
                Entity::Circle { center, radius } => Entity::Circle { center: m(center), radius },
                Entity::Arc { center, start, end } => {
                    // A mirrored arc reverses direction.
                    let c = self.point(center);
                    let s = self.point(start);
                    let e2 = self.point(end);
                    let flips = (f(s) - f(c)).perp_dot(f(e2) - f(c)).signum() != (s - c).perp_dot(e2 - c).signum();
                    if flips {
                        Entity::Arc { center: m(center), start: m(end), end: m(start) }
                    } else {
                        Entity::Arc { center: m(center), start: m(start), end: m(end) }
                    }
                }
                Entity::Ellipse { center, rx, ry, rotation } => {
                    let c = self.point(center);
                    let axis = f(c + DVec2::new(rotation.cos(), rotation.sin())) - f(c);
                    Entity::Ellipse { center: m(center), rx, ry, rotation: axis.y.atan2(axis.x) }
                }
                Entity::Spline { points, closed } => {
                    Entity::Spline { points: points.iter().map(|&i| m(i)).collect(), closed }
                }
            };
            out.push(self.entities.insert(ne));
        }
        out
    }

    /// Mirror entities across the line `a`-`b`.
    pub fn mirror_entities(&mut self, ids: &[EntityId], a: DVec2, b: DVec2) -> Vec<EntityId> {
        let d = (b - a).normalize_or_zero();
        self.duplicate(ids, |p| {
            let v = p - a;
            a + d * (2.0 * v.dot(d)) - v
        })
    }

    pub fn move_entities(&mut self, ids: &[EntityId], delta: DVec2, copy: bool) -> Vec<EntityId> {
        if copy {
            return self.duplicate(ids, |p| p + delta);
        }
        for pid in self.points_of(ids) {
            if let Entity::Point { pos, .. } = &mut self.entities[pid] {
                *pos += delta;
            }
        }
        ids.to_vec()
    }

    pub fn scale_entities(&mut self, ids: &[EntityId], center: DVec2, factor: f64, copy: bool) -> Vec<EntityId> {
        let f = move |p: DVec2| center + (p - center) * factor;
        if copy {
            let out = self.duplicate(ids, f);
            self.scale_radii(&out, factor);
            return out;
        }
        for pid in self.points_of(ids) {
            if let Entity::Point { pos, .. } = &mut self.entities[pid] {
                *pos = f(*pos);
            }
        }
        self.scale_radii(ids, factor);
        ids.to_vec()
    }

    fn scale_radii(&mut self, ids: &[EntityId], factor: f64) {
        for &id in ids {
            match self.entities.get_mut(id) {
                Some(Entity::Circle { radius, .. }) => *radius *= factor,
                Some(Entity::Ellipse { rx, ry, .. }) => {
                    *rx *= factor;
                    *ry *= factor;
                }
                _ => {}
            }
        }
    }

    pub fn pattern_rect(
        &mut self,
        ids: &[EntityId],
        step1: DVec2,
        n1: usize,
        step2: DVec2,
        n2: usize,
    ) -> Vec<EntityId> {
        let mut out = Vec::new();
        for i in 0..n1.max(1) {
            for j in 0..n2.max(1) {
                if i == 0 && j == 0 {
                    continue;
                }
                let d = step1 * i as f64 + step2 * j as f64;
                out.extend(self.duplicate(ids, |p| p + d));
            }
        }
        out
    }

    pub fn pattern_circ(&mut self, ids: &[EntityId], center: DVec2, count: usize, total_angle: f64) -> Vec<EntityId> {
        let n = count.max(1);
        let full = (total_angle.abs() - std::f64::consts::TAU).abs() < 1e-9;
        let step = if full {
            total_angle / n as f64
        } else if n > 1 {
            total_angle / (n - 1) as f64
        } else {
            0.0
        };
        let mut out = Vec::new();
        for i in 1..n {
            let (s, c) = (step * i as f64).sin_cos();
            out.extend(self.duplicate(ids, |p| {
                let v = p - center;
                center + DVec2::new(v.x * c - v.y * s, v.x * s + v.y * c)
            }));
        }
        out
    }

    /// All point ids used by the given entities, plus points in the list.
    pub fn points_of(&self, ids: &[EntityId]) -> Vec<EntityId> {
        let mut out = Vec::new();
        for &id in ids {
            if let Some(e) = self.entities.get(id) {
                if matches!(e, Entity::Point { .. }) {
                    out.push(id);
                }
                out.extend(e.point_refs());
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// Fillet the corner where two lines share an end point. Trims both
    /// lines and inserts a tangent arc. Returns the arc id.
    pub fn fillet_lines(&mut self, l1: EntityId, l2: EntityId, radius: f64) -> Result<EntityId, String> {
        let (a1, b1) = match self.entities.get(l1) {
            Some(Entity::Line { a, b, .. }) => (*a, *b),
            _ => return Err("first pick is not a line".into()),
        };
        let (a2, b2) = match self.entities.get(l2) {
            Some(Entity::Line { a, b, .. }) => (*a, *b),
            _ => return Err("second pick is not a line".into()),
        };
        // Find the shared corner, by id or by location.
        let corner_pairs = [(a1, a2), (a1, b2), (b1, a2), (b1, b2)];
        let shared = corner_pairs
            .iter()
            .find(|(p, q)| p == q || (self.point(*p) - self.point(*q)).length() < 1e-6)
            .copied()
            .ok_or("lines do not meet at a corner")?;
        let (c1, c2) = shared;
        let far1 = if c1 == a1 { b1 } else { a1 };
        let far2 = if c2 == a2 { b2 } else { a2 };
        let p = self.point(c1);
        let d1 = (self.point(far1) - p).normalize_or_zero();
        let d2 = (self.point(far2) - p).normalize_or_zero();
        let cos_t = d1.dot(d2).clamp(-1.0, 1.0);
        let theta = cos_t.acos();
        if theta < 1e-6 || (std::f64::consts::PI - theta) < 1e-6 {
            return Err("lines are collinear".into());
        }
        let t = radius / (theta / 2.0).tan();
        if t > (self.point(far1) - p).length() || t > (self.point(far2) - p).length() {
            return Err("radius too large for these lines".into());
        }
        let t1 = p + d1 * t;
        let t2 = p + d2 * t;
        let bis = (d1 + d2).normalize_or_zero();
        let center = p + bis * (radius / (theta / 2.0).sin());
        let np1 = self.add_point(t1.x, t1.y);
        let np2 = self.add_point(t2.x, t2.y);
        let cp = self.add_point(center.x, center.y);
        // Rewire the lines to the tangent points.
        if let Some(Entity::Line { a, b, .. }) = self.entities.get_mut(l1) {
            if *a == c1 {
                *a = np1
            } else {
                *b = np1
            }
        }
        if let Some(Entity::Line { a, b, .. }) = self.entities.get_mut(l2) {
            if *a == c2 {
                *a = np2
            } else {
                *b = np2
            }
        }
        // CCW arc from t1 to t2 or t2 to t1.
        let ccw = (t1 - center).perp_dot(t2 - center) > 0.0;
        let arc = if ccw { self.add_arc(cp, np1, np2) } else { self.add_arc(cp, np2, np1) };
        self.constrain(Constraint::Tangent(l1, arc));
        self.constrain(Constraint::Tangent(l2, arc));
        // Drop the old corner point if nothing else uses it.
        let used = self.entities.values().any(|e| e.point_refs().contains(&c1) || e.point_refs().contains(&c2));
        if !used {
            self.entities.remove(c1);
            if c2 != c1 {
                self.entities.remove(c2);
            }
            self.constraints.retain(|_, c| !c.refs().contains(&c1) && !c.refs().contains(&c2));
        }
        Ok(arc)
    }

    /// Chamfer the corner where two lines meet: trims both lines back by
    /// `d` and joins the new ends with a line. Returns the new line.
    pub fn chamfer_lines(&mut self, l1: EntityId, l2: EntityId, d: f64) -> Result<EntityId, String> {
        let ends = |s: &Sketch, id: EntityId| match s.entities.get(id) {
            Some(Entity::Line { a, b, .. }) => Some((*a, *b)),
            _ => None,
        };
        let (a1, b1) = ends(self, l1).ok_or("first pick is not a line")?;
        let (a2, b2) = ends(self, l2).ok_or("second pick is not a line")?;
        let pairs = [(a1, a2), (a1, b2), (b1, a2), (b1, b2)];
        let (c1, c2) = pairs
            .iter()
            .find(|(p, q)| p == q || (self.point(*p) - self.point(*q)).length() < 1e-6)
            .copied()
            .ok_or("lines do not meet at a corner")?;
        let far1 = if c1 == a1 { b1 } else { a1 };
        let far2 = if c2 == a2 { b2 } else { a2 };
        let p = self.point(c1);
        let v1 = self.point(far1) - p;
        let v2 = self.point(far2) - p;
        if d <= 0.0 || d >= v1.length() || d >= v2.length() {
            return Err("chamfer distance must be positive and shorter than both lines".into());
        }
        let t1 = p + v1.normalize() * d;
        let t2 = p + v2.normalize() * d;
        let n1 = self.add_point(t1.x, t1.y);
        let n2 = self.add_point(t2.x, t2.y);
        if let Some(Entity::Line { a, b, .. }) = self.entities.get_mut(l1) {
            if *a == c1 {
                *a = n1
            } else {
                *b = n1
            }
        }
        if let Some(Entity::Line { a, b, .. }) = self.entities.get_mut(l2) {
            if *a == c2 {
                *a = n2
            } else {
                *b = n2
            }
        }
        let used = self.entities.values().any(|e| e.point_refs().contains(&c1) || e.point_refs().contains(&c2));
        if !used {
            self.entities.remove(c1);
            if c2 != c1 {
                self.entities.remove(c2);
            }
            self.constraints.retain(|_, c| !c.refs().contains(&c1) && !c.refs().contains(&c2));
        }
        Ok(self.add_line(n1, n2))
    }

    /// Arc that starts at an existing end point, tangent to the line or arc
    /// that ends there, and finishes at `end`.
    pub fn add_tangent_arc(&mut self, start: EntityId, end: DVec2) -> Result<EntityId, String> {
        let s0 = self.point(start);
        // Incoming direction at `start`, pointing along the curve into the point.
        let mut dir: Option<DVec2> = None;
        for e in self.entities.values() {
            match e {
                Entity::Line { a, b, .. } if *a == start => dir = Some(s0 - self.point(*b)),
                Entity::Line { a, b, .. } if *b == start => dir = Some(s0 - self.point(*a)),
                Entity::Arc { center, start: st, end: en } if *en == start => {
                    let r = s0 - self.point(*center);
                    dir = Some(DVec2::new(-r.y, r.x));
                    let _ = st;
                }
                Entity::Arc { center, start: st, .. } if *st == start => {
                    let r = s0 - self.point(*center);
                    dir = Some(DVec2::new(r.y, -r.x));
                }
                _ => {}
            }
            if dir.is_some() {
                break;
            }
        }
        let t = dir.ok_or("the start point is not the end of a line or arc")?.normalize_or_zero();
        let chord = end - s0;
        let nrm = DVec2::new(-t.y, t.x);
        let denom = 2.0 * chord.dot(nrm);
        if denom.abs() < 1e-12 {
            return Err("the end point is straight ahead; use a line".into());
        }
        let r = chord.length_squared() / denom; // signed radius along nrm
        let c = s0 + nrm * r;
        let cp = self.add_point(c.x, c.y);
        let ep = self.add_point(end.x, end.y);
        // Counter-clockwise when the centre is to the left of the tangent.
        Ok(if r > 0.0 { self.add_arc(cp, start, ep) } else { self.add_arc(cp, ep, start) })
    }

    /// Parameters along line `id` (0..1) where other entities cross it.
    fn crossings(&self, id: EntityId) -> Vec<f64> {
        let (a, b) = match &self.entities[id] {
            Entity::Line { a, b, .. } => (self.point(*a), self.point(*b)),
            _ => return vec![],
        };
        let ab = b - a;
        let mut ts = Vec::new();
        for (oid, e) in &self.entities {
            if oid == id {
                continue;
            }
            match e {
                Entity::Line { a: c, b: d, .. } => {
                    let (c, d) = (self.point(*c), self.point(*d));
                    let cd = d - c;
                    let den = ab.perp_dot(cd);
                    if den.abs() < 1e-12 {
                        continue;
                    }
                    let t = (c - a).perp_dot(cd) / den;
                    let u = (c - a).perp_dot(ab) / den;
                    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                        ts.push(t);
                    }
                }
                Entity::Point { .. } => {}
                _ => {
                    // Curves: sample and intersect segment-by-segment.
                    let pts = self.sample(oid);
                    for w in pts.windows(2) {
                        let cd = w[1] - w[0];
                        let den = ab.perp_dot(cd);
                        if den.abs() < 1e-12 {
                            continue;
                        }
                        let t = (w[0] - a).perp_dot(cd) / den;
                        let u = (w[0] - a).perp_dot(ab) / den;
                        if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                            ts.push(t);
                        }
                    }
                }
            }
        }
        ts.retain(|t| *t > 1e-9 && *t < 1.0 - 1e-9);
        ts.sort_by(|x, y| x.partial_cmp(y).unwrap());
        ts.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
        ts
    }

    /// Remove the piece of line `id` under `click`, between the nearest
    /// crossings with other entities.
    pub fn trim_line(&mut self, id: EntityId, click: DVec2) -> Result<(), String> {
        let (pa, pb, a, b) = match self.entities.get(id) {
            Some(Entity::Line { a, b, .. }) => (*a, *b, self.point(*a), self.point(*b)),
            _ => return Err("trim works on lines".into()),
        };
        let ab = b - a;
        let tc = ((click - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
        let ts = self.crossings(id);
        let lo = ts.iter().copied().filter(|t| *t < tc).fold(0.0, f64::max);
        let hi = ts.iter().copied().filter(|t| *t > tc).fold(1.0, f64::min);
        if lo == 0.0 && hi == 1.0 {
            // Nothing crosses: delete the whole line.
            self.remove_entity(id);
            return Ok(());
        }
        let construction = matches!(self.entities[id], Entity::Line { construction: true, .. });
        self.remove_entity(id);
        if lo > 0.0 {
            let p = a + ab * lo;
            let np = self.add_point(p.x, p.y);
            let l = self.add_line(pa, np);
            if construction {
                if let Entity::Line { construction: c, .. } = &mut self.entities[l] {
                    *c = true;
                }
            }
        }
        if hi < 1.0 {
            let p = a + ab * hi;
            let np = self.add_point(p.x, p.y);
            let l = self.add_line(np, pb);
            if construction {
                if let Entity::Line { construction: c, .. } = &mut self.entities[l] {
                    *c = true;
                }
            }
        }
        Ok(())
    }

    /// Extend the end of line `id` nearer to `click` until it meets another
    /// entity along its own direction.
    pub fn extend_line(&mut self, id: EntityId, click: DVec2) -> Result<(), String> {
        let (pa, pb, a, b) = match self.entities.get(id) {
            Some(Entity::Line { a, b, .. }) => (*a, *b, self.point(*a), self.point(*b)),
            _ => return Err("extend works on lines".into()),
        };
        let near_b = (click - b).length() < (click - a).length();
        let (from, dir, moving) =
            if near_b { (b, (b - a).normalize_or_zero(), pb) } else { (a, (a - b).normalize_or_zero(), pa) };
        let mut best: Option<f64> = None;
        for (oid, _) in &self.entities {
            if oid == id {
                continue;
            }
            let pts = self.sample(oid);
            if pts.len() < 2 {
                continue;
            }
            for w in pts.windows(2) {
                let cd = w[1] - w[0];
                let den = dir.perp_dot(cd);
                if den.abs() < 1e-12 {
                    continue;
                }
                let t = (w[0] - from).perp_dot(cd) / den;
                let u = (w[0] - from).perp_dot(dir) / den;
                if t > 1e-6 && (0.0..=1.0).contains(&u) && best.is_none_or(|bt| t < bt) {
                    best = Some(t);
                }
            }
        }
        let t = best.ok_or("nothing to extend to")?;
        if let Entity::Point { pos, .. } = &mut self.entities[moving] {
            *pos = from + dir * t;
        }
        Ok(())
    }

    /// Offset selected entities by `d`. Closed loops of lines offset as a
    /// polygon (positive grows). Single lines get a parallel copy on the
    /// left for positive `d`. Circles and arcs change radius.
    pub fn offset_entities(&mut self, ids: &[EntityId], d: f64) -> Vec<EntityId> {
        let mut out = Vec::new();
        let lines: Vec<EntityId> =
            ids.iter().copied().filter(|&i| matches!(self.entities.get(i), Some(Entity::Line { .. }))).collect();
        let mut handled: Vec<EntityId> = Vec::new();
        // Try to walk the lines into one closed loop.
        if lines.len() >= 3 {
            let mut chain: Vec<DVec2> = Vec::new();
            let mut used = vec![false; lines.len()];
            let (a0, b0) = self.line_ends(lines[0]);
            chain.push(a0);
            chain.push(b0);
            used[0] = true;
            let mut closed = false;
            loop {
                let last = *chain.last().unwrap();
                let mut found = false;
                for (k, &l) in lines.iter().enumerate() {
                    if used[k] {
                        continue;
                    }
                    let (a, b) = self.line_ends(l);
                    if (a - last).length() < 1e-6 {
                        chain.push(b);
                        used[k] = true;
                        found = true;
                        break;
                    }
                    if (b - last).length() < 1e-6 {
                        chain.push(a);
                        used[k] = true;
                        found = true;
                        break;
                    }
                }
                if !found {
                    break;
                }
                if (chain.last().unwrap() - chain[0]).length() < 1e-6 {
                    chain.pop();
                    closed = true;
                    break;
                }
            }
            if closed && used.iter().all(|u| *u) {
                let poly = anvil_math_offset(&chain, d);
                let ids: Vec<EntityId> = poly.iter().map(|p| self.add_point(p.x, p.y)).collect();
                for i in 0..ids.len() {
                    out.push(self.add_line(ids[i], ids[(i + 1) % ids.len()]));
                }
                handled.extend(lines.iter());
            }
        }
        for &id in ids {
            if handled.contains(&id) {
                continue;
            }
            match self.entities.get(id).cloned() {
                Some(Entity::Line { a, b, .. }) => {
                    let (pa, pb) = (self.point(a), self.point(b));
                    let dir = (pb - pa).normalize_or_zero();
                    let n = DVec2::new(-dir.y, dir.x) * d;
                    let na = self.add_point(pa.x + n.x, pa.y + n.y);
                    let nb = self.add_point(pb.x + n.x, pb.y + n.y);
                    out.push(self.add_line(na, nb));
                }
                Some(Entity::Circle { center, radius }) => {
                    let c = self.point(center);
                    let nc = self.add_point(c.x, c.y);
                    out.push(self.add_circle(nc, (radius + d).max(1e-6)));
                }
                Some(Entity::Arc { center, start, end }) => {
                    let c = self.point(center);
                    let r = (self.point(start) - c).length();
                    let f = ((r + d).max(1e-6)) / r;
                    let s = c + (self.point(start) - c) * f;
                    let e = c + (self.point(end) - c) * f;
                    out.push(self.add_arc_center(c, s, e));
                }
                _ => {}
            }
        }
        out
    }

    fn line_ends(&self, id: EntityId) -> (DVec2, DVec2) {
        match &self.entities[id] {
            Entity::Line { a, b, .. } => (self.point(*a), self.point(*b)),
            _ => (DVec2::ZERO, DVec2::ZERO),
        }
    }

    /// Project 3D segments that lie in the sketch plane into fixed lines.
    /// Returns how many were added.
    pub fn project_segments(&mut self, segs: &[[DVec3; 2]], tol: f64) -> usize {
        let n = self.plane.normal();
        let mut count = 0;
        let mut nodes: Vec<(DVec2, EntityId)> = Vec::new();
        for [p, q] in segs {
            let dp = (*p - self.plane.origin).dot(n).abs();
            let dq = (*q - self.plane.origin).dot(n).abs();
            if dp > tol || dq > tol {
                continue;
            }
            let (a, b) = (self.plane.to_local(*p), self.plane.to_local(*q));
            if (a - b).length() < 1e-9 {
                continue;
            }
            // Skip if an identical line already exists.
            let exists = self.entities.values().any(|e| match e {
                Entity::Line { a: la, b: lb, .. } => {
                    let (pa, pb) = (self.point(*la), self.point(*lb));
                    ((pa - a).length() < 1e-6 && (pb - b).length() < 1e-6)
                        || ((pa - b).length() < 1e-6 && (pb - a).length() < 1e-6)
                }
                _ => false,
            });
            if exists {
                continue;
            }
            let mut node = |p: DVec2, sk: &mut Sketch| -> EntityId {
                if let Some((_, id)) = nodes.iter().find(|(q, _)| (*q - p).length() < 1e-6) {
                    return *id;
                }
                let id = sk.add_point(p.x, p.y);
                sk.constrain(Constraint::Fix(id));
                nodes.push((p, id));
                id
            };
            let ia = node(a, self);
            let ib = node(b, self);
            // Projected edges are reference geometry: they snap and
            // constrain but do not form profiles. Toggle Construction on
            // a projected line to use it in a profile.
            let l = self.add_line(ia, ib);
            if let Entity::Line { construction, .. } = &mut self.entities[l] {
                *construction = true;
            }
            count += 1;
        }
        count
    }

    /// Nearest point on any entity to `p` within `tol`, for snapping.
    pub fn nearest_on_curve(&self, p: DVec2, tol: f64) -> Option<DVec2> {
        let mut best: Option<(f64, DVec2)> = None;
        for (id, e) in &self.entities {
            if matches!(e, Entity::Point { .. }) {
                continue;
            }
            let pts = self.sample(id);
            for w in pts.windows(2) {
                let ab = w[1] - w[0];
                let t = if ab.length_squared() < 1e-18 {
                    0.0
                } else {
                    ((p - w[0]).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
                };
                let q = w[0] + ab * t;
                let d = dist_to_segment(p, w[0], w[1]);
                if d <= tol && best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, q));
                }
            }
        }
        best.map(|b| b.1)
    }
}

/// Bisector offset of a closed polygon (positive grows a CCW polygon).
fn anvil_math_offset(poly: &[DVec2], d: f64) -> Vec<DVec2> {
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
        let n1 = DVec2::new(d1.y, -d1.x) * sign;
        let n2 = DVec2::new(d2.y, -d2.x) * sign;
        let bis = (n1 + n2).normalize_or_zero();
        let cos_half = bis.dot(n1).max(0.2);
        out.push(p1 + bis * (d / cos_half));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_math::Plane;

    #[test]
    fn fillet_square_corner_keeps_profile_closed() {
        let mut s = Sketch::new(Plane::XY);
        let [l0, l1, ..] = s.add_rectangle(0.0, 0.0, 10.0, 10.0);
        s.fillet_lines(l0, l1, 2.0).unwrap();
        let p = s.profiles();
        assert_eq!(p.len(), 1);
        let exact = 100.0 - 4.0 + std::f64::consts::PI;
        assert!((p[0].signed_area() - exact).abs() < 0.05, "{}", p[0].signed_area());
    }

    #[test]
    fn trim_removes_middle_piece() {
        let mut s = Sketch::new(Plane::XY);
        let a = s.add_point(0.0, 0.0);
        let b = s.add_point(10.0, 0.0);
        let l = s.add_line(a, b);
        let c = s.add_point(3.0, -1.0);
        let d = s.add_point(3.0, 1.0);
        s.add_line(c, d);
        let e = s.add_point(7.0, -1.0);
        let f = s.add_point(7.0, 1.0);
        s.add_line(e, f);
        s.trim_line(l, DVec2::new(5.0, 0.0)).unwrap();
        let lines: Vec<_> = s.entities.values().filter(|e| matches!(e, Entity::Line { .. })).collect();
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn extend_reaches_the_crossing_line() {
        let mut s = Sketch::new(Plane::XY);
        let a = s.add_point(0.0, 0.0);
        let b = s.add_point(5.0, 0.0);
        let l = s.add_line(a, b);
        let c = s.add_point(8.0, -3.0);
        let d = s.add_point(8.0, 3.0);
        s.add_line(c, d);
        s.extend_line(l, DVec2::new(5.0, 0.0)).unwrap();
        assert!((s.point(b).x - 8.0).abs() < 1e-9);
    }

    #[test]
    fn mirror_and_patterns_duplicate() {
        let mut s = Sketch::new(Plane::XY);
        let lines = s.add_rectangle(1.0, 1.0, 3.0, 2.0).to_vec();
        let m = s.mirror_entities(&lines, DVec2::ZERO, DVec2::Y);
        assert_eq!(m.len(), 4);
        assert_eq!(s.profiles().len(), 2);
        s.pattern_circ(&lines, DVec2::ZERO, 4, std::f64::consts::TAU);
        assert_eq!(s.profiles().len(), 5);
    }

    #[test]
    fn offset_closed_loop_and_projection() {
        let mut s = Sketch::new(Plane::XY);
        let lines = s.add_rectangle(0.0, 0.0, 10.0, 10.0).to_vec();
        s.offset_entities(&lines, 1.0);
        let areas: Vec<f64> = s.profiles().iter().map(|p| p.signed_area()).collect();
        assert!(areas.iter().any(|a| (a - 144.0).abs() < 1e-6), "{areas:?}");
        let n = s.project_segments(
            &[
                [DVec3::new(0.0, 0.0, 0.0), DVec3::new(20.0, 0.0, 0.0)],
                [DVec3::new(0.0, 0.0, 5.0), DVec3::new(1.0, 0.0, 5.0)],
            ],
            1e-6,
        );
        assert_eq!(n, 1);
    }

    #[test]
    fn ellipse_and_spline_profiles() {
        let mut s = Sketch::new(Plane::XY);
        s.add_ellipse(DVec2::ZERO, 10.0, 5.0, 0.3);
        s.add_spline(&[DVec2::new(20.0, 0.0), DVec2::new(30.0, 5.0), DVec2::new(25.0, 12.0)], true);
        let p = s.profiles();
        assert_eq!(p.len(), 2);
        assert!(
            (p[0].signed_area().abs() - std::f64::consts::PI * 50.0).abs() < 1.0
                || (p[1].signed_area().abs() - std::f64::consts::PI * 50.0).abs() < 1.0
        );
    }

    #[test]
    fn chamfer_and_tangent_arc() {
        let mut s = Sketch::new(Plane::XY);
        let [l0, l1, ..] = s.add_rectangle(0.0, 0.0, 10.0, 10.0);
        s.chamfer_lines(l0, l1, 2.0).unwrap();
        let p = s.profiles();
        assert_eq!(p.len(), 1);
        assert!((p[0].signed_area() - 98.0).abs() < 1e-9, "{}", p[0].signed_area());

        let mut s = Sketch::new(Plane::XY);
        let a = s.add_point(0.0, 0.0);
        let b = s.add_point(10.0, 0.0);
        s.add_line(a, b);
        let arc = s.add_tangent_arc(b, DVec2::new(10.0, 10.0)).unwrap();
        match s.entities[arc] {
            Entity::Arc { center, .. } => assert!((s.point(center) - DVec2::new(10.0, 5.0)).length() < 1e-9),
            _ => panic!(),
        }
    }
}
