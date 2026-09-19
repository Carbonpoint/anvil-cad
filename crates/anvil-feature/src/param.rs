//! Generic parameter description so the UI can edit any feature without
//! knowing its concrete type.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ParamValue {
    /// An expression string, evaluated against the document expression table.
    Expr(String),
    Bool(bool),
    /// Index of another feature in the history (for example the sketch an
    /// extrude consumes).
    FeatureRef(usize),
    /// A choice from a fixed list, stored by label.
    Choice(String),
    Text(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParamKind {
    Length,
    Angle,
    Bool,
    /// Reference to a feature whose `type_id` is in the list.
    FeatureRef {
        accepts: Vec<&'static str>,
    },
    Choice {
        options: Vec<&'static str>,
    },
    Text,
    /// A font: stored as `builtin:<name>`, a file path, or empty for the default.
    Font,
    /// Which regions of a sketch to use, stored as text (see
    /// `region_select`). `sketch` names the FeatureRef param that holds the
    /// sketch, so the viewport can show its regions for picking.
    Regions {
        sketch: &'static str,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParamSpec {
    pub name: &'static str,
    pub label: &'static str,
    pub kind: ParamKind,
    pub value: ParamValue,
}

impl ParamSpec {
    pub fn length(name: &'static str, label: &'static str, expr: &str) -> Self {
        ParamSpec { name, label, kind: ParamKind::Length, value: ParamValue::Expr(expr.to_string()) }
    }
    pub fn angle(name: &'static str, label: &'static str, expr: &str) -> Self {
        ParamSpec { name, label, kind: ParamKind::Angle, value: ParamValue::Expr(expr.to_string()) }
    }
    pub fn boolean(name: &'static str, label: &'static str, v: bool) -> Self {
        ParamSpec { name, label, kind: ParamKind::Bool, value: ParamValue::Bool(v) }
    }
    pub fn feature_ref(name: &'static str, label: &'static str, accepts: Vec<&'static str>, idx: usize) -> Self {
        ParamSpec { name, label, kind: ParamKind::FeatureRef { accepts }, value: ParamValue::FeatureRef(idx) }
    }
    pub fn regions(name: &'static str, sketch: &'static str, spec: &str) -> Self {
        ParamSpec {
            name,
            label: "Regions",
            kind: ParamKind::Regions { sketch },
            value: ParamValue::Expr(spec.to_string()),
        }
    }
    pub fn choice(name: &'static str, label: &'static str, options: Vec<&'static str>, v: &str) -> Self {
        ParamSpec { name, label, kind: ParamKind::Choice { options }, value: ParamValue::Choice(v.to_string()) }
    }
}
