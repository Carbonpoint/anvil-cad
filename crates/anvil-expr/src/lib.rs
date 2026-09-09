//! Expression engine.
//!
//! An `ExprTable` holds named parameters like `width = 40` or
//! `height = width / 2 + 5`. Each parameter is an *expression*. The table
//! evaluates them in dependency order and reports cycles and missing names.
//! This mirrors the NX "Expressions" dialog at a small scale.

use std::collections::{BTreeMap, HashMap, HashSet};
use thiserror::Error;

mod parser;
pub use parser::{parse, Ast};

#[derive(Debug, Error, PartialEq)]
pub enum ExprError {
    #[error("parse error at {pos}: {msg}")]
    Parse { pos: usize, msg: String },
    #[error("unknown name '{0}'")]
    UnknownName(String),
    #[error("cycle through '{0}'")]
    Cycle(String),
    #[error("division by zero")]
    DivZero,
    #[error("unknown function '{0}'")]
    UnknownFunction(String),
}

/// A named parameter with its source text.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Param {
    pub name: String,
    pub source: String,
    #[serde(skip)]
    pub value: f64,
}

/// A table of named parameters.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ExprTable {
    params: BTreeMap<String, Param>,
}

impl ExprTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set or replace a parameter. Values are recomputed by `evaluate`.
    pub fn set(&mut self, name: &str, source: &str) -> Result<(), ExprError> {
        parse(source)?;
        self.params.insert(name.to_string(), Param { name: name.to_string(), source: source.to_string(), value: 0.0 });
        Ok(())
    }

    pub fn remove(&mut self, name: &str) {
        self.params.remove(name);
    }

    pub fn get(&self, name: &str) -> Option<&Param> {
        self.params.get(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Param> {
        self.params.values()
    }

    /// Evaluate one expression string against the current table values.
    pub fn eval_str(&self, source: &str) -> Result<f64, ExprError> {
        let ast = parse(source)?;
        let env: HashMap<&str, f64> = self.params.iter().map(|(k, v)| (k.as_str(), v.value)).collect();
        eval_ast(&ast, &env)
    }

    /// Evaluate every parameter in dependency order.
    pub fn evaluate(&mut self) -> Result<(), ExprError> {
        let asts: BTreeMap<String, Ast> =
            self.params.iter().map(|(k, p)| parse(&p.source).map(|a| (k.clone(), a))).collect::<Result<_, _>>()?;
        let mut order = Vec::new();
        let mut done = HashSet::new();
        let mut stack = HashSet::new();
        for name in asts.keys() {
            visit(name, &asts, &mut done, &mut stack, &mut order)?;
        }
        let mut env: HashMap<&str, f64> = HashMap::new();
        let mut values = Vec::new();
        for name in &order {
            let v = eval_ast(&asts[name], &env)?;
            values.push((name.clone(), v));
            env.insert(name.as_str(), v);
        }
        for (name, v) in values {
            self.params.get_mut(&name).unwrap().value = v;
        }
        Ok(())
    }
}

fn visit(
    name: &str,
    asts: &BTreeMap<String, Ast>,
    done: &mut HashSet<String>,
    stack: &mut HashSet<String>,
    order: &mut Vec<String>,
) -> Result<(), ExprError> {
    if done.contains(name) {
        return Ok(());
    }
    if !stack.insert(name.to_string()) {
        return Err(ExprError::Cycle(name.to_string()));
    }
    let ast = asts.get(name).ok_or_else(|| ExprError::UnknownName(name.to_string()))?;
    let mut deps = Vec::new();
    ast.names(&mut deps);
    for d in deps {
        visit(&d, asts, done, stack, order)?;
    }
    stack.remove(name);
    done.insert(name.to_string());
    order.push(name.to_string());
    Ok(())
}

pub fn eval_ast(ast: &Ast, env: &HashMap<&str, f64>) -> Result<f64, ExprError> {
    Ok(match ast {
        Ast::Num(v) => *v,
        Ast::Name(n) => *env.get(n.as_str()).ok_or_else(|| ExprError::UnknownName(n.clone()))?,
        Ast::Neg(a) => -eval_ast(a, env)?,
        Ast::Add(a, b) => eval_ast(a, env)? + eval_ast(b, env)?,
        Ast::Sub(a, b) => eval_ast(a, env)? - eval_ast(b, env)?,
        Ast::Mul(a, b) => eval_ast(a, env)? * eval_ast(b, env)?,
        Ast::Div(a, b) => {
            let d = eval_ast(b, env)?;
            if d == 0.0 {
                return Err(ExprError::DivZero);
            }
            eval_ast(a, env)? / d
        }
        Ast::Pow(a, b) => eval_ast(a, env)?.powf(eval_ast(b, env)?),
        Ast::Call(f, args) => {
            let v: Vec<f64> = args.iter().map(|a| eval_ast(a, env)).collect::<Result<_, _>>()?;
            match (f.as_str(), v.as_slice()) {
                ("sin", [x]) => x.to_radians().sin(),
                ("cos", [x]) => x.to_radians().cos(),
                ("tan", [x]) => x.to_radians().tan(),
                ("sqrt", [x]) => x.sqrt(),
                ("abs", [x]) => x.abs(),
                ("min", [a, b]) => a.min(*b),
                ("max", [a, b]) => a.max(*b),
                ("floor", [x]) => x.floor(),
                ("ceil", [x]) => x.ceil(),
                ("round", [x]) => x.round(),
                _ => return Err(ExprError::UnknownFunction(f.clone())),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_in_dependency_order() {
        let mut t = ExprTable::new();
        t.set("height", "width / 2 + 5").unwrap();
        t.set("width", "40").unwrap();
        t.evaluate().unwrap();
        assert_eq!(t.get("width").unwrap().value, 40.0);
        assert_eq!(t.get("height").unwrap().value, 25.0);
    }

    #[test]
    fn detects_cycles() {
        let mut t = ExprTable::new();
        t.set("a", "b + 1").unwrap();
        t.set("b", "a + 1").unwrap();
        assert!(matches!(t.evaluate(), Err(ExprError::Cycle(_))));
    }

    #[test]
    fn functions_and_precedence() {
        let t = ExprTable::new();
        assert_eq!(t.eval_str("2 + 3 * 4 ^ 2").unwrap(), 50.0);
        assert!((t.eval_str("sin(30)").unwrap() - 0.5).abs() < 1e-12);
        assert_eq!(t.eval_str("-(1 + 2) * max(3, 4)").unwrap(), -12.0);
    }
}
