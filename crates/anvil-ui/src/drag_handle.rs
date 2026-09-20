//! The arrow that drags the distance of a solid feature.
//!
//! Extrude and Press Pull get an arrow normal to their profile. Drag its
//! head to set the distance without typing a number.

use anvil_feature::{Document, ParamValue};
use anvil_math::{DVec2, DVec3};

/// Rounding of the dragged distance, in millimetres. Shift gives the fine step.
const STEP: f64 = 0.1;
const FINE_STEP: f64 = 0.01;

pub struct DragHandle {
    pub feature: usize,
    pub param: &'static str,
    /// Where the arrow starts, in world coordinates.
    pub origin: DVec3,
    /// Unit direction of the arrow, the profile normal.
    pub dir: DVec3,
    /// Current distance, in document units.
    pub distance: f64,
}

impl DragHandle {
    /// World point of the arrow head.
    pub fn tip(&self) -> DVec3 {
        self.origin + self.dir * self.distance
    }

    /// Round a dragged distance.
    pub fn snap(v: f64, fine: bool) -> f64 {
        let step = if fine { FINE_STEP } else { STEP };
        (v / step).round() * step
    }
}

/// The arrow for the selected feature, if it has one.
pub fn handle_for(doc: &Document, sel: Option<usize>) -> Option<DragHandle> {
    let idx = sel?;
    let node = doc.features.get(idx)?;
    if node.suppressed {
        return None;
    }
    let feature = &node.feature;
    let distance_expr = feature.params().into_iter().find_map(|p| match (p.name, p.value) {
        ("distance", ParamValue::Expr(s)) => Some(s),
        _ => None,
    })?;
    // A distance built from named expressions is not dragged: a drag would
    // throw the formula away.
    let distance = distance_expr.trim().parse::<f64>().ok()?;
    let (plane, centre) = match feature.kind() {
        "extrude" => {
            let sketch = feature.params().into_iter().find_map(|p| match (p.name, p.value) {
                ("sketch", ParamValue::FeatureRef(i)) => Some(i),
                _ => None,
            })?;
            let spec = feature.params().into_iter().find_map(|p| match (p.name, p.value) {
                ("regions", ParamValue::Expr(s)) => Some(s),
                _ => None,
            })?;
            let out = doc.features.get(sketch)?.output.as_ref()?;
            let plane = out.plane?;
            let picked = anvil_feature::region_select::select(&out.profiles, &spec).ok()?;
            (plane, centroid(picked.iter().flat_map(|(o, _)| o.iter().copied()))?)
        }
        _ => feature.drag_plane()?,
    };
    Some(DragHandle { feature: idx, param: "distance", origin: plane.to_world(centre), dir: plane.normal(), distance })
}

fn centroid(pts: impl Iterator<Item = DVec2>) -> Option<DVec2> {
    let mut sum = DVec2::ZERO;
    let mut n = 0.0;
    for p in pts {
        sum += p;
        n += 1.0;
    }
    (n > 0.0).then(|| sum / n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_feature::features::{extrude::ExtrudeFeature, sketch::SketchFeature};

    #[test]
    fn extrude_and_press_pull_get_an_arrow() {
        let mut doc = Document::new("t");
        doc.add_feature(Box::new(SketchFeature::rectangle("XY", 10.0, 20.0)));
        doc.add_feature(Box::new(ExtrudeFeature { distance: "7".into(), ..Default::default() }));
        let h = handle_for(&doc, Some(1)).expect("extrude has an arrow");
        assert_eq!(h.distance, 7.0);
        assert!((h.dir - DVec3::Z).length() < 1e-9);
        assert!((h.tip() - h.origin).length() - 7.0 < 1e-9);
        // The sketch itself has no arrow.
        assert!(handle_for(&doc, Some(0)).is_none());

        // A distance built from a named expression is not dragged.
        doc.set_expression("h", "5").unwrap();
        doc.edit_feature(1, |f| f.set_param("distance", ParamValue::Expr("h".into())).unwrap());
        assert!(handle_for(&doc, Some(1)).is_none());
    }

    #[test]
    fn snap_rounds_to_the_step() {
        assert_eq!(DragHandle::snap(7.043, false), 7.0);
        assert!((DragHandle::snap(7.043, true) - 7.04).abs() < 1e-9);
    }
}
