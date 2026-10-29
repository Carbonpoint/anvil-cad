//! Post-processors: neutral toolpath to controller text.
//!
//! One writer, many dialects. A dialect sets the program header and
//! footer, how a tool change reads, and whether the control has canned
//! drilling cycles (G81 and G83); without them, drilling is written out as
//! plain moves.

use crate::{Move, Toolpath};
use std::fmt::Write;

pub trait Post {
    fn name(&self) -> &str;
    fn emit(&self, tp: &Toolpath) -> String;
}

/// One controller family.
#[derive(Clone, Debug)]
pub struct Dialect {
    pub name: &'static str,
    /// Wrap the program in `%` lines.
    pub percent: bool,
    /// Start with a program number line, `O0001 (name)`.
    pub program_number: bool,
    /// Tool length offset (G43 H) after a tool change.
    pub length_offset: bool,
    /// The control changes tools with M6; without it the program stops
    /// (M0) for a manual change.
    pub m6: bool,
    pub canned_cycles: bool,
    pub footer: &'static [&'static str],
    pub program_name: String,
}

impl Dialect {
    pub fn linuxcnc() -> Self {
        Dialect {
            name: "LinuxCNC",
            percent: true,
            program_number: false,
            length_offset: true,
            m6: true,
            canned_cycles: true,
            footer: &["M2"],
            program_name: String::new(),
        }
    }
    pub fn grbl() -> Self {
        Dialect {
            name: "GRBL",
            percent: false,
            program_number: false,
            length_offset: false,
            m6: false,
            canned_cycles: false,
            footer: &["M2"],
            program_name: String::new(),
        }
    }
    pub fn fanuc() -> Self {
        Dialect {
            name: "Fanuc",
            percent: true,
            program_number: true,
            length_offset: true,
            m6: true,
            canned_cycles: true,
            footer: &["G28 G91 Z0", "G90", "M30"],
            program_name: String::new(),
        }
    }
    pub fn haas() -> Self {
        Dialect {
            name: "Haas",
            percent: true,
            program_number: true,
            length_offset: true,
            m6: true,
            canned_cycles: true,
            footer: &["G53 G00 Z0", "M30"],
            program_name: String::new(),
        }
    }
}

/// Every post Anvil offers, the plain one first.
pub fn posts() -> Vec<Box<dyn Post>> {
    vec![
        Box::new(GenericGcode::default()),
        Box::new(Dialect::linuxcnc()),
        Box::new(Dialect::grbl()),
        Box::new(Dialect::fanuc()),
        Box::new(Dialect::haas()),
    ]
}

/// Plain RS-274 G-code. Accepted by LinuxCNC, GRBL, and most Fanuc-style
/// controls for the subset of moves emitted here; drilling is written out
/// as moves, so it runs everywhere.
#[derive(Clone, Debug, Default)]
pub struct GenericGcode {
    pub program_name: String,
}

