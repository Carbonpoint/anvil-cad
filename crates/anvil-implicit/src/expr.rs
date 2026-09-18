//! A closed form expression tree for the fields that can be written
//! without loops or lookups: primitives, booleans, blends, offsets, and
//! the periodic sheets. It evaluates natively, and behind the `fidget`
//! cargo feature it compiles to Fidget's JIT for bulk evaluation and
//! meshes with Fidget's manifold dual contouring (milestone F5).

use crate::Field;
use anvil_math::DVec3;
use std::sync::Arc;

/// A node of the expression tree.
#[derive(Clone, Debug)]
pub enum Expr {
    X,
    Y,
    Z,
    Const(f64),
    Add(Arc<Expr>, Arc<Expr>),
    Sub(Arc<Expr>, Arc<Expr>),
    Mul(Arc<Expr>, Arc<Expr>),
    Div(Arc<Expr>, Arc<Expr>),
    Neg(Arc<Expr>),
    Sqrt(Arc<Expr>),
    Abs(Arc<Expr>),
    Min(Arc<Expr>, Arc<Expr>),
    Max(Arc<Expr>, Arc<Expr>),
    Sin(Arc<Expr>),
    Cos(Arc<Expr>),
}

impl Expr {
    pub fn x() -> Arc<Expr> {
        Arc::new(Expr::X)
    }
    pub fn y() -> Arc<Expr> {
        Arc::new(Expr::Y)
    }
    pub fn z() -> Arc<Expr> {
        Arc::new(Expr::Z)
    }
    pub fn c(v: f64) -> Arc<Expr> {
        Arc::new(Expr::Const(v))
    }

    /// Native evaluation.
    pub fn eval(&self, p: DVec3) -> f64 {
        match self {
            Expr::X => p.x,
            Expr::Y => p.y,
            Expr::Z => p.z,
            Expr::Const(v) => *v,
            Expr::Add(a, b) => a.eval(p) + b.eval(p),
            Expr::Sub(a, b) => a.eval(p) - b.eval(p),
            Expr::Mul(a, b) => a.eval(p) * b.eval(p),
            Expr::Div(a, b) => a.eval(p) / b.eval(p),
            Expr::Neg(a) => -a.eval(p),
            Expr::Sqrt(a) => a.eval(p).max(0.0).sqrt(),
            Expr::Abs(a) => a.eval(p).abs(),
            Expr::Min(a, b) => a.eval(p).min(b.eval(p)),
            Expr::Max(a, b) => a.eval(p).max(b.eval(p)),
            Expr::Sin(a) => a.eval(p).sin(),
            Expr::Cos(a) => a.eval(p).cos(),
        }
    }

    /// Number of nodes, counting shared ones once per reference.
    pub fn size(&self) -> usize {
        match self {
            Expr::X | Expr::Y | Expr::Z | Expr::Const(_) => 1,
            Expr::Add(a, b)
            | Expr::Sub(a, b)
            | Expr::Mul(a, b)
            | Expr::Div(a, b)
            | Expr::Min(a, b)
            | Expr::Max(a, b) => 1 + a.size() + b.size(),
            Expr::Neg(a) | Expr::Sqrt(a) | Expr::Abs(a) | Expr::Sin(a) | Expr::Cos(a) => 1 + a.size(),
        }
    }
}

/// Short constructors, as free functions on shared nodes.
pub mod build {
    use super::Expr;
    use anvil_math::DVec3;
    use std::sync::Arc;

    pub type E = Arc<Expr>;

