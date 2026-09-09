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

    /// Every body produced by the current history.
    pub fn bodies(&self) -> Vec<&Solid> {
        self.features.iter().filter_map(|n| n.output.as_ref()).flat_map(|o| o.bodies.iter()).collect()
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
