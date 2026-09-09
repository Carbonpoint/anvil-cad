use anvil_math::DVec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tool {
    pub number: u32,
    pub name: String,
    /// Cutting diameter in mm.
    pub diameter: f64,
    /// Spindle speed in rev/min.
    pub rpm: f64,
    /// Cutting feed in mm/min.
    pub feed: f64,
    /// Plunge feed in mm/min.
    pub plunge: f64,
}

impl Tool {
    pub fn radius(&self) -> f64 {
        self.diameter / 2.0
    }
}

/// One neutral toolpath move, in machine coordinates (mm).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Move {
    Rapid(DVec3),
    /// Straight cut at the given feed (mm/min).
    Linear {
        to: DVec3,
        feed: f64,
    },
    ToolChange(Tool),
    SpindleOn {
        rpm: f64,
        clockwise: bool,
    },
    SpindleOff,
    Comment(String),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Toolpath {
    pub moves: Vec<Move>,
}

impl Toolpath {
    pub fn push(&mut self, m: Move) {
        self.moves.push(m);
    }

    /// Total cutting length (linear moves only).
    pub fn cut_length(&self) -> f64 {
        let mut last: Option<DVec3> = None;
        let mut len = 0.0;
        for m in &self.moves {
            match m {
                Move::Rapid(p) => last = Some(*p),
                Move::Linear { to, .. } => {
                    if let Some(l) = last {
                        len += (*to - l).length();
                    }
                    last = Some(*to);
                }
                _ => {}
            }
        }
        len
    }
}
