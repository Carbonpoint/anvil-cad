//! Numeric constraint solver.
//!
//! Method: damped Newton-Raphson (Levenberg-Marquardt style damping) on the
//! stacked residual vector. The Jacobian is built by central finite
//! differences. This is simple and robust for the sketch sizes we target
//! now. A graph-decomposition front end can be added later without changing
//! the public API.

use crate::{Constraint, Entity, EntityId, Sketch};
use anvil_math::DVec2;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolveStatus {
    /// All residuals below tolerance.
    Converged,
    /// Iteration limit reached with residuals still above tolerance.
    NotConverged,
    /// Nothing to solve (no constraints or no free variables).
    Trivial,
}

#[derive(Clone, Debug)]
pub struct SolveReport {
    pub status: SolveStatus,
    pub iterations: usize,
    pub residual: f64,
    /// Free variables minus equations. Positive means under-constrained.
    pub dof: isize,
}

const MAX_ITER: usize = 100;
const TOL: f64 = 1e-10;

/// Collects every free scalar variable of the sketch into one vector.
struct VarMap {
    /// (entity, slot) -> index into x. slot 0/1 = point x/y, slot 2 = radius.
    index: HashMap<(EntityId, u8), usize>,
    x: Vec<f64>,
}

impl VarMap {
    fn build(s: &Sketch) -> Self {
        let mut index = HashMap::new();
        let mut x = Vec::new();
        let fixed: Vec<EntityId> =
            s.constraints.values().filter_map(|c| if let Constraint::Fix(p) = c { Some(*p) } else { None }).collect();
        for (id, e) in &s.entities {
            match e {
                Entity::Point { pos, fixed: f } => {
                    if *f || fixed.contains(&id) {
                        continue;
                    }
                    index.insert((id, 0), x.len());
                    x.push(pos.x);
                    index.insert((id, 1), x.len());
                    x.push(pos.y);
                }
                Entity::Circle { radius, .. } => {
                    index.insert((id, 2), x.len());
                    x.push(*radius);
                }
                _ => {}
            }
        }
        VarMap { index, x }
    }

    fn write_back(&self, s: &mut Sketch) {
        for ((id, slot), i) in &self.index {
            match (&mut s.entities[*id], slot) {
                (Entity::Point { pos, .. }, 0) => pos.x = self.x[*i],
                (Entity::Point { pos, .. }, 1) => pos.y = self.x[*i],
                (Entity::Circle { radius, .. }, 2) => *radius = self.x[*i],
                _ => {}
            }
        }
    }
}

/// Read-only view used while evaluating residuals against a trial `x`.
struct Eval<'a> {
    s: &'a Sketch,
    vm: &'a VarMap,
    x: &'a [f64],
}

