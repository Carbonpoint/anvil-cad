//! Post-processors: neutral toolpath to controller text.

use crate::{Move, Toolpath};
use std::fmt::Write;

pub trait Post {
    fn name(&self) -> &str;
    fn emit(&self, tp: &Toolpath) -> String;
}

/// Plain RS-274 G-code. Accepted by LinuxCNC, GRBL, and most Fanuc-style
/// controls for the subset of moves emitted here.
#[derive(Clone, Debug, Default)]
pub struct GenericGcode {
    pub program_name: String,
}

fn fmt(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

impl Post for GenericGcode {
    fn name(&self) -> &str {
        "Generic G-code"
    }
    fn emit(&self, tp: &Toolpath) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "%");
        let _ = writeln!(out, "({})", if self.program_name.is_empty() { "Anvil CAM" } else { &self.program_name });
        let _ = writeln!(out, "G21 G90 G17 G94");
        let mut last_feed = f64::NAN;
        for m in &tp.moves {
            match m {
                Move::Comment(c) => {
                    let _ = writeln!(out, "({})", c.replace(['(', ')'], " "));
                }
                Move::ToolChange(t) => {
                    let _ = writeln!(out, "T{} M6", t.number);
                }
                Move::SpindleOn { rpm, clockwise } => {
                    let _ = writeln!(out, "S{} {}", fmt(*rpm), if *clockwise { "M3" } else { "M4" });
                }
                Move::SpindleOff => {
                    let _ = writeln!(out, "M5");
                }
                Move::Rapid(p) => {
                    let _ = writeln!(out, "G0 X{} Y{} Z{}", fmt(p.x), fmt(p.y), fmt(p.z));
                }
                Move::Linear { to, feed } => {
                    if *feed != last_feed {
                        let _ = writeln!(out, "G1 X{} Y{} Z{} F{}", fmt(to.x), fmt(to.y), fmt(to.z), fmt(*feed));
                        last_feed = *feed;
                    } else {
                        let _ = writeln!(out, "G1 X{} Y{} Z{}", fmt(to.x), fmt(to.y), fmt(to.z));
                    }
                }
            }
        }
        let _ = writeln!(out, "M30");
        let _ = writeln!(out, "%");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anvil_math::DVec3;

    #[test]
    fn emits_header_and_moves() {
        let mut tp = Toolpath::default();
        tp.push(Move::Rapid(DVec3::new(1.0, 2.0, 5.0)));
        tp.push(Move::Linear { to: DVec3::new(1.0, 2.0, -1.0), feed: 100.0 });
        let g = GenericGcode::default().emit(&tp);
        assert!(g.contains("G21 G90"));
        assert!(g.contains("G0 X1 Y2 Z5"));
        assert!(g.contains("G1 X1 Y2 Z-1 F100"));
        assert!(g.trim_end().ends_with('%'));
    }
}
