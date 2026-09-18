//! Fast aerodynamic estimates for a lofted wing, so a design loop can run
//! inside the CAD in milliseconds: Prandtl lifting line for lift and
//! induced drag, a flat plate estimate for profile drag, and a Nelder
//! Mead search that adjusts taper and twist for the best lift to drag
//! ratio at a design lift coefficient. Numbers are estimates for early
//! design; validate the final shape with XFOIL or a CFD run.

use crate::wing::Wing;

/// Air at sea level, ISA.
pub const RHO: f64 = 1.225;
pub const NU: f64 = 1.46e-5;

/// Result of a lifting line solution.
#[derive(Clone, Debug, PartialEq)]
pub struct AeroResult {
    pub alpha_deg: f64,
    pub cl: f64,
    pub cd_induced: f64,
    pub cd_profile: f64,
    pub span_efficiency: f64,
    pub lift_to_drag: f64,
    /// Section lift coefficient at each station from root to tip.
    pub cl_sections: Vec<f64>,
}

impl AeroResult {
    pub fn cd(&self) -> f64 {
        self.cd_induced + self.cd_profile
    }
}

/// Prandtl lifting line with `n` Fourier terms, lift slope 2 pi per
/// radian, zero lift angle from the section camber. `alpha_deg` is the
/// root incidence; twist adds to it along the span.
pub fn lifting_line(wing: &Wing, alpha_deg: f64, speed: f64, n: usize) -> AeroResult {
    use std::f64::consts::PI;
    let n = n.max(4);
    let b = wing.span;
    let a0 = 2.0 * PI;
    let stations: Vec<f64> = (1..=n).map(|k| PI * k as f64 / (n as f64 + 1.0)).collect();
    // Solve sum_j A_j [ sin(j th) (4 b / (a0 c) + j / sin th) ] = alpha_eff(th).
    let mut mat = vec![vec![0.0; n]; n];
    let mut rhs = vec![0.0; n];
    for (i, &th) in stations.iter().enumerate() {
        let eta = th.cos().abs();
        let c = wing.chord(eta);
        let a_l0 = section_zero_lift(wing, eta);
        let alpha = (alpha_deg + wing.twist(eta)).to_radians() - a_l0;
        rhs[i] = alpha;
        for j in 1..=n {
            let jf = j as f64;
            mat[i][j - 1] = (jf * th).sin() * (4.0 * b / (a0 * c) + jf / th.sin());
        }
    }
    let a = solve(mat, rhs);
    let ar = wing.aspect_ratio();
    let cl = PI * ar * a[0];
    let delta: f64 = (2..=n).map(|j| j as f64 * (a[j - 1] / a[0].abs().max(1e-12)).powi(2)).sum();
    let e = 1.0 / (1.0 + delta);
    let cd_induced = cl * cl / (PI * ar * e);
    // Section lift coefficients: cl(th) = 4 b / c * sum A_j sin(j th).
    let cl_sections: Vec<f64> = (0..=10)
        .map(|k| {
            let eta = k as f64 / 10.0;
            let th = eta.min(0.999).acos();
            let c = wing.chord(eta);
            4.0 * b / c * (1..=n).map(|j| a[j - 1] * (j as f64 * th).sin()).sum::<f64>()
        })
        .collect();
    let cd_profile = profile_drag(wing, speed);
    let cd = cd_induced + cd_profile;
    AeroResult {
        alpha_deg,
        cl,
        cd_induced,
        cd_profile,
        span_efficiency: e,
        lift_to_drag: if cd > 1e-12 { cl / cd } else { 0.0 },
        cl_sections,
    }
}

fn section_zero_lift(wing: &Wing, eta: f64) -> f64 {
    let a = crate::wing::Naca4 {
        m: wing.root.m + (wing.tip.m - wing.root.m) * eta,
        p: wing.root.p + (wing.tip.p - wing.root.p) * eta,
        t: wing.root.t + (wing.tip.t - wing.root.t) * eta,
    };
    a.zero_lift_angle()
}

/// Profile drag from turbulent flat plate skin friction on the wetted
/// area with a thickness form factor, at the mean chord Reynolds number.
pub fn profile_drag(wing: &Wing, speed: f64) -> f64 {
    let c_mean = wing.area() / wing.span.max(1e-12);
    let re = (speed * c_mean / NU).max(1e4);
    let cf = 0.074 / re.powf(0.2);
    let tc = 0.5 * (wing.root.t + wing.tip.t);
    let form = 1.0 + 2.0 * tc + 100.0 * tc.powi(4);
    // Wetted area about twice the planform area.
    2.0 * cf * form
}

