//! The document: feature history, expressions, regeneration, undo.

use crate::{Feature, FeatureOutput};
use anvil_expr::ExprTable;
use anvil_kernel::{Kernel, KernelError, NativeKernel, Solid};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type FeatureId = usize;

fn bad_ref_text(me: &FeatureId, target: &FeatureId, what: &str) -> String {
    if *target == usize::MAX {
        format!("feature {me} uses a feature that was deleted; choose a new {what} in Properties")
    } else {
        format!("feature {me} uses feature {target}, which has no {what}")
    }
}

#[derive(Debug, Error)]
pub enum RegenError {
    #[error("{0}")]
    Kernel(#[from] KernelError),
    #[error("expression error: {0}")]
    Expr(#[from] anvil_expr::ExprError),
    #[error("{}", bad_ref_text(.0, .1, .2))]
    BadReference(FeatureId, FeatureId, &'static str),
    #[error("{0}")]
    Other(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeatureNode {
    pub feature: Box<dyn Feature>,
    pub suppressed: bool,
    #[serde(skip)]
    pub output: Option<FeatureOutput>,
    #[serde(skip)]
    pub error: Option<String>,
}

/// State handed to a feature while it regenerates.
pub struct RegenContext<'a> {
    pub kernel: &'a dyn Kernel,
    pub exprs: &'a ExprTable,
    /// Outputs of every earlier feature, by index. `None` if suppressed or failed.
    pub upstream: &'a [Option<FeatureOutput>],
    /// Index of the feature being regenerated.
    pub index: FeatureId,
    /// Faces that `PlaneRef::Face` references matched last time.
    pub face_memory: &'a std::collections::HashMap<crate::plane_ref::FaceKey, crate::plane_ref::FaceMemory>,
    /// Notes the feature gets besides its own, for example a picked face
    /// that was not found.
    pub notes: std::cell::RefCell<Vec<String>>,
    /// Face references that matched: the reference, the plane found, and
    /// the face. The Document writes the plane back into the reference.
    pub face_hits: std::cell::RefCell<Vec<(crate::PlaneRef, anvil_math::Plane, crate::plane_ref::FaceMemory)>>,
    /// Picked edges found again, with their two face normals. The
    /// Document writes them back into the feature.
    pub edge_hits: std::cell::RefCell<Vec<([anvil_math::DVec3; 2], [anvil_math::DVec3; 2])>>,
}

impl RegenContext<'_> {
    pub fn eval(&self, expr: &str) -> Result<f64, RegenError> {
        Ok(self.exprs.eval_str(expr)?)
    }
    /// Bodies produced by an earlier feature.
    pub fn bodies_of(&self, idx: FeatureId) -> Result<&[Solid], RegenError> {
        let out = self
            .upstream
            .get(idx)
            .and_then(|o| o.as_ref())
            .ok_or(RegenError::BadReference(self.index, idx, "output"))?;
        if out.bodies.is_empty() {
            return Err(RegenError::BadReference(self.index, idx, "body"));
        }
        Ok(&out.bodies)
    }

    /// Plane defined by an earlier feature (sketch or construction plane).
    pub fn plane_of(&self, idx: FeatureId) -> Result<anvil_math::Plane, RegenError> {
        self.upstream
            .get(idx)
            .and_then(|o| o.as_ref())
            .and_then(|o| o.plane)
            .ok_or(RegenError::BadReference(self.index, idx, "plane"))
    }

    /// Axis defined by an earlier construction feature.
    pub fn axis_of(&self, idx: FeatureId) -> Result<anvil_math::Axis, RegenError> {
        self.upstream
            .get(idx)
            .and_then(|o| o.as_ref())
            .and_then(|o| o.axis)
            .ok_or(RegenError::BadReference(self.index, idx, "axis"))
    }

    /// Point defined by an earlier construction feature.
    pub fn point_of(&self, idx: FeatureId) -> Result<anvil_math::DVec3, RegenError> {
        self.upstream
            .get(idx)
            .and_then(|o| o.as_ref())
            .and_then(|o| o.point)
            .ok_or(RegenError::BadReference(self.index, idx, "point"))
    }

    /// Open paths of an earlier sketch feature, in world coordinates.
    pub fn paths_of(&self, idx: FeatureId) -> Result<Vec<Vec<anvil_math::DVec3>>, RegenError> {
        let out = self
            .upstream
            .get(idx)
            .and_then(|o| o.as_ref())
            .ok_or(RegenError::BadReference(self.index, idx, "output"))?;
        let plane = out.plane.ok_or(RegenError::BadReference(self.index, idx, "sketch plane"))?;
        if out.paths.is_empty() {
            return Err(RegenError::BadReference(self.index, idx, "open path"));
        }
        Ok(out.paths.iter().map(|p| p.iter().map(|&q| plane.to_world(q)).collect()).collect())
    }

    pub fn profiles_of(&self, idx: FeatureId) -> Result<(&anvil_math::Plane, &[anvil_sketch::Profile]), RegenError> {
        let out = self
            .upstream
            .get(idx)
            .and_then(|o| o.as_ref())
            .ok_or(RegenError::BadReference(self.index, idx, "output"))?;
        let plane = out.plane.as_ref().ok_or(RegenError::BadReference(self.index, idx, "sketch plane"))?;
        if out.profiles.is_empty() {
            return Err(RegenError::BadReference(self.index, idx, "closed profile"));
        }
        Ok((plane, &out.profiles))
    }
}

/// Physical material: name and density in g/cm3.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Material {
    pub name: String,
    pub density: f64,
}

/// Materials offered by the ribbon. Density in g/cm3.
pub const MATERIALS: &[(&str, f64)] = &[
    ("Steel", 7.85),
    ("Stainless 316", 8.0),
    ("Aluminum 6061", 2.70),
    ("Titanium Ti-6Al-4V", 4.43),
    ("Brass", 8.5),
    ("Copper", 8.96),
    ("Magnesium AZ31", 1.77),
    ("ABS", 1.04),
    ("PLA", 1.24),
    ("Nylon", 1.15),
    ("Oak", 0.75),
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub name: String,
    pub features: Vec<FeatureNode>,
    pub exprs: ExprTable,
    /// Display colour per feature index, RGB.
    #[serde(default)]
    pub appearance: std::collections::HashMap<usize, [u8; 3]>,
    /// Physical material per feature index.
    #[serde(default)]
    pub material: std::collections::HashMap<usize, Material>,
    /// Display unit: "mm" or "in". Geometry is always stored in mm.
    #[serde(default = "default_unit")]
    pub unit: String,
    /// Rollback bar: features from this index on are held back and not
    /// computed. `None` runs the whole history.
    #[serde(default)]
    pub rollback: Option<usize>,
    /// Faces that plane references matched on the last Regenerate.
    #[serde(skip)]
    face_memory: std::collections::HashMap<crate::plane_ref::FaceKey, crate::plane_ref::FaceMemory>,
    #[serde(skip)]
    undo: Vec<Snapshot>,
    #[serde(skip)]
    redo: Vec<Snapshot>,
}

#[derive(Clone, Debug)]
struct Snapshot {
    features: Vec<FeatureNode>,
    exprs: ExprTable,
    rollback: Option<usize>,
}

fn default_unit() -> String {
    "mm".into()
}

impl Default for Document {
    fn default() -> Self {
        Document {
            name: "Untitled".into(),
            features: Vec::new(),
            exprs: ExprTable::new(),
            appearance: Default::default(),
            material: Default::default(),
            unit: default_unit(),
            rollback: None,
            face_memory: Default::default(),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
}

impl Document {
    pub fn new(name: &str) -> Self {
        Document { name: name.into(), ..Default::default() }
    }

    fn snapshot(&mut self) {
        self.undo.push(Snapshot {
            features: self.features.clone(),
            exprs: self.exprs.clone(),
            rollback: self.rollback,
        });
        self.redo.clear();
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some(s) = self.undo.pop() {
            self.redo.push(Snapshot {
                features: std::mem::take(&mut self.features),
                exprs: self.exprs.clone(),
                rollback: self.rollback,
            });
            self.features = s.features;
            self.exprs = s.exprs;
            self.rollback = s.rollback;
            self.regenerate();
        }
    }

    pub fn redo(&mut self) {
        if let Some(s) = self.redo.pop() {
            self.undo.push(Snapshot {
                features: std::mem::take(&mut self.features),
                exprs: self.exprs.clone(),
                rollback: self.rollback,
            });
            self.features = s.features;
            self.exprs = s.exprs;
            self.rollback = s.rollback;
            self.regenerate();
        }
    }

    /// Append a feature and regenerate. Returns its index.
    pub fn add_feature(&mut self, feature: Box<dyn Feature>) -> FeatureId {
        self.snapshot();
        let node = FeatureNode { feature, suppressed: false, output: None, error: None };
        // While the history is rolled back, a new feature goes in at the
        // bar, and the bar moves past it. Later features keep their order.
        let idx = match self.rollback {
            Some(r) if r < self.features.len() => {
                self.features.insert(r, node);
                let map = |old: usize| -> Option<usize> {
                    if old == usize::MAX || old < r {
                        Some(old)
                    } else {
                        Some(old + 1)
                    }
                };
                self.remap_all(&map);
                self.remap_maps(&map);
                self.rollback = Some(r + 1).filter(|n| *n < self.features.len());
                r
            }
            _ => {
                self.features.push(node);
                self.features.len() - 1
            }
        };
        self.regenerate_from(idx);
        idx
    }

    /// Remove a feature. References from later features are renumbered.
    /// References to the removed feature are marked broken, and the
    /// affected features report an error until the user fixes them.
    /// Returns the indices (after removal) of features with broken references.
    pub fn remove_feature(&mut self, idx: FeatureId) -> Vec<FeatureId> {
        if idx >= self.features.len() {
            return Vec::new();
        }
        self.snapshot();
        self.features.remove(idx);
        self.rollback = self.rollback.and_then(|r| match r {
            r if r > idx => Some(r - 1),
            r if r == idx => Some(r).filter(|n| *n < self.features.len()),
            r => Some(r),
        });
        let map = |old: usize| -> Option<usize> {
            if old == usize::MAX || old < idx {
                Some(old)
            } else if old == idx {
                None
            } else {
                Some(old - 1)
            }
        };
        let broken = self.remap_all(&map);
        self.remap_maps(&map);
        self.regenerate();
        broken
    }

    /// Move a feature from `from` to `to`, renumbering references. A move
    /// that would put a feature before something it depends on is refused.
    pub fn move_feature(&mut self, from: FeatureId, to: FeatureId) -> Result<(), String> {
        let n = self.features.len();
        if from >= n || to >= n || from == to {
            return Ok(());
        }
        let map = |old: usize| -> Option<usize> {
            if old == usize::MAX {
                return Some(old);
            }
            Some(if old == from {
                to
            } else if from < to && old > from && old <= to {
                old - 1
            } else if to < from && old >= to && old < from {
                old + 1
            } else {
                old
            })
        };
        // Dependency check on a scratch copy.
        let mut trial: Vec<FeatureNode> = self.features.clone();
        let node = trial.remove(from);
        trial.insert(to, node);
        for (i, f) in trial.iter_mut().enumerate() {
            let refs = std::cell::RefCell::new(Vec::new());
            let _ = f.feature.remap_refs(&|old| {
                let m = map(old);
                if let Some(v) = m {
                    refs.borrow_mut().push(v);
                }
                m
            });
            let refs = refs.into_inner();
            if refs.iter().any(|&r| r != usize::MAX && r >= i) {
                return Err(format!("{} would come before a feature it uses", f.feature.name()));
            }
        }
        self.snapshot();
        self.features = trial;
        self.remap_maps(&map);
        self.regenerate();
        Ok(())
    }

    fn remap_all(&mut self, map: &dyn Fn(usize) -> Option<usize>) -> Vec<FeatureId> {
        let mut broken = Vec::new();
        for (i, f) in self.features.iter_mut().enumerate() {
            if !f.feature.remap_refs(map).is_empty() {
                broken.push(i);
            }
        }
        broken
    }

    fn remap_maps(&mut self, map: &dyn Fn(usize) -> Option<usize>) {
        fn remap<V>(m: &mut std::collections::HashMap<usize, V>, map: &dyn Fn(usize) -> Option<usize>) {
            let old = std::mem::take(m);
            for (k, v) in old {
                if let Some(n) = map(k) {
                    m.insert(n, v);
                }
            }
        }
        remap(&mut self.appearance, map);
        remap(&mut self.material, map);
        // Face memory is keyed by Feature index; after a renumber the
        // stored planes are enough to find the faces again.
        self.face_memory.clear();
    }

    /// Edit a feature in place through a closure, with undo.
    pub fn edit_feature(&mut self, idx: FeatureId, f: impl FnOnce(&mut dyn Feature)) {
        if idx < self.features.len() {
            self.snapshot();
            f(self.features[idx].feature.as_mut());
            self.regenerate_from(idx);
        }
    }

    /// Edit a feature without adding an undo step. Used while a drag is in
    /// progress; the drag adds one undo step when it ends.
    pub fn edit_feature_quiet(&mut self, idx: FeatureId, f: impl FnOnce(&mut dyn Feature)) {
        if idx < self.features.len() {
            f(self.features[idx].feature.as_mut());
            self.regenerate_from(idx);
        }
    }

    pub fn set_suppressed(&mut self, idx: FeatureId, suppressed: bool) {
        if idx < self.features.len() {
            self.snapshot();
            self.features[idx].suppressed = suppressed;
            self.regenerate();
        }
    }

    pub fn set_expression(&mut self, name: &str, source: &str) -> Result<(), anvil_expr::ExprError> {
        self.snapshot();
        self.exprs.set(name, source)?;
        self.regenerate();
        Ok(())
    }

    /// Run the whole history in order. Errors are stored per feature; the
    /// rest of the history still runs.
    /// True when feature `idx` sits after the rollback bar, so it is not
    /// computed. Everything from the bar on is held back at once.
    pub fn rolled_back(&self, idx: FeatureId) -> bool {
        self.rollback.is_some_and(|r| idx >= r)
    }

    /// Move the rollback bar. `None` puts it after the last feature, so
    /// the whole history runs.
    pub fn set_rollback(&mut self, at: Option<FeatureId>) {
        let at = at.filter(|r| *r < self.features.len());
        if at == self.rollback {
            return;
        }
        self.snapshot();
        let first = match (self.rollback, at) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (None, None) => 0,
        };
        self.rollback = at;
        self.regenerate_from(first);
    }

    pub fn regenerate(&mut self) {
        self.regenerate_from(0);
    }

    /// Run the history from feature `start` on, keeping the outputs of
    /// earlier features. Adding or editing a feature cannot change what
    /// comes before it, so this skips that work.
    pub fn regenerate_from(&mut self, start: FeatureId) {
        let kernel = NativeKernel;
        if let Err(e) = self.exprs.evaluate() {
            log::warn!("expression table: {e}");
        }
        let start = start.min(self.features.len());
        let mut outputs: Vec<Option<FeatureOutput>> = Vec::with_capacity(self.features.len());
        for node in &self.features[..start] {
            outputs.push(node.output.clone());
        }
        for i in start..self.features.len() {
            if self.features[i].suppressed || self.rolled_back(i) {
                self.features[i].output = None;
                self.features[i].error = None;
                outputs.push(None);
                continue;
            }
            let (result, notes, hits) = {
                let mut ctx = RegenContext {
                    kernel: &kernel,
                    exprs: &self.exprs,
                    upstream: &outputs,
                    index: i,
                    face_memory: &self.face_memory,
                    notes: Default::default(),
                    face_hits: Default::default(),
                    edge_hits: Default::default(),
                };
                let r = self.features[i].feature.regenerate(&mut ctx);
                (r, ctx.notes.into_inner(), (ctx.face_hits.into_inner(), ctx.edge_hits.into_inner()))
            };
            let (hits, edges) = hits;
            self.remember_faces(i, hits);
            if result.is_ok() && !edges.is_empty() {
                self.features[i].feature.update_edges(&edges);
            }
            match result {
                Ok(mut out) => {
                    for n in notes {
                        out.note = Some(match out.note.take() {
                            Some(old) => format!("{old}; {n}"),
                            None => n,
                        });
                    }
                    self.features[i].output = Some(out.clone());
                    self.features[i].error = None;
                    outputs.push(Some(out));
                }
                Err(e) => {
                    log::warn!("feature {i} ({}): {e}", self.features[i].feature.name());
                    self.features[i].output = None;
                    self.features[i].error = Some(e.to_string());
                    outputs.push(None);
                }
            }
        }
    }

    /// Store the faces that feature `i`'s plane references matched, and
    /// write each new plane back into its reference.
    fn remember_faces(
        &mut self,
        i: FeatureId,
        hits: Vec<(crate::PlaneRef, anvil_math::Plane, crate::plane_ref::FaceMemory)>,
    ) {
        use crate::{ParamValue, PlaneRef};
        for (old, plane, memory) in hits {
            let PlaneRef::Face { feature, body, .. } = old else { continue };
            self.face_memory.insert(crate::plane_ref::face_key(i, feature, body, &plane), memory);
            let new = PlaneRef::Face { feature, body, plane };
            if new == old {
                continue;
            }
            let f = &mut self.features[i].feature;
            for p in f.params() {
                if p.value == ParamValue::Plane(old.clone()) {
                    let _ = f.set_param(p.name, ParamValue::Plane(new.clone()));
                }
            }
        }
    }

    /// Every visible body with the index of the feature that made it.
    /// Bodies consumed by a later feature (move, fillet, ...) are hidden.
    pub fn visible_bodies(&self) -> Vec<(FeatureId, &Solid)> {
        let consumed: std::collections::HashSet<usize> =
            self.features.iter().filter_map(|n| n.output.as_ref()).flat_map(|o| o.consumes.iter().copied()).collect();
        self.features
            .iter()
            .enumerate()
            .filter(|(i, _)| !consumed.contains(i))
            .filter_map(|(i, n)| n.output.as_ref().map(|o| (i, o)))
            .flat_map(|(i, o)| o.bodies.iter().map(move |b| (i, b)))
            .collect()
    }

    /// Every visible body produced by the current history.
    pub fn bodies(&self) -> Vec<&Solid> {
        self.visible_bodies().into_iter().map(|(_, b)| b).collect()
    }

    /// Mass in grams of a feature's bodies, if it has a material.
    pub fn mass_of(&self, idx: FeatureId) -> Option<f64> {
        let m = self.material.get(&idx)?;
        let vol_mm3: f64 = self.features.get(idx)?.output.as_ref()?.bodies.iter().map(|b| b.volume()).sum();
        Some(vol_mm3 / 1000.0 * m.density)
    }

    /// Format a length in the document unit.
    pub fn fmt_length(&self, mm: f64) -> String {
        if self.unit == "in" {
            format!("{:.4} in", mm / 25.4)
        } else {
            format!("{mm:.3} mm")
        }
    }

    pub fn fmt_volume(&self, mm3: f64) -> String {
        if self.unit == "in" {
            format!("{:.4} in3", mm3 / 16387.064)
        } else {
            format!("{mm3:.2} mm3")
        }
    }

    /// Format an area in the display unit.
    pub fn fmt_area(&self, mm2: f64) -> String {
        if self.unit == "in" {
            format!("{:.4} in2", mm2 / 645.16)
        } else {
            format!("{mm2:.2} mm2")
        }
    }

    /// Insert a feature at a position (for editing history order).
    pub fn insert_feature(&mut self, at: FeatureId, feature: Box<dyn Feature>) -> FeatureId {
        self.snapshot();
        let at = at.min(self.features.len());
        let map = |old: usize| -> Option<usize> { Some(if old != usize::MAX && old >= at { old + 1 } else { old }) };
        self.remap_all(&map);
        self.remap_maps(&map);
        self.features.insert(at, FeatureNode { feature, suppressed: false, output: None, error: None });
        self.regenerate();
        at
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(s: &str) -> serde_json::Result<Self> {
        let mut d: Document = serde_json::from_str(s)?;
        d.regenerate();
        Ok(d)
    }
}
