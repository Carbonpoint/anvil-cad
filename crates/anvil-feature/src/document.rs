//! The document: feature history, expressions, regeneration, undo.

use crate::{Feature, FeatureOutput};
use anvil_expr::ExprTable;
use anvil_kernel::{Kernel, KernelError, NativeKernel, Solid};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type FeatureId = usize;

#[derive(Debug, Error)]
pub enum RegenError {
    #[error("{0}")]
    Kernel(#[from] KernelError),
    #[error("expression error: {0}")]
    Expr(#[from] anvil_expr::ExprError),
    #[error("feature {0} references feature {1}, which has no {2}")]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub name: String,
    pub features: Vec<FeatureNode>,
    pub exprs: ExprTable,
    #[serde(skip)]
    undo: Vec<Snapshot>,
    #[serde(skip)]
    redo: Vec<Snapshot>,
}

#[derive(Clone, Debug)]
struct Snapshot {
    features: Vec<FeatureNode>,
    exprs: ExprTable,
}

impl Default for Document {
    fn default() -> Self {
        Document {
            name: "Untitled".into(),
            features: Vec::new(),
            exprs: ExprTable::new(),
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
        self.undo.push(Snapshot { features: self.features.clone(), exprs: self.exprs.clone() });
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
            self.redo.push(Snapshot { features: std::mem::take(&mut self.features), exprs: self.exprs.clone() });
            self.features = s.features;
            self.exprs = s.exprs;
            self.regenerate();
        }
    }

    pub fn redo(&mut self) {
        if let Some(s) = self.redo.pop() {
            self.undo.push(Snapshot { features: std::mem::take(&mut self.features), exprs: self.exprs.clone() });
            self.features = s.features;
            self.exprs = s.exprs;
            self.regenerate();
        }
    }

    /// Append a feature and regenerate. Returns its index.
    pub fn add_feature(&mut self, feature: Box<dyn Feature>) -> FeatureId {
        self.snapshot();
        self.features.push(FeatureNode { feature, suppressed: false, output: None, error: None });
        self.regenerate();
        self.features.len() - 1
    }

    pub fn remove_feature(&mut self, idx: FeatureId) {
        if idx < self.features.len() {
            self.snapshot();
            self.features.remove(idx);
            self.regenerate();
        }
    }

    /// Edit a feature in place through a closure, with undo.
    pub fn edit_feature(&mut self, idx: FeatureId, f: impl FnOnce(&mut dyn Feature)) {
        if idx < self.features.len() {
            self.snapshot();
            f(self.features[idx].feature.as_mut());
            self.regenerate();
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
    pub fn regenerate(&mut self) {
        let kernel = NativeKernel;
        if let Err(e) = self.exprs.evaluate() {
            log::warn!("expression table: {e}");
        }
        let mut outputs: Vec<Option<FeatureOutput>> = Vec::with_capacity(self.features.len());
        for i in 0..self.features.len() {
            if self.features[i].suppressed {
                self.features[i].output = None;
                self.features[i].error = None;
                outputs.push(None);
                continue;
            }
            let result = {
                let mut ctx = RegenContext { kernel: &kernel, exprs: &self.exprs, upstream: &outputs, index: i };
                self.features[i].feature.regenerate(&mut ctx)
            };
            match result {
                Ok(out) => {
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

    /// Insert a feature at a position (for editing history order).
    pub fn insert_feature(&mut self, at: FeatureId, feature: Box<dyn Feature>) -> FeatureId {
        self.snapshot();
        let at = at.min(self.features.len());
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