/// Root incidence that gives lift coefficient `cl_target`, by secant
/// iteration on the lifting line.
pub fn alpha_for_cl(wing: &Wing, cl_target: f64, speed: f64) -> f64 {
    let f = |a: f64| lifting_line(wing, a, speed, 12).cl - cl_target;
    let (mut a0, mut a1) = (0.0, 4.0);
    let (mut f0, mut f1) = (f(a0), f(a1));
    for _ in 0..20 {
        if (f1 - f0).abs() < 1e-12 {
            break;
        }
        let a2 = a1 - f1 * (a1 - a0) / (f1 - f0);
        a0 = a1;
        f0 = f1;
        a1 = a2;
        f1 = f(a1);
        if f1.abs() < 1e-8 {
            break;
        }
    }
    a1
}

/// Lift to drag ratio at the design lift coefficient.
pub fn lift_to_drag_at(wing: &Wing, cl_target: f64, speed: f64) -> (f64, AeroResult) {
    let alpha = alpha_for_cl(wing, cl_target, speed);
    let r = lifting_line(wing, alpha, speed, 12);
    (r.lift_to_drag, r)
}

/// Gaussian elimination with partial pivoting.
#[allow(clippy::needless_range_loop)]
fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Vec<f64> {
    let n = b.len();
    for k in 0..n {
        let piv = (k..n).max_by(|&i, &j| a[i][k].abs().total_cmp(&a[j][k].abs())).unwrap();
        a.swap(k, piv);
        b.swap(k, piv);
        let d = a[k][k];
        if d.abs() < 1e-300 {
            continue;
        }
        for i in k + 1..n {
            let f = a[i][k] / d;
            if f == 0.0 {
                continue;
            }
            for j in k..n {
                a[i][j] -= f * a[k][j];
            }
            b[i] -= f * b[k];
        }
    }
    let mut x = vec![0.0; n];
    for k in (0..n).rev() {
        let s: f64 = (k + 1..n).map(|j| a[k][j] * x[j]).sum();
        x[k] = if a[k][k].abs() < 1e-300 { 0.0 } else { (b[k] - s) / a[k][k] };
    }
    x
}

/// Nelder Mead minimisation of `f` from `start` with initial steps
/// `step`, bounded to `lo..hi` per coordinate.
#[allow(clippy::needless_range_loop)]
pub fn nelder_mead(
    f: &dyn Fn(&[f64]) -> f64,
    start: &[f64],
    step: &[f64],
    lo: &[f64],
    hi: &[f64],
    iters: usize,
) -> (Vec<f64>, f64) {
    let n = start.len();
    let clampv = |v: &mut Vec<f64>| {
        for i in 0..n {
            v[i] = v[i].clamp(lo[i], hi[i]);
        }
    };
    let mut simplex: Vec<(Vec<f64>, f64)> = Vec::with_capacity(n + 1);
    let mut s0 = start.to_vec();
    clampv(&mut s0);
    simplex.push((s0.clone(), f(&s0)));
    for i in 0..n {
        let mut v = s0.clone();
        v[i] += step[i];
        clampv(&mut v);
        let fv = f(&v);
        simplex.push((v, fv));
    }
    for _ in 0..iters {
        simplex.sort_by(|a, b| a.1.total_cmp(&b.1));
        let best = simplex[0].1;
        let worst = simplex[n].1;
        if (worst - best).abs() < 1e-9 {
            break;
        }
        let mut centroid = vec![0.0; n];
        for s in &simplex[..n] {
            for i in 0..n {
                centroid[i] += s.0[i] / n as f64;
            }
        }
        let reflect = |k: f64| -> Vec<f64> {
            let mut v: Vec<f64> = (0..n).map(|i| centroid[i] + k * (centroid[i] - simplex[n].0[i])).collect();
            clampv(&mut v);
            v
        };
        let r = reflect(1.0);
        let fr = f(&r);
        if fr < simplex[0].1 {
            let e = reflect(2.0);
            let fe = f(&e);
            simplex[n] = if fe < fr { (e, fe) } else { (r, fr) };
        } else if fr < simplex[n - 1].1 {
            simplex[n] = (r, fr);
        } else {
            let c = reflect(-0.5);
            let fc = f(&c);
            if fc < simplex[n].1 {
                simplex[n] = (c, fc);
            } else {
                let b0 = simplex[0].0.clone();
                for s in simplex.iter_mut().skip(1) {
                    for i in 0..n {
                        s.0[i] = b0[i] + 0.5 * (s.0[i] - b0[i]);
                    }
                    s.1 = f(&s.0);
                }
            }
        }
    }
    simplex.sort_by(|a, b| a.1.total_cmp(&b.1));
    simplex.remove(0)
}