impl Post for GenericGcode {
    fn name(&self) -> &str {
        "Generic G-code"
    }
    fn emit(&self, tp: &Toolpath) -> String {
        let d = Dialect {
            name: "Generic G-code",
            percent: true,
            program_number: false,
            length_offset: false,
            m6: true,
            canned_cycles: false,
            footer: &["M30"],
            program_name: self.program_name.clone(),
        };
        d.emit(tp)
    }
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

impl Post for Dialect {
    fn name(&self) -> &str {
        self.name
    }
    fn emit(&self, tp: &Toolpath) -> String {
        let mut out = String::new();
        let title = if self.program_name.is_empty() { "Anvil CAM" } else { &self.program_name };
        if self.percent {
            let _ = writeln!(out, "%");
        }
        if self.program_number {
            let _ = writeln!(out, "O0001 ({})", title.replace(['(', ')'], " "));
        } else {
            let _ = writeln!(out, "({})", title.replace(['(', ')'], " "));
        }
        let _ = writeln!(out, "G21 G90 G17 G94");
        let mut last_feed = f64::NAN;
        let mut in_cycle = false;
        for m in &tp.moves {
            if in_cycle && !matches!(m, Move::Drill { .. }) {
                let _ = writeln!(out, "G80");
                in_cycle = false;
            }
            match m {
                Move::Comment(c) => {
                    let _ = writeln!(out, "({})", c.replace(['(', ')'], " "));
                }
                Move::ToolChange(t) => {
                    if self.m6 {
                        let _ = writeln!(out, "T{} M6", t.number);
                    } else {
                        let _ =
                            writeln!(out, "(Change to tool {}: {}, {} mm, then resume)", t.number, t.name, t.diameter);
                        let _ = writeln!(out, "M0");
                    }
                    if self.length_offset {
                        let _ = writeln!(out, "G43 H{}", t.number);
                    }
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
                Move::Drill { at, bottom_z, retract_z, peck, feed } => {
                    if self.canned_cycles {
                        let (x, y, z, r, f) = (fmt(at.x), fmt(at.y), fmt(*bottom_z), fmt(*retract_z), fmt(*feed));
                        if *peck > 0.0 {
                            let _ = writeln!(out, "G98 G83 X{x} Y{y} Z{z} R{r} Q{} F{f}", fmt(*peck));
                        } else {
                            let _ = writeln!(out, "G98 G81 X{x} Y{y} Z{z} R{r} F{f}");
                        }
                        in_cycle = true;
                        last_feed = *feed;
                    } else {
                        // The cycle as plain moves: pecks back to the
                        // retract height, then down to just above the last
                        // depth at rapid.
                        let _ = writeln!(out, "G0 X{} Y{}", fmt(at.x), fmt(at.y));
                        let _ = writeln!(out, "G0 Z{}", fmt(*retract_z));
                        let step = if *peck > 0.0 { *peck } else { retract_z - bottom_z };
                        let mut z = *retract_z;
                        while z > *bottom_z + 1e-9 {
                            let next = (z - step).max(*bottom_z);
                            if z < *retract_z {
                                let _ = writeln!(out, "G0 Z{}", fmt((z + 0.5).min(*retract_z)));
                            }
                            let _ = writeln!(out, "G1 Z{} F{}", fmt(next), fmt(*feed));
                            let _ = writeln!(out, "G0 Z{}", fmt(*retract_z));
                            z = next;
                        }
                        last_feed = *feed;
                    }
                }
            }
        }
        if in_cycle {
            let _ = writeln!(out, "G80");
        }
        for line in self.footer {
            let _ = writeln!(out, "{line}");
        }
        if self.percent {
            let _ = writeln!(out, "%");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{drill, DrillParams};
    use crate::Tool;
    use anvil_math::{DVec2, DVec3};

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

    fn holes() -> Toolpath {
        let tool = Tool { number: 3, name: "5mm drill".into(), diameter: 5.0, rpm: 3000.0, feed: 300.0, plunge: 120.0 };
        let p = DrillParams { top_z: 0.0, depth: 10.0, peck: 3.0, retract: 2.0, clearance: 10.0 };
        drill(&[DVec2::new(10.0, 10.0), DVec2::new(30.0, 10.0)], &tool, &p)
    }

    #[test]
    fn linuxcnc_uses_peck_cycles_and_cancels_them() {
        let g = Dialect::linuxcnc().emit(&holes());
        assert_eq!(g.matches("G98 G83").count(), 2, "{g}");
        assert!(g.contains("Z-10 R2 Q3 F120"), "{g}");
        assert!(g.contains("G80"));
        assert!(g.contains("T3 M6") && g.contains("G43 H3"));
        assert!(g.contains("\nM2\n"));
    }

    #[test]
    fn grbl_gets_plain_moves_and_a_pause_for_the_tool() {
        let g = Dialect::grbl().emit(&holes());
        assert!(!g.contains("G83") && !g.contains("G81") && !g.contains("M6"), "{g}");
        assert!(g.contains("M0"));
        // Four pecks of 3 mm to 10 mm deep per hole, from 2 mm above.
        assert_eq!(g.matches("G1 Z").count(), 2 * 4, "{g}");
        assert!(g.contains("G1 Z-10"));
        assert!(!g.starts_with('%'));
    }

    #[test]
    fn fanuc_and_haas_have_a_program_number() {
        for d in [Dialect::fanuc(), Dialect::haas()] {
            let g = d.emit(&holes());
            assert!(g.starts_with("%\nO0001"), "{}", d.name);
            assert!(g.contains("M30"));
        }
    }

    #[test]
    fn every_post_has_a_name_and_writes_something() {
        for p in posts() {
            assert!(!p.name().is_empty());
            assert!(p.emit(&holes()).contains("G21"));
        }
    }
}