    pub fn add(a: &E, b: &E) -> E {
        Arc::new(Expr::Add(a.clone(), b.clone()))
    }
    pub fn sub(a: &E, b: &E) -> E {
        Arc::new(Expr::Sub(a.clone(), b.clone()))
    }
    pub fn mul(a: &E, b: &E) -> E {
        Arc::new(Expr::Mul(a.clone(), b.clone()))
    }
    pub fn div(a: &E, b: &E) -> E {
        Arc::new(Expr::Div(a.clone(), b.clone()))
    }
    pub fn neg(a: &E) -> E {
        Arc::new(Expr::Neg(a.clone()))
    }
    pub fn sqrt(a: &E) -> E {
        Arc::new(Expr::Sqrt(a.clone()))
    }
    pub fn abs(a: &E) -> E {
        Arc::new(Expr::Abs(a.clone()))
    }
    pub fn min(a: &E, b: &E) -> E {
        Arc::new(Expr::Min(a.clone(), b.clone()))
    }
    pub fn max(a: &E, b: &E) -> E {
        Arc::new(Expr::Max(a.clone(), b.clone()))
    }
    pub fn sin(a: &E) -> E {
        Arc::new(Expr::Sin(a.clone()))
    }
    pub fn cos(a: &E) -> E {
        Arc::new(Expr::Cos(a.clone()))
    }
    pub fn scale(a: &E, k: f64) -> E {
        mul(a, &Expr::c(k))
    }
    pub fn shift(a: &E, k: f64) -> E {
        add(a, &Expr::c(k))
    }
    fn sq(a: &E) -> E {
        mul(a, a)
    }

    /// Sphere of radius `r` about `c`.
    pub fn sphere(c: DVec3, r: f64) -> E {
        let dx = shift(&Expr::x(), -c.x);
        let dy = shift(&Expr::y(), -c.y);
        let dz = shift(&Expr::z(), -c.z);
        shift(&sqrt(&add(&add(&sq(&dx), &sq(&dy)), &sq(&dz))), -r)
    }

    /// Axis aligned box, exact distance.
    pub fn boxed(lo: DVec3, hi: DVec3) -> E {
        let c = (lo + hi) * 0.5;
        let h = (hi - lo) * 0.5;
        let q = [
            shift(&abs(&shift(&Expr::x(), -c.x)), -h.x),
            shift(&abs(&shift(&Expr::y(), -c.y)), -h.y),
            shift(&abs(&shift(&Expr::z(), -c.z)), -h.z),
        ];
        let zero = Expr::c(0.0);
        let outside = sqrt(&add(&add(&sq(&max(&q[0], &zero)), &sq(&max(&q[1], &zero))), &sq(&max(&q[2], &zero))));
        let inside = min(&max(&max(&q[0], &q[1]), &q[2]), &zero);
        add(&outside, &inside)
    }

    pub fn union(a: &E, b: &E) -> E {
        min(a, b)
    }
    pub fn intersect(a: &E, b: &E) -> E {
        max(a, b)
    }
    pub fn subtract(a: &E, b: &E) -> E {
        max(a, &neg(b))
    }
    pub fn offset(a: &E, d: f64) -> E {
        shift(a, -d)
    }
    pub fn shell(a: &E, t: f64) -> E {
        shift(&abs(a), -0.5 * t)
    }

    /// Smooth union with blend width `k` (the quadratic polynomial form
    /// written without a clamp: `min(a, b) - h * h * k / 4` where
    /// `h = max(k - |a - b|, 0) / k`).
    pub fn smooth_union(a: &E, b: &E, k: f64) -> E {
        let h = scale(&max(&shift(&neg(&abs(&sub(a, b))), k), &Expr::c(0.0)), 1.0 / k);
        sub(&min(a, b), &scale(&sq(&h), 0.25 * k))
    }

    /// Gyroid sheet with `cell` and `wall`, normalised by a constant
    /// gradient of 1.2 (the analytic normalisation has no closed form
    /// without a division by a root; this one is within a few percent).
    pub fn gyroid(cell: f64, wall: f64) -> E {
        let w = std::f64::consts::TAU / cell;
        let (x, y, z) = (scale(&Expr::x(), w), scale(&Expr::y(), w), scale(&Expr::z(), w));
        let g = add(&add(&mul(&sin(&x), &cos(&y)), &mul(&sin(&y), &cos(&z))), &mul(&sin(&z), &cos(&x)));
        shift(&scale(&abs(&g), 1.0 / (1.2 * w)), -0.5 * wall)
    }