impl Eval<'_> {
    fn pt(&self, id: EntityId) -> DVec2 {
        let base = match &self.s.entities[id] {
            Entity::Point { pos, .. } => *pos,
            _ => panic!("not a point"),
        };
        let x = self.vm.index.get(&(id, 0)).map_or(base.x, |i| self.x[*i]);
        let y = self.vm.index.get(&(id, 1)).map_or(base.y, |i| self.x[*i]);
        DVec2::new(x, y)
    }
    fn line(&self, id: EntityId) -> (DVec2, DVec2) {
        match &self.s.entities[id] {
            Entity::Line { a, b, .. } => (self.pt(*a), self.pt(*b)),
            _ => panic!("not a line"),
        }
    }
    fn circle(&self, id: EntityId) -> (DVec2, f64) {
        match &self.s.entities[id] {
            Entity::Circle { center, radius } => {
                let r = self.vm.index.get(&(id, 2)).map_or(*radius, |i| self.x[*i]);
                (self.pt(*center), r)
            }
            Entity::Arc { center, start, .. } => {
                let c = self.pt(*center);
                (c, (self.pt(*start) - c).length())
            }
            _ => panic!("not a circle or arc"),
        }
    }

    fn residuals(&self, out: &mut Vec<f64>) {
        out.clear();
        for c in self.s.constraints.values() {
            match c {
                Constraint::Coincident(a, b) => {
                    let d = self.pt(*a) - self.pt(*b);
                    out.push(d.x);
                    out.push(d.y);
                }
                Constraint::Horizontal(l) => {
                    let (a, b) = self.line(*l);
                    out.push(a.y - b.y);
                }
                Constraint::Vertical(l) => {
                    let (a, b) = self.line(*l);
                    out.push(a.x - b.x);
                }
                Constraint::Distance(a, b, d) => {
                    out.push((self.pt(*a) - self.pt(*b)).length() - d);
                }
                Constraint::Length(l, d) => {
                    let (a, b) = self.line(*l);
                    out.push((a - b).length() - d);
                }
                Constraint::Parallel(l1, l2) => {
                    let (a, b) = self.line(*l1);
                    let (c, d) = self.line(*l2);
                    out.push((b - a).perp_dot(d - c));
                }
                Constraint::Perpendicular(l1, l2) => {
                    let (a, b) = self.line(*l1);
                    let (c, d) = self.line(*l2);
                    out.push((b - a).dot(d - c));
                }
                Constraint::EqualLength(l1, l2) => {
                    let (a, b) = self.line(*l1);
                    let (c, d) = self.line(*l2);
                    out.push((b - a).length() - (d - c).length());
                }
                Constraint::Radius(cid, r) => {
                    let (_, cr) = self.circle(*cid);
                    out.push(cr - r);
                }
                Constraint::PointOnLine(p, l) => {
                    let (a, b) = self.line(*l);
                    let dir = (b - a).normalize_or_zero();
                    out.push(dir.perp_dot(self.pt(*p) - a));
                }
                Constraint::PointOnCircle(p, cid) => {
                    let (c, r) = self.circle(*cid);
                    out.push((self.pt(*p) - c).length() - r);
                }
                Constraint::EqualRadius(c1, c2) => {
                    out.push(self.circle(*c1).1 - self.circle(*c2).1);
                }
                Constraint::Concentric(c1, c2) => {
                    let d = self.circle(*c1).0 - self.circle(*c2).0;
                    out.push(d.x);
                    out.push(d.y);
                }
                Constraint::Midpoint(p, l) => {
                    let (a, b) = self.line(*l);
                    let d = self.pt(*p) - (a + b) * 0.5;
                    out.push(d.x);
                    out.push(d.y);
                }
                Constraint::Tangent(l, cid) => {
                    let (a, b) = self.line(*l);
                    let (c, r) = self.circle(*cid);
                    let dir = (b - a).normalize_or_zero();
                    out.push(dir.perp_dot(c - a).abs() - r);
                }
                Constraint::Angle(l1, l2, deg) => {
                    let (a, b) = self.line(*l1);
                    let (c, d) = self.line(*l2);
                    let u = (b - a).normalize_or_zero();
                    let v = (d - c).normalize_or_zero();
                    let ang = u.perp_dot(v).atan2(u.dot(v));
                    out.push(ang - deg.to_radians());
                }
                Constraint::Symmetric(p1, p2, l) => {
                    let (a, b) = self.line(*l);
                    let dir = (b - a).normalize_or_zero();
                    let m = (self.pt(*p1) + self.pt(*p2)) * 0.5;
                    let d = self.pt(*p2) - self.pt(*p1);
                    // Midpoint on the line, and the join perpendicular to it.
                    out.push(dir.perp_dot(m - a));
                    out.push(dir.dot(d));
                }
                Constraint::Collinear(l1, l2) => {
                    let (a, b) = self.line(*l1);
                    let (c, d) = self.line(*l2);
                    let dir = (b - a).normalize_or_zero();
                    out.push(dir.perp_dot(c - a));
                    out.push(dir.perp_dot(d - a));
                }
                Constraint::Fix(_) => {}
                Constraint::FixX(p, v) => out.push(self.pt(*p).x - v),
                Constraint::FixY(p, v) => out.push(self.pt(*p).y - v),
            }
        }
        // Implicit: an arc's end point is at the same radius as its start.
        for e in self.s.entities.values() {
            if let Entity::Arc { center, start, end } = e {
                let c = self.pt(*center);
                out.push((self.pt(*start) - c).length() - (self.pt(*end) - c).length());
            }
        }
    }
}

