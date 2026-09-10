//! Interactive sketch editor.
//!
//! The editor works on a copy of the sketch. Every finished tool action is
//! committed to the document as one undo step. Points that land on an
//! existing point are reused, so chained lines close into profiles.

use crate::camera::Projector;
use anvil_feature::features::sketch::SketchFeature;
use anvil_feature::Document;
use anvil_math::DVec2;
use anvil_sketch::{Constraint, Entity, EntityId, Sketch, SolveReport};
use egui::{Color32, Pos2, Stroke};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Point,
    Line,
    Rect2,
    RectCenter,
    CircleCenter,
    Circle2,
    Circle3,
    Arc3,
    ArcCenter,
    Polygon,
    Slot,
}

impl Tool {
    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Point => "Point",
            Tool::Line => "Line",
            Tool::Rect2 => "Rect 2-pt",
            Tool::RectCenter => "Rect centre",
            Tool::CircleCenter => "Circle centre",
            Tool::Circle2 => "Circle 2-pt",
            Tool::Circle3 => "Circle 3-pt",
            Tool::Arc3 => "Arc 3-pt",
            Tool::ArcCenter => "Arc centre",
            Tool::Polygon => "Polygon",
            Tool::Slot => "Slot",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            Tool::Select => "Click to select, Shift+click to add, drag a point to move it",
            Tool::Point => "Click to place a point",
            Tool::Line => "Click start, click next points; Esc or right-click to end",
            Tool::Rect2 => "Click two opposite corners",
            Tool::RectCenter => "Click the centre, then a corner",
            Tool::CircleCenter => "Click the centre, then a point on the circle",
            Tool::Circle2 => "Click two ends of a diameter",
            Tool::Circle3 => "Click three points on the circle",
            Tool::Arc3 => "Click start, a point on the arc, then the end",
            Tool::ArcCenter => "Click the centre, the start, then the end (counter-clockwise)",
            Tool::Polygon => "Click the centre, then a vertex. Sides in the ribbon field",
            Tool::Slot => "Click the two arc centres. Width in the ribbon field",
        }
    }
    pub const ALL: [Tool; 12] = [
        Tool::Select,
        Tool::Line,
        Tool::Rect2,
        Tool::RectCenter,
        Tool::CircleCenter,
        Tool::Circle2,
        Tool::Circle3,
        Tool::Arc3,
        Tool::ArcCenter,
        Tool::Polygon,
        Tool::Slot,
        Tool::Point,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstraintTool {
    Coincident,
    Horizontal,
    Vertical,
    Parallel,
    Perpendicular,
    Equal,
    Tangent,
    Midpoint,
    Concentric,
    Symmetric,
    PointOnCurve,
    Fix,
}

impl ConstraintTool {
    pub fn label(self) -> &'static str {
        match self {
            ConstraintTool::Coincident => "Coincident",
            ConstraintTool::Horizontal => "Horizontal",
            ConstraintTool::Vertical => "Vertical",
            ConstraintTool::Parallel => "Parallel",
            ConstraintTool::Perpendicular => "Perpendicular",
            ConstraintTool::Equal => "Equal",
            ConstraintTool::Tangent => "Tangent",
            ConstraintTool::Midpoint => "Midpoint",
            ConstraintTool::Concentric => "Concentric",
            ConstraintTool::Symmetric => "Symmetric",
            ConstraintTool::PointOnCurve => "On curve",
            ConstraintTool::Fix => "Fix",
        }
    }
    pub const ALL: [ConstraintTool; 12] = [
        ConstraintTool::Coincident,
        ConstraintTool::Horizontal,
        ConstraintTool::Vertical,
        ConstraintTool::Parallel,
        ConstraintTool::Perpendicular,
        ConstraintTool::Equal,
        ConstraintTool::Tangent,
        ConstraintTool::Midpoint,
        ConstraintTool::Concentric,
        ConstraintTool::Symmetric,
        ConstraintTool::PointOnCurve,
        ConstraintTool::Fix,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DimensionTool {
    /// Length of a line, or distance between two points.
    Length,
    Radius,
    Angle,
}

pub struct SketchEditor {
    pub feature: usize,
    pub sketch: Sketch,
    pub tool: Tool,
    pub clicks: Vec<DVec2>,
    /// Point ids reused for the clicks (None when a new point was created).
    pub click_points: Vec<Option<EntityId>>,
    pub selection: Vec<EntityId>,
    pub hover: Option<EntityId>,
    pub cursor: Option<DVec2>,
    pub dragging: Option<EntityId>,
    pub polygon_sides: usize,
    pub slot_width: f64,
    pub dim_value: String,
    pub report: Option<SolveReport>,
    pub message: String,
    pub grid: f64,
    pub snap_grid: bool,
    dirty: bool,
}

const PICK_TOL_PX: f64 = 8.0;

impl SketchEditor {
    pub fn open(doc: &Document, feature: usize) -> Option<Self> {
        let f = doc.features.get(feature)?.feature.downcast_ref::<SketchFeature>()?;
        let mut sketch = f.sketch.clone();
        if let Some(out) = &doc.features[feature].output {
            if let Some(p) = out.plane {
                sketch.plane = p;
            }
        }
        let report = sketch.solve();
        Some(SketchEditor {
            feature,
            sketch,
            tool: Tool::Line,
            clicks: Vec::new(),
            click_points: Vec::new(),
            selection: Vec::new(),
            hover: None,
            cursor: None,
            dragging: None,
            polygon_sides: 6,
            slot_width: 6.0,
            dim_value: "10".into(),
            report: Some(report),
            message: String::new(),
            grid: 5.0,
            snap_grid: false,
            dirty: false,
        })
    }

    /// Write the working sketch back into the document as one undo step.
    pub fn commit(&mut self, doc: &mut Document) {
        if !self.dirty {
            return;
        }
        let s = self.sketch.clone();
        doc.edit_feature(self.feature, |f| {
            if let Some(sf) = f.downcast_mut::<SketchFeature>() {
                sf.sketch = s;
            }
        });
        self.dirty = false;
    }

    fn solve(&mut self) {
        self.report = Some(self.sketch.solve());
        self.dirty = true;
    }

    pub fn set_tool(&mut self, t: Tool) {
        self.tool = t;
        self.clicks.clear();
        self.click_points.clear();
        self.message = t.hint().into();
    }

    pub fn cancel(&mut self) {
        self.clicks.clear();
        self.click_points.clear();
    }

    /// World-unit tolerance for picking at the current zoom.
    fn tol(&self, proj: &Projector) -> f64 {
        PICK_TOL_PX / proj.scale
    }

    fn snap(&self, p: DVec2, proj: &Projector) -> (DVec2, Option<EntityId>) {
        if let Some(id) = self.sketch.pick(p, self.tol(proj)) {
            if let Entity::Point { pos, .. } = self.sketch.entities[id] {
                return (pos, Some(id));
            }
        }
        if self.snap_grid && self.grid > 0.0 {
            return (DVec2::new((p.x / self.grid).round() * self.grid, (p.y / self.grid).round() * self.grid), None);
        }
        (p, None)
    }

    fn point_or_new(&mut self, p: DVec2, existing: Option<EntityId>) -> EntityId {
        existing.unwrap_or_else(|| self.sketch.add_point(p.x, p.y))
    }

    pub fn on_hover(&mut self, p: Option<DVec2>, proj: &Projector) {
        self.cursor = p.map(|q| self.snap(q, proj).0);
        self.hover = p.and_then(|q| self.sketch.pick(q, self.tol(proj)));
    }

    pub fn on_click(&mut self, p: DVec2, shift: bool, proj: &Projector, doc: &mut Document) {
        let (sp, existing) = self.snap(p, proj);
        match self.tool {
            Tool::Select => {
                let hit = self.sketch.pick(p, self.tol(proj));
                match hit {
                    Some(id) => {
                        if shift {
                            if let Some(i) = self.selection.iter().position(|&s| s == id) {
                                self.selection.remove(i);
                            } else {
                                self.selection.push(id);
                            }
                        } else {
                            self.selection = vec![id];
                        }
                    }
                    None => {
                        if !shift {
                            self.selection.clear();
                        }
                    }
                }
                return;
            }
            Tool::Point => {
                if existing.is_none() {
                    self.sketch.add_point(sp.x, sp.y);
                    self.solve();
                    self.commit(doc);
                }
                return;
            }
            _ => {}
        }
        self.clicks.push(sp);
        self.click_points.push(existing);
        let n = self.clicks.len();
        let c = self.clicks.clone();
        let cp = self.click_points.clone();
        let mut finished = false;
        match self.tool {
            Tool::Line => {
                if n >= 2 {
                    let a = self.point_or_new(c[n - 2], cp[n - 2]);
                    let b = self.point_or_new(c[n - 1], cp[n - 1]);
                    // Reuse ids for the chain.
                    self.click_points[n - 2] = Some(a);
                    self.click_points[n - 1] = Some(b);
                    self.sketch.add_line(a, b);
                    self.solve();
                    self.commit(doc);
                    // A click on an existing point ends the chain (closed loop).
                    if cp[n - 1].is_some() {
                        finished = true;
                    }
                }
            }
            Tool::Rect2 if n == 2 => {
                self.sketch.add_rectangle(c[0].x, c[0].y, c[1].x, c[1].y);
                finished = true;
            }
            Tool::RectCenter if n == 2 => {
                let d = c[1] - c[0];
                self.sketch.add_rectangle_center(c[0].x, c[0].y, 2.0 * d.x.abs(), 2.0 * d.y.abs());
                finished = true;
            }
            Tool::CircleCenter if n == 2 => {
                let ctr = self.point_or_new(c[0], cp[0]);
                self.sketch.add_circle(ctr, (c[1] - c[0]).length().max(1e-6));
                finished = true;
            }
            Tool::Circle2 if n == 2 => {
                self.sketch.add_circle_2pt(c[0], c[1]);
                finished = true;
            }
            Tool::Circle3 if n == 3 => {
                if self.sketch.add_circle_3pt(c[0], c[1], c[2]).is_none() {
                    self.message = "Points are collinear".into();
                }
                finished = true;
            }
            Tool::Arc3 if n == 3 => {
                if self.sketch.add_arc_3pt(c[0], c[1], c[2]).is_none() {
                    self.message = "Points are collinear".into();
                }
                finished = true;
            }
            Tool::ArcCenter if n == 3 => {
                self.sketch.add_arc_center(c[0], c[1], c[2]);
                finished = true;
            }
            Tool::Polygon if n == 2 => {
                let d = c[1] - c[0];
                self.sketch.add_polygon(c[0], d.length().max(1e-6), self.polygon_sides, d.y.atan2(d.x));
                finished = true;
            }
            Tool::Slot if n == 2 => {
                self.sketch.add_slot(c[0], c[1], self.slot_width.max(1e-3));
                finished = true;
            }
            _ => {}
        }
        if finished {
            self.solve();
            self.commit(doc);
            self.clicks.clear();
            self.click_points.clear();
        }
    }

    pub fn on_drag_start(&mut self, p: DVec2, proj: &Projector) {
        if self.tool != Tool::Select {
            return;
        }
        if let Some(id) = self.sketch.pick(p, self.tol(proj)) {
            if matches!(self.sketch.entities[id], Entity::Point { .. }) {
                self.dragging = Some(id);
            }
        }
    }

    pub fn on_drag(&mut self, p: DVec2) {
        if let Some(id) = self.dragging {
            if let Entity::Point { pos, .. } = &mut self.sketch.entities[id] {
                *pos = p;
            }
            self.report = Some(self.sketch.solve());
            self.dirty = true;
        }
    }

    pub fn on_drag_end(&mut self, doc: &mut Document) {
        if self.dragging.take().is_some() {
            self.commit(doc);
        }
    }

    pub fn delete_selection(&mut self, doc: &mut Document) {
        for id in std::mem::take(&mut self.selection) {
            if self.sketch.entities.contains_key(id) {
                self.sketch.remove_entity(id);
            }
        }
        self.solve();
        self.commit(doc);
    }

    pub fn toggle_construction(&mut self, doc: &mut Document) {
        for &id in &self.selection {
            if let Some(Entity::Line { construction, .. }) = self.sketch.entities.get_mut(id) {
                *construction = !*construction;
            }
        }
        self.solve();
        self.commit(doc);
    }

    fn kind(&self, id: EntityId) -> &'static str {
        match self.sketch.entities.get(id) {
            Some(Entity::Point { .. }) => "point",
            Some(Entity::Line { .. }) => "line",
            Some(Entity::Circle { .. }) | Some(Entity::Arc { .. }) => "circle",
            None => "none",
        }
    }

    /// Apply a constraint to the current selection. Returns a message.
    pub fn apply_constraint(&mut self, t: ConstraintTool, doc: &mut Document) {
        let sel = self.selection.clone();
        let kinds: Vec<&str> = sel.iter().map(|&i| self.kind(i)).collect();
        let c = match (t, kinds.as_slice()) {
            (ConstraintTool::Coincident, ["point", "point"]) => Some(Constraint::Coincident(sel[0], sel[1])),
            (ConstraintTool::Horizontal, ["line"]) => Some(Constraint::Horizontal(sel[0])),
            (ConstraintTool::Vertical, ["line"]) => Some(Constraint::Vertical(sel[0])),
            (ConstraintTool::Parallel, ["line", "line"]) => Some(Constraint::Parallel(sel[0], sel[1])),
            (ConstraintTool::Perpendicular, ["line", "line"]) => Some(Constraint::Perpendicular(sel[0], sel[1])),
            (ConstraintTool::Equal, ["line", "line"]) => Some(Constraint::EqualLength(sel[0], sel[1])),
            (ConstraintTool::Equal, ["circle", "circle"]) => Some(Constraint::EqualRadius(sel[0], sel[1])),
            (ConstraintTool::Tangent, ["line", "circle"]) => Some(Constraint::Tangent(sel[0], sel[1])),
            (ConstraintTool::Tangent, ["circle", "line"]) => Some(Constraint::Tangent(sel[1], sel[0])),
            (ConstraintTool::Midpoint, ["point", "line"]) => Some(Constraint::Midpoint(sel[0], sel[1])),
            (ConstraintTool::Midpoint, ["line", "point"]) => Some(Constraint::Midpoint(sel[1], sel[0])),
            (ConstraintTool::Concentric, ["circle", "circle"]) => Some(Constraint::Concentric(sel[0], sel[1])),
            (ConstraintTool::Symmetric, ["point", "point", "line"]) => {
                Some(Constraint::Symmetric(sel[0], sel[1], sel[2]))
            }
            (ConstraintTool::PointOnCurve, ["point", "line"]) => Some(Constraint::PointOnLine(sel[0], sel[1])),
            (ConstraintTool::PointOnCurve, ["point", "circle"]) => Some(Constraint::PointOnCircle(sel[0], sel[1])),
            (ConstraintTool::Fix, ["point"]) => Some(Constraint::Fix(sel[0])),
            _ => None,
        };
        match c {
            Some(c) => {
                // Horizontal/Vertical/Fix may apply to each selected item.
                self.sketch.constrain(c);
                if matches!(t, ConstraintTool::Horizontal | ConstraintTool::Vertical | ConstraintTool::Fix) {
                    for &id in sel.iter().skip(1) {
                        let extra = match (t, self.kind(id)) {
                            (ConstraintTool::Horizontal, "line") => Some(Constraint::Horizontal(id)),
                            (ConstraintTool::Vertical, "line") => Some(Constraint::Vertical(id)),
                            (ConstraintTool::Fix, "point") => Some(Constraint::Fix(id)),
                            _ => None,
                        };
                        if let Some(e) = extra {
                            self.sketch.constrain(e);
                        }
                    }
                }
                self.solve();
                self.commit(doc);
                self.message = format!("Added {}", t.label());
                self.selection.clear();
            }
            None => {
                self.message = format!("{} needs a different selection ({})", t.label(), kinds.join(", "));
            }
        }
    }

    pub fn apply_dimension(&mut self, t: DimensionTool, doc: &mut Document) {
        let Ok(v) = self.dim_value.trim().parse::<f64>() else {
            self.message = "Enter a number in the value field".into();
            return;
        };
        let sel = self.selection.clone();
        let kinds: Vec<&str> = sel.iter().map(|&i| self.kind(i)).collect();
        let c = match (t, kinds.as_slice()) {
            (DimensionTool::Length, ["line"]) => Some(Constraint::Length(sel[0], v)),
            (DimensionTool::Length, ["point", "point"]) => Some(Constraint::Distance(sel[0], sel[1], v)),
            (DimensionTool::Radius, ["circle"]) => Some(Constraint::Radius(sel[0], v)),
            (DimensionTool::Angle, ["line", "line"]) => Some(Constraint::Angle(sel[0], sel[1], v)),
            _ => None,
        };
        match c {
            Some(c) => {
                self.sketch.constrain(c);
                self.solve();
                self.commit(doc);
                self.message = "Dimension added".into();
                self.selection.clear();
            }
            None => {
                self.message =
                    format!("Dimension needs a line, two points, a circle, or two lines ({})", kinds.join(", "))
            }
        }
    }

    pub fn remove_constraints_on_selection(&mut self, doc: &mut Document) {
        let sel = self.selection.clone();
        self.sketch.constraints.retain(|_, c| !c.refs().iter().any(|r| sel.contains(r)));
        self.solve();
        self.commit(doc);
    }

    // ---------------- drawing ----------------

    pub fn draw(&self, painter: &egui::Painter, origin: Pos2, proj: &Projector) {
        let plane = self.sketch.plane;
        let to_screen = |p: DVec2| -> Option<Pos2> {
            proj.project(plane.to_world(p)).map(|(x, y, _)| Pos2::new(origin.x + x as f32, origin.y + y as f32))
        };
        // Grid.
        if self.grid > 0.0 && proj.scale * self.grid >= 6.0 {
            let half_w = proj.width / proj.scale;
            let half_h = proj.height / proj.scale;
            let c = plane.to_local(proj.eye + proj.fwd * 1.0);
            let c = DVec2::new(c.x, c.y);
            let n = ((half_w.max(half_h)) / self.grid).ceil() as i64 + 1;
            let gx = (c.x / self.grid).round() as i64;
            let gy = (c.y / self.grid).round() as i64;
            let faint = Stroke::new(0.5f32, Color32::from_rgb(205, 210, 218));
            let strong = Stroke::new(1.0f32, Color32::from_rgb(150, 155, 165));
            for i in -n..=n {
                let x = (gx + i) as f64 * self.grid;
                let y = (gy + i) as f64 * self.grid;
                if let (Some(a), Some(b)) =
                    (to_screen(DVec2::new(x, c.y - half_h)), to_screen(DVec2::new(x, c.y + half_h)))
                {
                    painter.line_segment([a, b], if x == 0.0 { strong } else { faint });
                }
                if let (Some(a), Some(b)) =
                    (to_screen(DVec2::new(c.x - half_w, y)), to_screen(DVec2::new(c.x + half_w, y)))
                {
                    painter.line_segment([a, b], if y == 0.0 { strong } else { faint });
                }
            }
        }
        let normal = Stroke::new(1.6f32, Color32::from_rgb(20, 40, 90));
        let constr = Stroke::new(1.0f32, Color32::from_rgb(120, 130, 150));
        let sel_stroke = Stroke::new(2.4f32, Color32::from_rgb(240, 150, 30));
        let hov_stroke = Stroke::new(2.2f32, Color32::from_rgb(60, 160, 240));
        let stroke_for = |id: EntityId, construction: bool| {
            if self.selection.contains(&id) {
                sel_stroke
            } else if self.hover == Some(id) {
                hov_stroke
            } else if construction {
                constr
            } else {
                normal
            }
        };
        for (id, e) in &self.sketch.entities {
            match e {
                Entity::Line { a, b, construction } => {
                    if let (Some(p), Some(q)) = (to_screen(self.sketch.point(*a)), to_screen(self.sketch.point(*b))) {
                        painter.line_segment([p, q], stroke_for(id, *construction));
                    }
                }
                Entity::Circle { center, radius } => {
                    let c = self.sketch.point(*center);
                    let pts: Vec<Pos2> = (0..=64)
                        .filter_map(|i| {
                            let t = i as f64 / 64.0 * std::f64::consts::TAU;
                            to_screen(c + DVec2::new(t.cos(), t.sin()) * *radius)
                        })
                        .collect();
                    painter.add(egui::Shape::line(pts, stroke_for(id, false)));
                }
                Entity::Arc { center, start, end } => {
                    let c = self.sketch.point(*center);
                    let s = self.sketch.point(*start);
                    let e = self.sketch.point(*end);
                    let r = (s - c).length();
                    let a0 = (s - c).y.atan2((s - c).x);
                    let mut a1 = (e - c).y.atan2((e - c).x);
                    if a1 <= a0 {
                        a1 += std::f64::consts::TAU;
                    }
                    let n = (((a1 - a0) / std::f64::consts::TAU) * 64.0).ceil().max(2.0) as usize;
                    let pts: Vec<Pos2> = (0..=n)
                        .filter_map(|i| {
                            let t = a0 + (a1 - a0) * i as f64 / n as f64;
                            to_screen(c + DVec2::new(t.cos(), t.sin()) * r)
                        })
                        .collect();
                    painter.add(egui::Shape::line(pts, stroke_for(id, false)));
                }
                Entity::Point { .. } => {}
            }
        }
        for (id, e) in &self.sketch.entities {
            if let Entity::Point { pos, .. } = e {
                if let Some(p) = to_screen(*pos) {
                    let fixed = self.sketch.constraints.values().any(|c| matches!(c, Constraint::Fix(f) if *f == id));
                    let col = if self.selection.contains(&id) {
                        Color32::from_rgb(240, 150, 30)
                    } else if self.hover == Some(id) {
                        Color32::from_rgb(60, 160, 240)
                    } else if fixed {
                        Color32::from_rgb(30, 30, 30)
                    } else {
                        Color32::from_rgb(20, 60, 160)
                    };
                    painter.rect_filled(egui::Rect::from_center_size(p, egui::vec2(6.0, 6.0)), 1.0, col);
                }
            }
        }
        // Constraint glyphs: a small label near the first referenced entity.
        for c in self.sketch.constraints.values() {
            let Some(&first) = c.refs().first() else { continue };
            let anchor = match self.sketch.entities.get(first) {
                Some(Entity::Point { pos, .. }) => Some(*pos),
                Some(Entity::Line { a, b, .. }) => Some((self.sketch.point(*a) + self.sketch.point(*b)) * 0.5),
                Some(Entity::Circle { center, radius }) => Some(self.sketch.point(*center) + DVec2::new(*radius, 0.0)),
                Some(Entity::Arc { center, start, .. }) => {
                    Some((self.sketch.point(*center) + self.sketch.point(*start)) * 0.5)
                }
                None => None,
            };
            if let Some(p) = anchor.and_then(to_screen) {
                let glyph = match c {
                    Constraint::Horizontal(_) => "H".to_string(),
                    Constraint::Vertical(_) => "V".to_string(),
                    Constraint::Parallel(..) => "//".to_string(),
                    Constraint::Perpendicular(..) => "T".to_string(),
                    Constraint::EqualLength(..) | Constraint::EqualRadius(..) => "=".to_string(),
                    Constraint::Tangent(..) => "tan".to_string(),
                    Constraint::Coincident(..) => "o".to_string(),
                    Constraint::Fix(_) => String::new(),
                    other => other.value().map(|v| format!("{v:.2}")).unwrap_or_else(|| other.label()),
                };
                if !glyph.is_empty() {
                    painter.text(
                        p + egui::vec2(6.0, -10.0),
                        egui::Align2::LEFT_BOTTOM,
                        glyph,
                        egui::FontId::proportional(11.0),
                        Color32::from_rgb(150, 60, 20),
                    );
                }
            }
        }
        // Tool preview.
        if let Some(cur) = self.cursor {
            let preview = Stroke::new(1.2f32, Color32::from_rgb(90, 90, 90));
            let c = &self.clicks;
            let mut segs: Vec<[DVec2; 2]> = Vec::new();
            let mut circles: Vec<(DVec2, f64)> = Vec::new();
            match (self.tool, c.len()) {
                (Tool::Line, n) if n >= 1 => segs.push([c[n - 1], cur]),
                (Tool::Rect2, 1) => {
                    segs.extend([
                        [c[0], DVec2::new(cur.x, c[0].y)],
                        [DVec2::new(cur.x, c[0].y), cur],
                        [cur, DVec2::new(c[0].x, cur.y)],
                        [DVec2::new(c[0].x, cur.y), c[0]],
                    ]);
                }
                (Tool::RectCenter, 1) => {
                    let d = cur - c[0];
                    let (a, b) = (c[0] - d, c[0] + d);
                    segs.extend([
                        [a, DVec2::new(b.x, a.y)],
                        [DVec2::new(b.x, a.y), b],
                        [b, DVec2::new(a.x, b.y)],
                        [DVec2::new(a.x, b.y), a],
                    ]);
                }
                (Tool::CircleCenter, 1) => circles.push((c[0], (cur - c[0]).length())),
                (Tool::Circle2, 1) => circles.push(((c[0] + cur) * 0.5, (cur - c[0]).length() / 2.0)),
                (Tool::Circle3, 2) | (Tool::Arc3, 2) => {
                    if let Some((ctr, r)) = anvil_sketch::circumcircle(c[0], c[1], cur) {
                        circles.push((ctr, r));
                    }
                }
                (Tool::ArcCenter, 1) => segs.push([c[0], cur]),
                (Tool::ArcCenter, 2) => circles.push((c[0], (c[1] - c[0]).length())),
                (Tool::Polygon, 1) => {
                    let d = cur - c[0];
                    let r = d.length();
                    let rot = d.y.atan2(d.x);
                    let n = self.polygon_sides.max(3);
                    for i in 0..n {
                        let t0 = rot + std::f64::consts::TAU * i as f64 / n as f64;
                        let t1 = rot + std::f64::consts::TAU * (i + 1) as f64 / n as f64;
                        segs.push([
                            c[0] + DVec2::new(t0.cos(), t0.sin()) * r,
                            c[0] + DVec2::new(t1.cos(), t1.sin()) * r,
                        ]);
                    }
                }
                (Tool::Slot, 1) => {
                    segs.push([c[0], cur]);
                    circles.push((c[0], self.slot_width / 2.0));
                    circles.push((cur, self.slot_width / 2.0));
                }
                _ => {}
            }
            for [a, b] in segs {
                if let (Some(p), Some(q)) = (to_screen(a), to_screen(b)) {
                    painter.line_segment([p, q], preview);
                }
            }
            for (ctr, r) in circles {
                let pts: Vec<Pos2> = (0..=48)
                    .filter_map(|i| {
                        let t = i as f64 / 48.0 * std::f64::consts::TAU;
                        to_screen(ctr + DVec2::new(t.cos(), t.sin()) * r)
                    })
                    .collect();
                painter.add(egui::Shape::line(pts, preview));
            }
            if let Some(p) = to_screen(cur) {
                painter.circle_stroke(p, 4.0, Stroke::new(1.0f32, Color32::from_rgb(60, 60, 60)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Camera;

    fn setup() -> (Document, SketchEditor, Projector) {
        let mut doc = Document::new("t");
        doc.add_feature(Box::new(SketchFeature::on_datum("XY")));
        let ed = SketchEditor::open(&doc, 0).unwrap();
        let mut cam = Camera::default();
        cam.look_at_plane(&anvil_math::Plane::XY, 100.0);
        let proj = Projector::new(&cam, 800.0, 600.0);
        (doc, ed, proj)
    }

    #[test]
    fn line_chain_closes_into_a_profile_and_extrudes() {
        let (mut doc, mut ed, proj) = setup();
        ed.set_tool(Tool::Line);
        for p in [(0.0, 0.0), (20.0, 0.0), (20.0, 10.0), (0.0, 10.0), (0.0, 0.0)] {
            ed.on_click(DVec2::new(p.0, p.1), false, &proj, &mut doc);
        }
        assert!(ed.clicks.is_empty(), "chain ends when it closes on the start point");
        let out = doc.features[0].output.as_ref().unwrap();
        assert_eq!(out.profiles.len(), 1);
        assert!((out.profiles[0].signed_area() - 200.0).abs() < 1e-9);
        doc.add_feature(Box::new(anvil_feature::features::extrude::ExtrudeFeature {
            sketch: 0,
            distance: "5".into(),
            symmetric: false,
        }));
        assert!((doc.bodies()[0].volume() - 1000.0).abs() < 1e-6);
    }

    #[test]
    fn dimension_and_constraint_flow() {
        let (mut doc, mut ed, proj) = setup();
        ed.set_tool(Tool::Rect2);
        ed.on_click(DVec2::new(0.0, 0.0), false, &proj, &mut doc);
        ed.on_click(DVec2::new(30.0, 20.0), false, &proj, &mut doc);
        assert_eq!(ed.sketch.entities.len(), 8);
        ed.set_tool(Tool::Select);
        // Select the bottom line by clicking its middle, then dimension it.
        ed.on_click(DVec2::new(15.0, 0.0), false, &proj, &mut doc);
        assert_eq!(ed.selection.len(), 1);
        ed.dim_value = "50".into();
        ed.apply_dimension(DimensionTool::Length, &mut doc);
        let out = doc.features[0].output.as_ref().unwrap();
        let w = out.profiles[0].points.iter().map(|p| p.x).fold(f64::MIN, f64::max)
            - out.profiles[0].points.iter().map(|p| p.x).fold(f64::MAX, f64::min);
        assert!((w - 50.0).abs() < 1e-6, "width {w}");
        // Fix a corner and check the report.
        ed.on_click(DVec2::new(0.0, 0.0), false, &proj, &mut doc);
        ed.apply_constraint(ConstraintTool::Fix, &mut doc);
        assert!(ed.report.as_ref().unwrap().dof > 0);
    }

    #[test]
    fn pixel_to_plane_round_trip() {
        let (_, _, proj) = setup();
        let p = proj.pixel_to_plane(400.0, 300.0, &anvil_math::Plane::XY).unwrap();
        assert!(p.length() < 1e-9, "{p:?}");
        let (x, y, _) = proj.project(anvil_math::Plane::XY.to_world(DVec2::new(10.0, 5.0))).unwrap();
        let q = proj.pixel_to_plane(x, y, &anvil_math::Plane::XY).unwrap();
        assert!((q - DVec2::new(10.0, 5.0)).length() < 1e-9, "{q:?}");
    }
}