    /// Schwarz P sheet.
    pub fn schwarz(cell: f64, wall: f64) -> E {
        let w = std::f64::consts::TAU / cell;
        let g = add(&add(&cos(&scale(&Expr::x(), w)), &cos(&scale(&Expr::y(), w))), &cos(&scale(&Expr::z(), w)));
        shift(&scale(&abs(&g), 1.0 / (1.2 * w)), -0.5 * wall)
    }
}

impl Field for Expr {
    fn at(&self, p: DVec3) -> f64 {
        self.eval(p)
    }
}

#[cfg(feature = "fidget")]
pub mod jit {
    //! Fidget back end: compile an `Expr` to a JIT tape, evaluate many
    //! points at once, and mesh by manifold dual contouring.
    use super::Expr;
    use anvil_math::DVec3;
    use fidget::context::Tree;

    /// Convert to a Fidget tree.
    pub fn tree(e: &Expr) -> Tree {
        match e {
            Expr::X => Tree::x(),
            Expr::Y => Tree::y(),
            Expr::Z => Tree::z(),
            Expr::Const(v) => Tree::constant(*v),
            Expr::Add(a, b) => tree(a) + tree(b),
            Expr::Sub(a, b) => tree(a) - tree(b),
            Expr::Mul(a, b) => tree(a) * tree(b),
            Expr::Div(a, b) => tree(a) / tree(b),
            Expr::Neg(a) => -tree(a),
            Expr::Sqrt(a) => tree(a).sqrt(),
            Expr::Abs(a) => tree(a).abs(),
            Expr::Min(a, b) => tree(a).min(tree(b)),
            Expr::Max(a, b) => tree(a).max(tree(b)),
            Expr::Sin(a) => tree(a).sin(),
            Expr::Cos(a) => tree(a).cos(),
        }
    }

    /// A compiled expression with its JIT point evaluator.
    pub struct Compiled {
        shape: fidget::jit::JitShape,
    }

    impl Compiled {
        pub fn new(e: &Expr) -> Result<Compiled, String> {
            let t = tree(e);
            let shape = fidget::jit::JitShape::from(t);
            Ok(Compiled { shape })
        }

        /// Evaluate the field at every point, in one pass over the JIT.
        pub fn eval_many(&self, pts: &[DVec3]) -> Result<Vec<f64>, String> {
            use fidget::shape::EzShape;
            let tape = self.shape.ez_point_tape();
            let mut eval = fidget::jit::JitShape::new_point_eval();
            let mut out = Vec::with_capacity(pts.len());
            for p in pts {
                let (v, _) = eval.eval(&tape, p.x as f32, p.y as f32, p.z as f32).map_err(|e| e.to_string())?;
                out.push(v as f64);
            }
            Ok(out)
        }