/// Adjust taper ratio and tip twist for the best lift to drag ratio at
/// `cl_target`, keeping span and planform area. Returns the improved wing
/// and its result.
pub fn optimize_taper_and_twist(wing: &Wing, cl_target: f64, speed: f64) -> (Wing, AeroResult) {
    let area = wing.area();
    let build = |v: &[f64]| -> Wing {
        let taper = v[0];
        let root = 2.0 * area / (wing.span * (1.0 + taper));
        Wing::new(wing.span, root, root * taper, wing.sweep, wing.dihedral, wing.twist_root, v[1], wing.root, wing.tip)
    };
    let objective = |v: &[f64]| -> f64 { -lift_to_drag_at(&build(v), cl_target, speed).0 };
    let start = [wing.taper(), wing.twist_tip];
    let (best, _) = nelder_mead(&objective, &start, &[0.1, 1.0], &[0.2, -8.0], &[1.0, 4.0], 80);
    let w = build(&best);
    let (_, r) = lift_to_drag_at(&w, cl_target, speed);
    (w, r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wing::Naca4;

    fn wing(taper: f64, twist_tip: f64) -> Wing {
        let root = 2.0 / (1.0 + taper);
        Wing::new(
            10.0,
            root,
            root * taper,
            0.0,
            0.0,
            0.0,
            twist_tip,
            Naca4::parse("0012").unwrap(),
            Naca4::parse("0012").unwrap(),
        )
    }

    #[test]
    fn elliptic_like_loading_has_high_span_efficiency() {
        // An untwisted rectangular wing of aspect ratio 10 has e about 0.9;
        // lift slope near 2 pi / (1 + 2 / AR).
        let w = wing(1.0, 0.0);
        let r = lifting_line(&w, 5.0, 30.0, 16);
        assert!(r.span_efficiency > 0.85 && r.span_efficiency <= 1.0, "{}", r.span_efficiency);
        let slope = r.cl / 5.0f64.to_radians();
        let expect = 2.0 * std::f64::consts::PI / (1.0 + 2.0 / 10.0);
        assert!((slope - expect).abs() / expect < 0.08, "{slope} vs {expect}");
        assert!(r.cd_induced > 0.0 && r.cd_profile > 0.0);
    }

    #[test]
    fn taper_near_0_4_beats_a_rectangular_wing() {
        let (_, rect) = lift_to_drag_at(&wing(1.0, 0.0), 0.5, 30.0);
        let (_, tapered) = lift_to_drag_at(&wing(0.4, 0.0), 0.5, 30.0);
        assert!(tapered.span_efficiency > rect.span_efficiency);
    }

    #[test]
    fn optimizer_improves_lift_to_drag() {
        let w = wing(1.0, 3.0);
        let (_, before) = lift_to_drag_at(&w, 0.5, 30.0);
        let (better, after) = optimize_taper_and_twist(&w, 0.5, 30.0);
        assert!(after.lift_to_drag >= before.lift_to_drag - 1e-9, "{} vs {}", after.lift_to_drag, before.lift_to_drag);
        assert!((better.area() - w.area()).abs() < 1e-9, "area kept");
        assert!((0.2..=1.0).contains(&better.taper()));
        for (taper, twist) in [(1.0, 0.0), (0.4, 0.0), (1.0, -3.0)] {
            let r = lifting_line(&wing(taper, twist), 5.0, 30.0, 16);
            eprintln!("taper {taper} twist {twist}: e {:.3}, CL {:.3}", r.span_efficiency, r.cl);
        }
        eprintln!(
            "optimum: taper {:.2}, tip twist {:.2}, L/D {:.2}, e {:.3}",
            better.taper(),
            better.twist_tip,
            after.lift_to_drag,
            after.span_efficiency
        );
    }
}