pub fn solve(s: &mut Sketch) -> SolveReport {
    let mut vm = VarMap::build(s);
    let n = vm.x.len();
    let mut r = Vec::new();
    {
        let x = vm.x.clone();
        Eval { s, vm: &vm, x: &x }.residuals(&mut r);
    }
    let m = r.len();
    let dof = n as isize - m as isize;
    if n == 0 || m == 0 {
        return SolveReport { status: SolveStatus::Trivial, iterations: 0, residual: norm(&r), dof };
    }

    let mut lambda = 1e-3;
    let mut iter = 0;
    let mut jac = vec![0.0; m * n];
    let mut r_trial = Vec::new();
    while iter < MAX_ITER {
        let res = norm(&r);
        if res < TOL {
            break;
        }
        // Jacobian by central differences.
        for j in 0..n {
            let h = 1e-7 * (1.0 + vm.x[j].abs());
            let mut xp = vm.x.clone();
            xp[j] += h;
            let mut xm = vm.x.clone();
            xm[j] -= h;
            let mut rp = Vec::new();
            let mut rm = Vec::new();
            Eval { s, vm: &vm, x: &xp }.residuals(&mut rp);
            Eval { s, vm: &vm, x: &xm }.residuals(&mut rm);
            for i in 0..m {
                jac[i * n + j] = (rp[i] - rm[i]) / (2.0 * h);
            }
        }
        // Solve (J^T J + lambda I) dx = -J^T r
        let mut a = vec![0.0; n * n];
        let mut b = vec![0.0; n];
        for i in 0..m {
            for j in 0..n {
                b[j] -= jac[i * n + j] * r[i];
                for k in 0..n {
                    a[j * n + k] += jac[i * n + j] * jac[i * n + k];
                }
            }
        }
        for j in 0..n {
            a[j * n + j] += lambda * (1.0 + a[j * n + j]);
        }
        let dx = match gauss_solve(a, b, n) {
            Some(d) => d,
            None => {
                lambda *= 10.0;
                iter += 1;
                continue;
            }
        };
        let x_trial: Vec<f64> = vm.x.iter().zip(&dx).map(|(x, d)| x + d).collect();
        Eval { s, vm: &vm, x: &x_trial }.residuals(&mut r_trial);
        if norm(&r_trial) < res {
            vm.x = x_trial;
            std::mem::swap(&mut r, &mut r_trial);
            lambda = (lambda * 0.3).max(1e-12);
        } else {
            lambda *= 10.0;
        }
        iter += 1;
    }
    vm.write_back(s);
    let residual = norm(&r);
    SolveReport {
        status: if residual < TOL { SolveStatus::Converged } else { SolveStatus::NotConverged },
        iterations: iter,
        residual,
        dof,
    }
}

fn norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// Gaussian elimination with partial pivoting. Returns None if singular.
fn gauss_solve(mut a: Vec<f64>, mut b: Vec<f64>, n: usize) -> Option<Vec<f64>> {
    for col in 0..n {
        let mut piv = col;
        for row in col + 1..n {
            if a[row * n + col].abs() > a[piv * n + col].abs() {
                piv = row;
            }
        }
        if a[piv * n + col].abs() < 1e-300 {
            return None;
        }
        if piv != col {
            for k in 0..n {
                a.swap(col * n + k, piv * n + k);
            }
            b.swap(col, piv);
        }
        for row in col + 1..n {
            let f = a[row * n + col] / a[col * n + col];
            if f == 0.0 {
                continue;
            }
            for k in col..n {
                a[row * n + k] -= f * a[col * n + k];
            }
            b[row] -= f * b[col];
        }
    }
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let mut s = b[row];
        for k in row + 1..n {
            s -= a[row * n + k] * x[k];
        }
        x[row] = s / a[row * n + row];
    }
    Some(x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_math::Plane;

    #[test]
    fn rectangle_with_dimensions_converges() {
        let mut s = Sketch::new(Plane::XY);
        let [l0, l1, _, _] = s.add_rectangle(0.0, 0.0, 3.0, 2.5);
        let p0 = match s.entities[l0] {
            Entity::Line { a, .. } => a,
            _ => unreachable!(),
        };
        s.constrain(Constraint::Fix(p0));
        s.constrain(Constraint::Length(l0, 40.0));
        s.constrain(Constraint::Length(l1, 20.0));
        let rep = s.solve();
        assert_eq!(rep.status, SolveStatus::Converged, "{rep:?}");
        let (a, b) = match s.entities[l0] {
            Entity::Line { a, b, .. } => (s.point(a), s.point(b)),
            _ => unreachable!(),
        };
        assert!(((b - a).length() - 40.0).abs() < 1e-8);
        assert_eq!(rep.dof, 0);
    }

    #[test]
    fn circle_radius_and_point_on_circle() {
        let mut s = Sketch::new(Plane::XY);
        let c = s.add_point(0.0, 0.0);
        let circ = s.add_circle(c, 1.0);
        let p = s.add_point(3.0, 4.0);
        s.constrain(Constraint::Fix(c));
        s.constrain(Constraint::Radius(circ, 10.0));
        s.constrain(Constraint::PointOnCircle(p, circ));
        s.constrain(Constraint::FixY(p, 0.0));
        let rep = s.solve();
        assert_eq!(rep.status, SolveStatus::Converged, "{rep:?}");
        assert!((s.point(p).x.abs() - 10.0).abs() < 1e-8);
    }
}