        /// Mesh by Fidget's manifold dual contouring inside the box
        /// `lo..hi` at octree `depth` (cells per side are 2 to the depth).
        pub fn mesh(&self, lo: DVec3, hi: DVec3, depth: u8) -> Result<Vec<[DVec3; 3]>, String> {
            use fidget::mesh::{Octree, Settings};
            // Fidget meshes the [-1, 1] cube in model space; map the box onto it.
            // Fidget's `world_to_model` is applied to the points it samples
            // in its [-1, 1] cube, so it is the map from that cube into our
            // box: scale by the half size and move to the centre.
            let centre = (lo + hi) * 0.5;
            let half = ((hi - lo) * 0.5).max_element();
            let world_to_model = nalgebra::Matrix4::new(
                half as f32,
                0.0,
                0.0,
                centre.x as f32,
                0.0,
                half as f32,
                0.0,
                centre.y as f32,
                0.0,
                0.0,
                half as f32,
                centre.z as f32,
                0.0,
                0.0,
                0.0,
                1.0,
            );
            let bound = self.shape.clone().try_into().map_err(|_| "shape has free variables".to_string())?;
            let settings = Settings { depth, world_to_model, ..Default::default() };
            let octree = Octree::build(&bound, &settings).ok_or("meshing was cancelled")?;
            let mesh = octree.walk_dual();
            // Vertices come back in model space when the transform is
            // used; map them to world space if so.
            let in_model = mesh.vertices.iter().all(|v| v.x.abs() <= 1.001 && v.y.abs() <= 1.001 && v.z.abs() <= 1.001);
            let to_world = |q: nalgebra::Vector3<f32>| {
                let p = DVec3::new(q.x as f64, q.y as f64, q.z as f64);
                if in_model && half > 1.01 {
                    p * half + centre
                } else {
                    p
                }
            };
            let out: Vec<[DVec3; 3]> = mesh
                .triangles
                .iter()
                .map(|t| [to_world(mesh.vertices[t.x]), to_world(mesh.vertices[t.y]), to_world(mesh.vertices[t.z])])
                .collect();
            if out.is_empty() {
                return Err("fidget produced no triangles".into());
            }
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::build::*;
    use super::*;
    use crate::{BoxField, Sphere};

    #[test]
    fn expressions_match_the_native_fields() {
        let s = sphere(DVec3::new(1.0, 2.0, 3.0), 4.0);
        let sn = Sphere { c: DVec3::new(1.0, 2.0, 3.0), r: 4.0 };
        let b = boxed(DVec3::new(-2.0, -1.0, -3.0), DVec3::new(2.0, 1.0, 3.0));
        let bn = BoxField { lo: DVec3::new(-2.0, -1.0, -3.0), hi: DVec3::new(2.0, 1.0, 3.0) };
        let u = smooth_union(&s, &b, 1.5);
        let un = crate::SmoothUnion { a: &sn, b: &bn, k: 1.5 };
        for p in [DVec3::ZERO, DVec3::new(3.0, 0.5, -1.0), DVec3::new(-5.0, 4.0, 2.0), DVec3::new(0.5, 0.5, 0.5)] {
            assert!((s.eval(p) - sn.at(p)).abs() < 1e-12);
            assert!((b.eval(p) - bn.at(p)).abs() < 1e-12);
            assert!((u.eval(p) - un.at(p)).abs() < 1e-9, "smooth union {} vs {}", u.eval(p), un.at(p));
        }
        assert!(u.size() > 30);
        let g = gyroid(10.0, 1.0);
        assert!((g.eval(DVec3::ZERO) + 0.5).abs() < 1e-12);
    }

    #[cfg(feature = "fidget")]
    #[test]
    fn fidget_agrees_and_meshes() {
        use crate::mesh::volume;
        let e = subtract(&sphere(DVec3::ZERO, 10.0), &boxed(DVec3::new(-3.0, -3.0, -20.0), DVec3::new(3.0, 3.0, 20.0)));
        let c = jit::Compiled::new(&e).unwrap();
        let pts: Vec<DVec3> = (0..50).map(|i| DVec3::new(i as f64 * 0.3 - 7.0, 1.0, -2.0)).collect();
        let vals = c.eval_many(&pts).unwrap();
        for (p, v) in pts.iter().zip(&vals) {
            assert!((v - e.eval(*p)).abs() < 1e-3, "{v} vs {}", e.eval(*p));
        }
        let tris = c.mesh(DVec3::splat(-12.0), DVec3::splat(12.0), 6).unwrap();
        let exact = 4.0 / 3.0 * std::f64::consts::PI * 1000.0 - 36.0 * 2.0 * (100.0f64 - 18.0).sqrt();
        let v = volume(&tris);
        assert!((v - exact).abs() / exact < 0.05, "fidget volume {v} vs about {exact}");
    }
}
