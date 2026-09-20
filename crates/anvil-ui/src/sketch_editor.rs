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
    MidpointLine,
    Rect2,
    RectCenter,
    Rect3,
    CircleCenter,
    Circle2,
    Circle3,
    Arc3,
    ArcCenter,
    Polygon,
    PolygonInscribed,
    PolygonEdge,
    Ellipse,
    Slot,
    SlotCenter,
    Spline,
    Fillet,
    Trim,
    Extend,
    Mirror,
    MoveCopy,
    ScaleSel,
    PatternRect,
    PatternCirc,
    Dimension,
    Chamfer,
    TangentArc,
}

impl Tool {
    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Point => "Point",
            Tool::Line => "Line",
            Tool::MidpointLine => "Midpoint Line",
            Tool::Rect2 => "2-Point Rectangle",
            Tool::RectCenter => "Center Rectangle",
            Tool::Rect3 => "3-Point Rectangle",
            Tool::CircleCenter => "Center Diameter Circle",
            Tool::Circle2 => "2-Point Circle",
            Tool::Circle3 => "3-Point Circle",
            Tool::Arc3 => "3-Point Arc",
            Tool::ArcCenter => "Center Point Arc",
            Tool::Polygon => "Circumscribed Polygon",
            Tool::PolygonInscribed => "Inscribed Polygon",
            Tool::PolygonEdge => "Edge Polygon",
            Tool::Ellipse => "Ellipse",
            Tool::Slot => "Center to Center Slot",
            Tool::SlotCenter => "Center Point Slot",
            Tool::Spline => "Spline",
            Tool::Fillet => "Fillet",
            Tool::Trim => "Trim",
            Tool::Extend => "Extend",
            Tool::Mirror => "Mirror",
            Tool::MoveCopy => "Move/Copy",
            Tool::ScaleSel => "Scale",
            Tool::PatternRect => "Rectangular Pattern",
            Tool::PatternCirc => "Circular Pattern",
            Tool::Dimension => "Dimension",
            Tool::Chamfer => "Chamfer",
            Tool::TangentArc => "Tangent Arc",
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
            Tool::MidpointLine => "Click the midpoint, then one end",
            Tool::Rect3 => "Click two corners of one edge, then a point on the opposite edge",
            Tool::PolygonInscribed => "Click the centre, then the middle of an edge. Sides in the ribbon field",
            Tool::PolygonEdge => "Click both ends of one edge. Sides in the ribbon field",
            Tool::Ellipse => "Click the centre, the end of the first axis, then a point on the ellipse",
            Tool::SlotCenter => "Click the slot centre, then one arc centre. Width in the ribbon field",
            Tool::Spline => "Click control points; click the first point to close; right-click to end",
            Tool::Fillet => "Click two lines that share a corner. Radius from the value field",
            Tool::Trim => "Click the piece of a line to remove",
            Tool::Extend => "Click near the end of a line to extend it to the next curve",
            Tool::Mirror => "Select entities, then click a mirror line",
            Tool::MoveCopy => "Select entities, click a base point, then a target point. Copy checkbox in the ribbon",
            Tool::ScaleSel => "Select entities, then click the scale centre. Factor from the value field",
            Tool::PatternRect => {
                "Select entities, click the end of the first step, then the end of the second. Counts in the ribbon"
            }
            Tool::PatternCirc => "Select entities, then click the pattern centre. Count and angle in the ribbon",
            Tool::Chamfer => "Click two lines that share a corner. Distance from the chamfer field",
            Tool::TangentArc => "Click the end of a line or arc, then the arc end point",
            Tool::Dimension => "Click a line, a circle, two points, or two lines; then type the value and press Enter. Click a label to edit it",
        }
    }
    /// Menu structure for the ribbon: (menu label, tools).
    pub const MENUS: [(&'static str, &'static [Tool]); 8] = [
        ("Line", &[Tool::Line, Tool::MidpointLine]),
        ("Rectangle", &[Tool::Rect2, Tool::RectCenter, Tool::Rect3]),
        ("Circle", &[Tool::CircleCenter, Tool::Circle2, Tool::Circle3]),
        ("Arc", &[Tool::Arc3, Tool::ArcCenter, Tool::TangentArc]),
        ("Polygon", &[Tool::Polygon, Tool::PolygonInscribed, Tool::PolygonEdge]),
        ("Slot", &[Tool::Slot, Tool::SlotCenter]),
        ("Curve", &[Tool::Ellipse, Tool::Spline]),
        ("Point", &[Tool::Point]),
    ];
    pub const MODIFY: [Tool; 9] = [
        Tool::Fillet,
        Tool::Chamfer,
        Tool::Trim,
        Tool::Extend,
        Tool::Mirror,
        Tool::MoveCopy,
        Tool::ScaleSel,
        Tool::PatternRect,
        Tool::PatternCirc,
    ];
    /// Used by the icon-coverage test in `icons.rs`.
    #[allow(dead_code)]
    pub const ALL: [Tool; 30] = [
        Tool::Select,
        Tool::Point,
        Tool::Line,
        Tool::MidpointLine,
        Tool::Rect2,
        Tool::RectCenter,
        Tool::Rect3,
        Tool::CircleCenter,
        Tool::Circle2,
        Tool::Circle3,
        Tool::Arc3,
        Tool::ArcCenter,
        Tool::Polygon,
        Tool::PolygonInscribed,
        Tool::PolygonEdge,
        Tool::Ellipse,
        Tool::Slot,
        Tool::SlotCenter,
        Tool::Spline,
        Tool::Fillet,
        Tool::Trim,
        Tool::Extend,
        Tool::Mirror,
        Tool::MoveCopy,
        Tool::ScaleSel,
        Tool::PatternRect,
        Tool::PatternCirc,
        Tool::Dimension,
        Tool::Chamfer,
        Tool::TangentArc,
    ];

    /// The icon id to look up in `crate::icons::paint`.
    pub fn icon_id(self) -> &'static str {
        match self {
            Tool::Select => "tool_select",
            Tool::Point => "tool_point",
            Tool::Line => "tool_line",
            Tool::MidpointLine => "tool_midpoint_line",
            Tool::Rect2 => "tool_rect2",
            Tool::RectCenter => "tool_rect_center",
            Tool::Rect3 => "tool_rect3",
            Tool::CircleCenter => "tool_circle_center",
            Tool::Circle2 => "tool_circle2",
            Tool::Circle3 => "tool_circle3",
            Tool::Arc3 => "tool_arc3",
            Tool::ArcCenter => "tool_arc_center",
            Tool::Polygon => "tool_polygon",
            Tool::PolygonInscribed => "tool_polygon_inscribed",
            Tool::PolygonEdge => "tool_polygon_edge",
            Tool::Ellipse => "tool_ellipse",
            Tool::Slot => "tool_slot",
            Tool::SlotCenter => "tool_slot_center",
            Tool::Spline => "tool_spline",
            Tool::Fillet => "tool_fillet",
            Tool::Trim => "tool_trim",
            Tool::Extend => "tool_extend",
            Tool::Mirror => "tool_mirror",
            Tool::MoveCopy => "tool_move_copy",
            Tool::ScaleSel => "tool_scale",
            Tool::PatternRect => "tool_pattern_rect",
            Tool::PatternCirc => "tool_pattern_circ",
            Tool::Dimension => "tool_dimension",
            Tool::Chamfer => "tool_chamfer",
            Tool::TangentArc => "tool_tangent_arc",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstraintTool {
    Coincident,
    Collinear,
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
            ConstraintTool::Collinear => "Collinear",
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
    pub const ALL: [ConstraintTool; 13] = [
        ConstraintTool::Coincident,
        ConstraintTool::Collinear,
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

    /// The icon id to look up in `crate::icons::paint`.
    pub fn icon_id(self) -> &'static str {
        match self {
            ConstraintTool::Coincident => "constraint_coincident",
            ConstraintTool::Collinear => "constraint_collinear",
            ConstraintTool::Horizontal => "constraint_horizontal",
            ConstraintTool::Vertical => "constraint_vertical",
            ConstraintTool::Parallel => "constraint_parallel",
            ConstraintTool::Perpendicular => "constraint_perpendicular",
            ConstraintTool::Equal => "constraint_equal",
            ConstraintTool::Tangent => "constraint_tangent",
            ConstraintTool::Midpoint => "constraint_midpoint",
            ConstraintTool::Concentric => "constraint_concentric",
            ConstraintTool::Symmetric => "constraint_symmetric",
            ConstraintTool::PointOnCurve => "constraint_point_on_curve",
            ConstraintTool::Fix => "constraint_fix",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DimensionTool {
    /// Length of a line, or distance between two points.
    Length,
    Radius,
    Angle,
    /// Picks Length, Radius, or Angle from the selection.
    Smart,
}

impl DimensionTool {
    /// Used by the icon-coverage test in `icons.rs`.
    #[allow(dead_code)]
    pub const ALL: [DimensionTool; 4] =
        [DimensionTool::Length, DimensionTool::Radius, DimensionTool::Angle, DimensionTool::Smart];

    /// The icon id to look up in `crate::icons::paint`.
    pub fn icon_id(self) -> &'static str {
        match self {
            DimensionTool::Length => "dim_length",
            DimensionTool::Radius => "dim_radius",
            DimensionTool::Angle => "dim_angle",
            DimensionTool::Smart => "dim_smart",
        }
    }
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
    pub count1: usize,
    pub count2: usize,
    pub pattern_angle: f64,
    pub copy: bool,
    /// Dimension label being edited (value in `dim_value`).
    pub editing: Option<anvil_sketch::ConstraintId>,
    /// Dimensions driven by expressions, saved with the sketch feature.
    pub dim_exprs: Vec<(anvil_sketch::ConstraintId, String)>,
    /// DXF file path for Import DXF.
    pub dxf_path: String,
    /// Chamfer distance for the sketch chamfer tool.
    pub chamfer: f64,
    /// Drag-box selection in sketch coordinates: (start, current).
    pub box_select: Option<(DVec2, DVec2)>,
    /// Where the cursor snapped and what kind: "point", "mid", "center", "curve", "grid".
    pub snap_kind: &'static str,
    pub show_points: bool,
    pub show_constraints: bool,
    pub show_grid: bool,
    pub snap_curves: bool,

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
        let dim_exprs = f.dim_exprs.clone();
        let mut sketch = f.sketch.clone();
        if let Some(out) = &doc.features[feature].output {
            if let Some(p) = out.plane {
                sketch.plane = p;
            }
        }
        for (cid, expr) in &dim_exprs {
            if let (Ok(v), Some(c)) = (doc.exprs.eval_str(expr), sketch.constraints.get_mut(*cid)) {
                c.set_value(v);
            }
        }
        let report = sketch.solve();
        Some(SketchEditor {
            dim_exprs,
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
            count1: 3,
            count2: 1,
            pattern_angle: 360.0,
            copy: true,
            editing: None,
            dxf_path: "logo.dxf".into(),
            chamfer: 1.0,
            box_select: None,
            snap_kind: "",
            show_points: true,
            show_constraints: true,
            show_grid: true,
            snap_curves: true,

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
        self.dim_exprs.retain(|(cid, _)| s.constraints.contains_key(*cid));
        let exprs = self.dim_exprs.clone();
        doc.edit_feature(self.feature, |f| {
            if let Some(sf) = f.downcast_mut::<SketchFeature>() {
                sf.sketch = s;
                sf.dim_exprs = exprs;
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

    /// Right-click or Esc. An open spline with 2+ points is committed.
    /// Returns true if something was committed.
    pub fn cancel(&mut self, doc: &mut Document) -> bool {
        let mut committed = false;
        if self.tool == Tool::Spline && self.clicks.len() >= 2 {
            let pts = self.clicks.clone();
            self.sketch.add_spline(&pts, false);
            self.solve();
            self.commit(doc);
            committed = true;
        }
        self.clicks.clear();
        self.click_points.clear();
        committed
    }

    /// World-unit tolerance for picking at the current zoom.
    fn tol(&self, proj: &Projector) -> f64 {
        PICK_TOL_PX / proj.scale
    }

    /// Snap candidates besides points: (position, kind).
    fn special_snaps(&self) -> Vec<(DVec2, &'static str)> {
        let mut out = Vec::new();
        for e in self.sketch.entities.values() {
            match e {
                Entity::Line { a, b, .. } => out.push(((self.sketch.point(*a) + self.sketch.point(*b)) * 0.5, "mid")),
                Entity::Circle { center, radius } => {
                    let c = self.sketch.point(*center);
                    out.push((c, "center"));
                    for d in [DVec2::X, DVec2::Y, -DVec2::X, -DVec2::Y] {
                        out.push((c + d * *radius, "quad"));
                    }
                }
                Entity::Arc { center, .. } | Entity::Ellipse { center, .. } => {
                    out.push((self.sketch.point(*center), "center"))
                }
                _ => {}
            }
        }
        out
    }

    fn snap_full(&self, p: DVec2, proj: &Projector) -> (DVec2, Option<EntityId>, &'static str) {
        let tol = self.tol(proj);
        if let Some(id) = self.sketch.pick(p, tol) {
            if let Entity::Point { pos, .. } = self.sketch.entities[id] {
                return (pos, Some(id), "point");
            }
        }
        if let Some((q, kind)) = self
            .special_snaps()
            .into_iter()
            .filter(|(q, _)| (*q - p).length() <= tol)
            .min_by(|a, b| (a.0 - p).length().partial_cmp(&(b.0 - p).length()).unwrap())
        {
            return (q, None, kind);
        }
        if self.snap_grid && self.grid > 0.0 {
            return (
                DVec2::new((p.x / self.grid).round() * self.grid, (p.y / self.grid).round() * self.grid),
                None,
                "grid",
            );
        }
        if self.snap_curves {
            if let Some(q) = self.sketch.nearest_on_curve(p, tol) {
                return (q, None, "curve");
            }
        }
        (p, None, "")
    }

    fn snap(&self, p: DVec2, proj: &Projector) -> (DVec2, Option<EntityId>) {
        let (q, id, kind) = self.snap_full(p, proj);
        if id.is_some() || !kind.is_empty() {
            return (q, id);
        }
        if self.snap_grid && self.grid > 0.0 {
            return (DVec2::new((p.x / self.grid).round() * self.grid, (p.y / self.grid).round() * self.grid), None);
        }
        if self.snap_curves {
            if let Some(q) = self.sketch.nearest_on_curve(p, self.tol(proj)) {
                return (q, None);
            }
        }
        (p, None)
    }

    fn point_or_new(&mut self, p: DVec2, existing: Option<EntityId>) -> EntityId {
        existing.unwrap_or_else(|| self.sketch.add_point(p.x, p.y))
    }

    pub fn on_hover(&mut self, p: Option<DVec2>, proj: &Projector) {
        match p {
            Some(q) => {
                let (sp, _, kind) = self.snap_full(q, proj);
                self.cursor = Some(sp);
                self.snap_kind = kind;
                self.hover = self.sketch.pick(q, self.tol(proj));
            }
            None => {
                self.cursor = None;
                self.hover = None;
                self.snap_kind = "";
            }
        }
    }

    /// Screen positions of dimension labels, for click-to-edit.
    pub fn label_positions(&self, proj: &Projector) -> Vec<(anvil_sketch::ConstraintId, DVec2)> {
        let mut out = Vec::new();
        for (cid, c) in &self.sketch.constraints {
            if c.value().is_none() {
                continue;
            }
            let Some(&first) = c.refs().first() else { continue };
            if let Some(a) = self.anchor_of(first) {
                out.push((cid, a));
            }
        }
        let _ = proj;
        out
    }

    fn anchor_of(&self, id: EntityId) -> Option<DVec2> {
        match self.sketch.entities.get(id) {
            Some(Entity::Point { pos, .. }) => Some(*pos),
            Some(Entity::Line { a, b, .. }) => Some((self.sketch.point(*a) + self.sketch.point(*b)) * 0.5),
            Some(Entity::Circle { center, radius }) => Some(self.sketch.point(*center) + DVec2::new(*radius, 0.0)),
            Some(Entity::Arc { center, start, .. }) => {
                Some((self.sketch.point(*center) + self.sketch.point(*start)) * 0.5)
            }
            Some(Entity::Ellipse { center, .. }) => Some(self.sketch.point(*center)),
            Some(Entity::Spline { points, .. }) => points.first().map(|&p| self.sketch.point(p)),
            None => None,
        }
    }

    /// Read the value field as a number, or as an expression over the
    /// document parameters. Returns (value, Some(expression) if not a plain number).
    fn value_or_expr(&mut self, doc: &Document) -> Option<(f64, Option<String>)> {
        let text = self.dim_value.trim().to_string();
        if let Ok(v) = text.parse::<f64>() {
            return Some((v, None));
        }
        match doc.exprs.eval_str(&text) {
            Ok(v) => Some((v, Some(text))),
            Err(e) => {
                self.message = format!("Not a number or expression: {e}");
                None
            }
        }
    }

    /// Import LINE, CIRCLE, ARC, and LWPOLYLINE entities from a DXF file,
    /// placed at the sketch origin in drawing units (assumed mm).
    pub fn import_dxf(&mut self, doc: &mut Document) {
        match std::fs::read_to_string(self.dxf_path.trim()) {
            Err(e) => self.message = format!("DXF: {e}"),
            Ok(text) => {
                let n = crate::dxf::import(&text, &mut self.sketch);
                self.solve();
                self.commit(doc);
                self.message = format!("Imported {n} entities from {}", self.dxf_path.trim());
            }
        }
    }

    /// Apply the value field to the dimension being edited.
    pub fn commit_edit(&mut self, doc: &mut Document) {
        let Some(cid) = self.editing else { return };
        let Some((v, expr)) = self.value_or_expr(doc) else { return };
        if let Some(c) = self.sketch.constraints.get_mut(cid) {
            c.set_value(v);
        }
        self.dim_exprs.retain(|(c, _)| *c != cid);
        if let Some(e) = expr {
            self.dim_exprs.push((cid, e));
        }
        self.editing = None;
        self.solve();
        self.commit(doc);
        self.message = "Dimension updated".into();
    }

    /// Start editing the dimension label nearest to `p`, if any.
    pub fn try_edit_label(&mut self, p: DVec2, proj: &Projector) -> bool {
        let tol = self.tol(proj) * 2.0;
        let hit = self
            .label_positions(proj)
            .into_iter()
            .filter(|(_, a)| (*a - p).length() <= tol)
            .min_by(|a, b| (a.1 - p).length().partial_cmp(&(b.1 - p).length()).unwrap());
        if let Some((cid, _)) = hit {
            self.editing = Some(cid);
            self.dim_value = format!("{:.3}", self.sketch.constraints[cid].value().unwrap_or(0.0));
            self.message = "Type the new value and press Enter".into();
            return true;
        }
        false
    }

    /// Box selection: window (all inside) when dragged left to right,
    /// crossing (any inside) when dragged right to left.
    pub fn finish_box_select(&mut self, shift: bool) {
        let Some((a, b)) = self.box_select.take() else { return };
        let (lo, hi) = (a.min(b), a.max(b));
        let crossing = b.x < a.x;
        if !shift {
            self.selection.clear();
        }
        let inside = |p: DVec2| p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y;
        for (id, e) in &self.sketch.entities {
            let pts = self.sketch.sample(id);
            let hit = if matches!(e, Entity::Point { .. }) {
                inside(pts[0])
            } else if crossing {
                pts.iter().any(|&p| inside(p))
            } else {
                pts.iter().all(|&p| inside(p))
            };
            if hit && !self.selection.contains(&id) {
                self.selection.push(id);
            }
        }
    }

    pub fn on_click(&mut self, p: DVec2, shift: bool, proj: &Projector, doc: &mut Document) {
        let (sp, existing) = self.snap(p, proj);
        match self.tool {
            Tool::Dimension => {
                if self.try_edit_label(p, proj) {
                    return;
                }
                let hit = self.sketch.pick(p, self.tol(proj));
                let Some(id) = hit else {
                    self.selection.clear();
                    return;
                };
                if !self.selection.contains(&id) {
                    self.selection.push(id);
                }
                let kinds: Vec<&str> = self.selection.iter().map(|&i| self.kind(i)).collect();
                let sel = self.selection.clone();
                let c = match kinds.as_slice() {
                    ["line"] => {
                        let (a, b) = match self.sketch.entities[sel[0]] {
                            Entity::Line { a, b, .. } => (a, b),
                            _ => unreachable!(),
                        };
                        Some(Constraint::Length(sel[0], (self.sketch.point(a) - self.sketch.point(b)).length()))
                    }
                    ["circle"] => {
                        let r = match &self.sketch.entities[sel[0]] {
                            Entity::Circle { radius, .. } => *radius,
                            Entity::Arc { center, start, .. } => {
                                (self.sketch.point(*start) - self.sketch.point(*center)).length()
                            }
                            _ => 0.0,
                        };
                        Some(Constraint::Radius(sel[0], r))
                    }
                    ["point", "point"] => Some(Constraint::Distance(
                        sel[0],
                        sel[1],
                        (self.sketch.point(sel[0]) - self.sketch.point(sel[1])).length(),
                    )),
                    ["line", "line"] => {
                        let dir = |id: EntityId| match self.sketch.entities[id] {
                            Entity::Line { a, b, .. } => {
                                (self.sketch.point(b) - self.sketch.point(a)).normalize_or_zero()
                            }
                            _ => DVec2::X,
                        };
                        let (u, v) = (dir(sel[0]), dir(sel[1]));
                        Some(Constraint::Angle(sel[0], sel[1], u.perp_dot(v).atan2(u.dot(v)).to_degrees()))
                    }
                    ["point"] => None,
                    _ => {
                        self.selection = vec![id];
                        None
                    }
                };
                if let Some(c) = c {
                    let v = c.value().unwrap_or(0.0);
                    let cid = self.sketch.constrain(c);
                    self.editing = Some(cid);
                    self.dim_value = format!("{v:.3}");
                    self.selection.clear();
                    self.solve();
                    self.commit(doc);
                    self.message = "Type the value and press Enter (or leave as measured)".into();
                }
                return;
            }
            Tool::Select => {
                if self.try_edit_label(p, proj) {
                    return;
                }
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
            Tool::Trim | Tool::Extend => {
                let hit = self.sketch.pick(p, self.tol(proj));
                let Some(id) = hit else {
                    self.message = "Click on a line".into();
                    return;
                };
                let r =
                    if self.tool == Tool::Trim { self.sketch.trim_line(id, p) } else { self.sketch.extend_line(id, p) };
                match r {
                    Ok(()) => {
                        self.solve();
                        self.commit(doc);
                    }
                    Err(e) => self.message = e,
                }
                return;
            }
            Tool::Fillet => {
                let hit = self.sketch.pick(p, self.tol(proj));
                let Some(id) = hit.filter(|&i| matches!(self.sketch.entities[i], Entity::Line { .. })) else {
                    self.message = "Click a line".into();
                    return;
                };
                if !self.selection.contains(&id) {
                    self.selection.push(id);
                }
                if self.selection.len() >= 2 {
                    let r = self.value().unwrap_or(1.0);
                    let (l1, l2) = (self.selection[0], self.selection[1]);
                    match self.sketch.fillet_lines(l1, l2, r) {
                        Ok(_) => {
                            self.solve();
                            self.commit(doc);
                            self.message = "Fillet added".into();
                        }
                        Err(e) => self.message = e,
                    }
                    self.selection.clear();
                }
                return;
            }
            Tool::Chamfer => {
                let hit = self.sketch.pick(p, self.tol(proj));
                let Some(id) = hit.filter(|&i| matches!(self.sketch.entities[i], Entity::Line { .. })) else {
                    self.message = "Click a line".into();
                    return;
                };
                if !self.selection.contains(&id) {
                    self.selection.push(id);
                }
                if self.selection.len() >= 2 {
                    let (l1, l2) = (self.selection[0], self.selection[1]);
                    match self.sketch.chamfer_lines(l1, l2, self.chamfer) {
                        Ok(_) => {
                            self.solve();
                            self.commit(doc);
                            self.message = "Chamfer added".into();
                        }
                        Err(e) => self.message = e,
                    }
                    self.selection.clear();
                }
                return;
            }
            Tool::Mirror => {
                let hit = self.sketch.pick(p, self.tol(proj));
                match hit.and_then(|i| match self.sketch.entities[i] {
                    Entity::Line { a, b, .. } => Some((self.sketch.point(a), self.sketch.point(b))),
                    _ => None,
                }) {
                    Some((a, b)) if !self.selection.is_empty() => {
                        let sel = self.selection.clone();
                        let out = self.sketch.mirror_entities(&sel, a, b);
                        self.selection = out;
                        self.solve();
                        self.commit(doc);
                        self.message = "Mirrored".into();
                    }
                    Some(_) => self.message = "Select entities first, then click the mirror line".into(),
                    None => self.message = "Click a line to mirror across".into(),
                }
                return;
            }
            Tool::ScaleSel => {
                if self.selection.is_empty() {
                    self.message = "Select entities first".into();
                    return;
                }
                let f = self.value().unwrap_or(2.0);
                let sel = self.selection.clone();
                let out = self.sketch.scale_entities(&sel, sp, f, self.copy);
                self.selection = out;
                self.solve();
                self.commit(doc);
                self.message = "Scaled".into();
                return;
            }
            Tool::PatternCirc => {
                if self.selection.is_empty() {
                    self.message = "Select entities first".into();
                    return;
                }
                let sel = self.selection.clone();
                self.sketch.pattern_circ(&sel, sp, self.count1, self.pattern_angle.to_radians());
                self.solve();
                self.commit(doc);
                self.message = "Pattern added".into();
                return;
            }
            Tool::MoveCopy | Tool::PatternRect if self.selection.is_empty() => {
                self.message = "Select entities first (Select tool), then use this tool".into();
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
            Tool::TangentArc if n == 1 && cp[0].is_none() => {
                self.message = "Start the tangent arc on the end of a line or arc".into();
                self.clicks.clear();
                self.click_points.clear();
            }
            Tool::TangentArc if n == 2 => {
                if let Some(start) = cp[0] {
                    match self.sketch.add_tangent_arc(start, c[1]) {
                        Ok(_) => {}
                        Err(e) => self.message = e,
                    }
                }
                finished = true;
            }
            Tool::Slot if n == 2 => {
                self.sketch.add_slot(c[0], c[1], self.slot_width.max(1e-3));
                finished = true;
            }
            Tool::SlotCenter if n == 2 => {
                self.sketch.add_slot_center_point(c[0], c[1], self.slot_width.max(1e-3));
                finished = true;
            }
            Tool::MidpointLine if n == 2 => {
                self.sketch.add_line_midpoint(c[0], c[1]);
                finished = true;
            }
            Tool::Rect3 if n == 3 => {
                self.sketch.add_rectangle_3pt(c[0], c[1], c[2]);
                finished = true;
            }
            Tool::PolygonInscribed if n == 2 => {
                let d = c[1] - c[0];
                self.sketch.add_polygon_inscribed(c[0], d.length().max(1e-6), self.polygon_sides, d.y.atan2(d.x));
                finished = true;
            }
            Tool::PolygonEdge if n == 2 => {
                self.sketch.add_polygon_edge(c[0], c[1], self.polygon_sides);
                finished = true;
            }
            Tool::Ellipse if n == 3 => {
                let a = c[1] - c[0];
                let rot = a.y.atan2(a.x);
                let rx = a.length().max(1e-6);
                // Solve ry so the ellipse passes through the third click.
                let v = c[2] - c[0];
                let (s_, co) = rot.sin_cos();
                let lx = v.x * co + v.y * s_;
                let ly = -v.x * s_ + v.y * co;
                let k = 1.0 - (lx / rx).powi(2);
                let ry = if k > 1e-9 { (ly.abs() / k.sqrt()).max(1e-6) } else { rx * 0.5 };
                self.sketch.add_ellipse(c[0], rx, ry, rot);
                finished = true;
            }
            Tool::Spline => {
                // Closing click on the first point finishes as a closed spline.
                if n >= 3 && (c[n - 1] - c[0]).length() < self.tol(proj) * 1.5 {
                    let pts: Vec<DVec2> = c[..n - 1].to_vec();
                    self.sketch.add_spline(&pts, true);
                    finished = true;
                }
            }
            Tool::MoveCopy if n == 2 => {
                let sel = self.selection.clone();
                let out = self.sketch.move_entities(&sel, c[1] - c[0], self.copy);
                self.selection = out;
                finished = true;
            }
            Tool::PatternRect if n == 3 => {
                let sel = self.selection.clone();
                self.sketch.pattern_rect(&sel, c[1] - c[0], self.count1, c[2] - c[0], self.count2);
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
        match self.sketch.pick(p, self.tol(proj)) {
            Some(id) if matches!(self.sketch.entities[id], Entity::Point { .. }) => self.dragging = Some(id),
            Some(_) => {}
            None => self.box_select = Some((p, p)),
        }
    }

    pub fn on_drag(&mut self, p: DVec2) {
        if let Some((a, _)) = self.box_select {
            self.box_select = Some((a, p));
            return;
        }
        if let Some(id) = self.dragging {
            if let Entity::Point { pos, .. } = &mut self.sketch.entities[id] {
                *pos = p;
            }
            self.report = Some(self.sketch.solve());
            self.dirty = true;
        }
    }

    pub fn on_drag_end(&mut self, doc: &mut Document, shift: bool) {
        if self.box_select.is_some() {
            self.finish_box_select(shift);
            return;
        }
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
            Some(Entity::Ellipse { .. }) => "ellipse",
            Some(Entity::Spline { .. }) => "spline",
            None => "none",
        }
    }

    pub fn value(&self) -> Option<f64> {
        self.dim_value.trim().parse().ok()
    }

    /// Offset the selection by the value field.
    pub fn offset_selection(&mut self, doc: &mut Document) {
        let Some(d) = self.value() else {
            self.message = "Enter the offset distance in the value field".into();
            return;
        };
        if self.selection.is_empty() {
            self.message = "Select entities to offset first".into();
            return;
        }
        let sel = self.selection.clone();
        let out = self.sketch.offset_entities(&sel, d);
        self.selection = out;
        self.solve();
        self.commit(doc);
        self.message = "Offset added".into();
    }

    /// Project 3D segments that lie in the sketch plane.
    pub fn project(&mut self, segs: &[[anvil_math::DVec3; 2]], doc: &mut Document) {
        let n = self.sketch.project_segments(segs, 1e-4);
        if n > 0 {
            self.solve();
            self.commit(doc);
        }
        self.message = format!("Projected {n} edges");
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
            (ConstraintTool::Collinear, ["line", "line"]) => Some(Constraint::Collinear(sel[0], sel[1])),
            (ConstraintTool::Perpendicular, ["line", "line"]) => Some(Constraint::Perpendicular(sel[0], sel[1])),
            (ConstraintTool::Equal, ["line", "line"]) => Some(Constraint::EqualLength(sel[0], sel[1])),
            (ConstraintTool::Equal, ["circle", "circle"]) => Some(Constraint::EqualRadius(sel[0], sel[1])),
            (ConstraintTool::Tangent, ["line", "circle"]) => Some(Constraint::Tangent(sel[0], sel[1])),
            (ConstraintTool::Tangent, ["circle", "line"]) => Some(Constraint::Tangent(sel[1], sel[0])),
            (ConstraintTool::Tangent, ["circle", "circle"]) => {
                Some(Constraint::TangentCircles(sel[0], sel[1], self.sketch.circles_nested(sel[0], sel[1])))
            }
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
        let Some((v, expr)) = self.value_or_expr(doc) else { return };
        let sel = self.selection.clone();
        let kinds: Vec<&str> = sel.iter().map(|&i| self.kind(i)).collect();
        let t = if t == DimensionTool::Smart {
            match kinds.as_slice() {
                ["circle"] => DimensionTool::Radius,
                ["line", "line"] => DimensionTool::Angle,
                _ => DimensionTool::Length,
            }
        } else {
            t
        };
        let c = match (t, kinds.as_slice()) {
            (DimensionTool::Length, ["line"]) => Some(Constraint::Length(sel[0], v)),
            (DimensionTool::Length, ["point", "point"]) => Some(Constraint::Distance(sel[0], sel[1], v)),
            (DimensionTool::Radius, ["circle"]) => Some(Constraint::Radius(sel[0], v)),
            (DimensionTool::Angle, ["line", "line"]) => Some(Constraint::Angle(sel[0], sel[1], v)),
            _ => None,
        };
        match c {
            Some(c) => {
                let cid = self.sketch.constrain(c);
                if let Some(e) = expr {
                    self.dim_exprs.push((cid, e));
                }
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
        let style = painter.ctx().style();
        let vis = &style.visuals;
        let plane = self.sketch.plane;
        let to_screen = |p: DVec2| -> Option<Pos2> {
            proj.project(plane.to_world(p)).map(|(x, y, _)| Pos2::new(origin.x + x as f32, origin.y + y as f32))
        };
        // Grid.
        if self.show_grid && self.grid > 0.0 && proj.scale * self.grid >= 6.0 {
            let half_w = proj.width / proj.scale;
            let half_h = proj.height / proj.scale;
            let c = plane.to_local(proj.eye + proj.fwd * 1.0);
            let c = DVec2::new(c.x, c.y);
            let n = ((half_w.max(half_h)) / self.grid).ceil() as i64 + 1;
            let gx = (c.x / self.grid).round() as i64;
            let gy = (c.y / self.grid).round() as i64;
            let faint = Stroke::new(0.5f32, vis.weak_text_color().gamma_multiply(0.5));
            let strong = Stroke::new(1.0f32, vis.weak_text_color());
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
        let normal = Stroke::new(1.6f32, vis.text_color());
        let constr = Stroke::new(1.0f32, vis.weak_text_color());
        let sel_stroke = Stroke::new(2.4f32, vis.warn_fg_color);
        let hov_stroke = Stroke::new(2.2f32, vis.hyperlink_color);
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
                Entity::Point { .. } => {}
                Entity::Line { a, b, construction } => {
                    if let (Some(p), Some(q)) = (to_screen(self.sketch.point(*a)), to_screen(self.sketch.point(*b))) {
                        painter.line_segment([p, q], stroke_for(id, *construction));
                    }
                }
                _ => {
                    let pts: Vec<Pos2> = self.sketch.sample(id).into_iter().filter_map(to_screen).collect();
                    painter.add(egui::Shape::line(pts, stroke_for(id, false)));
                }
            }
        }
        if self.show_points {
            for (id, e) in &self.sketch.entities {
                if let Entity::Point { pos, .. } = e {
                    if let Some(p) = to_screen(*pos) {
                        let fixed =
                            self.sketch.constraints.values().any(|c| matches!(c, Constraint::Fix(f) if *f == id));
                        let col = if self.selection.contains(&id) {
                            vis.warn_fg_color
                        } else if self.hover == Some(id) {
                            vis.hyperlink_color
                        } else if fixed {
                            vis.text_color()
                        } else {
                            vis.weak_text_color()
                        };
                        painter.rect_filled(egui::Rect::from_center_size(p, egui::vec2(6.0, 6.0)), 1.0, col);
                    }
                }
            }
        }
        // Constraint glyphs: a small label near the first referenced entity.
        for (cid, c) in self.sketch.constraints.iter().filter(|_| self.show_constraints) {
            let editing = self.editing == Some(cid);
            let Some(&first) = c.refs().first() else { continue };
            let anchor = match self.sketch.entities.get(first) {
                Some(Entity::Point { pos, .. }) => Some(*pos),
                Some(Entity::Line { a, b, .. }) => Some((self.sketch.point(*a) + self.sketch.point(*b)) * 0.5),
                Some(Entity::Circle { center, radius }) => Some(self.sketch.point(*center) + DVec2::new(*radius, 0.0)),
                Some(Entity::Arc { center, start, .. }) => {
                    Some((self.sketch.point(*center) + self.sketch.point(*start)) * 0.5)
                }
                Some(Entity::Ellipse { center, .. }) => Some(self.sketch.point(*center)),
                Some(Entity::Spline { points, .. }) => points.first().map(|&p| self.sketch.point(p)),
                None => None,
            };
            if let Some(p) = anchor.and_then(to_screen) {
                let glyph = match c {
                    Constraint::Horizontal(_) => "H".to_string(),
                    Constraint::Vertical(_) => "V".to_string(),
                    Constraint::Parallel(..) => "//".to_string(),
                    Constraint::Perpendicular(..) => "T".to_string(),
                    Constraint::EqualLength(..) | Constraint::EqualRadius(..) => "=".to_string(),
                    Constraint::Tangent(..) | Constraint::TangentCircles(..) => "tan".to_string(),
                    Constraint::Coincident(..) => "o".to_string(),
                    Constraint::Fix(_) => String::new(),
                    other => match self.dim_exprs.iter().find(|(c, _)| *c == cid) {
                        Some((_, e)) => format!("{e} = {:.2}", other.value().unwrap_or(0.0)),
                        None => other.value().map(|v| format!("{v:.2}")).unwrap_or_else(|| other.label()),
                    },
                };
                if !glyph.is_empty() {
                    let col = if editing { vis.warn_fg_color } else { vis.weak_text_color() };
                    let size = if c.value().is_some() { 12.5 } else { 11.0 };
                    painter.text(
                        p + egui::vec2(6.0, -10.0),
                        egui::Align2::LEFT_BOTTOM,
                        glyph,
                        egui::FontId::proportional(size),
                        col,
                    );
                }
            }
        }
        // Tool preview.
        if let Some(cur) = self.cursor {
            let preview = Stroke::new(1.2f32, vis.weak_text_color());
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
                (Tool::SlotCenter, 1) => {
                    let d = cur - c[0];
                    segs.push([c[0] - d, c[0] + d]);
                    circles.push((c[0] - d, self.slot_width / 2.0));
                    circles.push((c[0] + d, self.slot_width / 2.0));
                }
                (Tool::MidpointLine, 1) => segs.push([c[0] * 2.0 - cur, cur]),
                (Tool::Rect3, 1) => segs.push([c[0], cur]),
                (Tool::Rect3, 2) => {
                    let d = (c[1] - c[0]).normalize_or_zero();
                    let nn = DVec2::new(-d.y, d.x);
                    let h = (cur - c[0]).dot(nn);
                    let (p2, p3) = (c[1] + nn * h, c[0] + nn * h);
                    segs.extend([[c[0], c[1]], [c[1], p2], [p2, p3], [p3, c[0]]]);
                }
                (Tool::PolygonInscribed, 1) | (Tool::PolygonEdge, 1) => segs.push([c[0], cur]),
                (Tool::Ellipse, 1) => segs.push([c[0], cur]),
                (Tool::Ellipse, 2) => {
                    let a = c[1] - c[0];
                    let rot = a.y.atan2(a.x);
                    let rx = a.length().max(1e-6);
                    let ry = ((cur - c[0]).perp_dot(a.normalize_or_zero())).abs().max(1e-6);
                    let (sr, cr) = rot.sin_cos();
                    let pts: Vec<DVec2> = (0..=48)
                        .map(|i| {
                            let t = i as f64 / 48.0 * std::f64::consts::TAU;
                            let (x, y) = (rx * t.cos(), ry * t.sin());
                            c[0] + DVec2::new(x * cr - y * sr, x * sr + y * cr)
                        })
                        .collect();
                    for w in pts.windows(2) {
                        segs.push([w[0], w[1]]);
                    }
                }
                (Tool::Spline, n) if n >= 1 => {
                    let mut pts = c.clone();
                    pts.push(cur);
                    for w in anvil_sketch::catmull_rom(&pts, false, 8).windows(2) {
                        segs.push([w[0], w[1]]);
                    }
                }
                (Tool::MoveCopy, 1) | (Tool::PatternRect, 1) | (Tool::PatternRect, 2) => segs.push([c[0], cur]),
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
                let col = vis.text_color();
                let snap_col = vis.warn_fg_color;
                match self.snap_kind {
                    "point" => {
                        painter.rect_stroke(
                            egui::Rect::from_center_size(p, egui::vec2(10.0, 10.0)),
                            0.0,
                            Stroke::new(1.5f32, snap_col),
                            egui::StrokeKind::Middle,
                        );
                    }
                    "mid" => {
                        painter.add(egui::Shape::closed_line(
                            vec![p + egui::vec2(0.0, -6.0), p + egui::vec2(6.0, 5.0), p + egui::vec2(-6.0, 5.0)],
                            Stroke::new(1.5f32, snap_col),
                        ));
                    }
                    "center" => {
                        painter.circle_stroke(p, 6.0, Stroke::new(1.5f32, snap_col));
                    }
                    "quad" => {
                        painter.add(egui::Shape::closed_line(
                            vec![
                                p + egui::vec2(0.0, -6.0),
                                p + egui::vec2(6.0, 0.0),
                                p + egui::vec2(0.0, 6.0),
                                p + egui::vec2(-6.0, 0.0),
                            ],
                            Stroke::new(1.5f32, snap_col),
                        ));
                    }
                    "curve" => {
                        painter.line_segment(
                            [p + egui::vec2(-5.0, -5.0), p + egui::vec2(5.0, 5.0)],
                            Stroke::new(1.5f32, snap_col),
                        );
                    }
                    _ => {
                        painter.circle_stroke(p, 4.0, Stroke::new(1.0f32, col));
                    }
                }
            }
        }
        if let Some((a, b)) = self.box_select {
            if let (Some(pa), Some(pb)) = (to_screen(a), to_screen(b)) {
                let crossing = b.x < a.x;
                let col = if crossing { Color32::from_rgb(60, 160, 60) } else { Color32::from_rgb(60, 100, 220) };
                painter.rect_stroke(
                    egui::Rect::from_two_pos(pa, pb),
                    0.0,
                    Stroke::new(1.0f32, col),
                    egui::StrokeKind::Middle,
                );
                painter.rect_filled(
                    egui::Rect::from_two_pos(pa, pb),
                    0.0,
                    Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 25),
                );
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
            ..Default::default()
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
