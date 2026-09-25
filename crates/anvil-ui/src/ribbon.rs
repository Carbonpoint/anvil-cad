//! Data-driven ribbon.
//!
//! Tabs and groups come from two sources:
//! 1. `anvil_feature::descriptors()`: every registered feature adds a button.
//! 2. `app_actions()` below for app-level commands (save, fit, measure, ...).
//!
//! The Sketch tab is contextual. It is drawn by the app while the sketch
//! editor is open and is not part of this list.
//!
//! The layout copies Fusion: one row of panels per tab. A panel shows a
//! few pinned icons, and its name under them is a menu of every command of
//! the panel. `PINNED` and `SHORTCUTS` decide which icons show and which
//! keys the menus name. A command that no table names is still in its
//! panel's menu, so a new feature needs no edit here.

use anvil_feature::descriptors;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RibbonAction {
    Undo,
    Redo,
    NewDocument,
    Save,
    /// Save to a path chosen in a file dialog.
    SaveAs,
    Load,
    /// Open the Export dialog: pick a format, then a file.
    Export,
    FitView,
    DemoPart,
    ExportGcode,
    EditSketch,
    Measure,
    ToggleEdges,
    ViewIso,
    /// Show or hide the CPU, memory, and fps overlay.
    TogglePerf,
    /// Time the view with the GPU and with the software renderer.
    Benchmark,
    /// Measure the face the section plane cuts.
    SectionAnalysis,
    /// Draw the viewport with OpenGL, or with the software rasterizer.
    ToggleGpu,
    /// Reverse the mouse wheel zoom direction.
    ToggleScrollDir,
    /// One view or four views (Front, Right, Top, angled).
    ToggleQuadView,
    /// Orthographic or perspective for the active view.
    ToggleProjection,
    /// Which world axis stays vertical: 0 X, 1 Y, 2 Z, 3 free.
    SetUpAxis(u8),
    /// Show or hide the UI scale and text size settings window.
    ToggleSettings,
    ViewTop,
    ViewFront,
    ViewRight,
    DeleteFeature,
    ComputeAll,
    CenterOfMass,
    BillOfMaterials,
    ToggleUnits,
    SampleCard,
    PressPull,
    Workbook(u8),
    /// Load the kettle sample, see docs/KETTLE.md.
    Kettle,
    /// The kettle with a sprue, runner, ingate, and riser.
    KettleGated,
    /// The kettle body split into pattern halves, core, and core box.
    KettleMold,
    Interference,
    /// What a click in the view picks: 0 All, 1 Body, 2 Face, 3 Edge.
    SetSelectFilter(u8),
}

pub struct RibbonButton {
    pub label: &'static str,
    pub tooltip: &'static str,
    pub order: u32,
    pub kind: ButtonKind,
    /// True when the command shows as an icon in the row. Every command
    /// is in its panel's menu either way.
    pub pinned: bool,
    /// The key that runs the command, as the menus show it.
    pub shortcut: Option<&'static str>,
}

impl RibbonButton {
    fn new(label: &'static str, tooltip: &'static str, order: u32, kind: ButtonKind) -> Self {
        RibbonButton { label, tooltip, order, kind, pinned: false, shortcut: None }
    }
}

pub enum ButtonKind {
    Feature(&'static str),
    Action(RibbonAction),
}

impl ButtonKind {
    /// The icon id to look up in `crate::icons::paint`.
    pub fn icon_id(&self) -> &'static str {
        match self {
            ButtonKind::Feature(id) => id,
            ButtonKind::Action(a) => a.icon_id(),
        }
    }
}

impl RibbonAction {
    /// The icon id to look up in `crate::icons::paint`.
    pub fn icon_id(self) -> &'static str {
        match self {
            RibbonAction::Undo => "undo",
            RibbonAction::Redo => "redo",
            RibbonAction::NewDocument => "new_document",
            RibbonAction::Save => "save",
            RibbonAction::SaveAs => "save_as",
            RibbonAction::Load => "load",
            RibbonAction::Export => "export",
            RibbonAction::FitView => "fit_view",
            RibbonAction::DemoPart => "demo_part",
            RibbonAction::ExportGcode => "export_gcode",
            RibbonAction::EditSketch => "edit_sketch",
            RibbonAction::Measure => "measure",
            RibbonAction::ToggleEdges => "toggle_edges",
            RibbonAction::TogglePerf => "toggle_perf",
            RibbonAction::Benchmark => "benchmark",
            RibbonAction::SectionAnalysis => "section_analysis",
            RibbonAction::ToggleGpu => "toggle_gpu",
            RibbonAction::ToggleScrollDir => "toggle_scroll",
            RibbonAction::ToggleQuadView => "view_quad",
            RibbonAction::ToggleProjection => "projection",
            RibbonAction::SetUpAxis(_) => "up_axis",
            RibbonAction::ToggleSettings => "settings",
            RibbonAction::ViewIso => "view_iso",
            RibbonAction::ViewTop => "view_top",
            RibbonAction::ViewFront => "view_front",
            RibbonAction::ViewRight => "view_right",
            RibbonAction::DeleteFeature => "delete_feature",
            RibbonAction::ComputeAll => "compute_all",
            RibbonAction::CenterOfMass => "center_of_mass",
            RibbonAction::BillOfMaterials => "bill_of_materials",
            RibbonAction::ToggleUnits => "toggle_units",
            RibbonAction::SampleCard => "sample_card",
            RibbonAction::PressPull => "press_pull",
            RibbonAction::Workbook(_) => "workbook",
            RibbonAction::Kettle => "kettle",
            RibbonAction::KettleGated => "kettle_gated",
            RibbonAction::KettleMold => "kettle_mold",
            RibbonAction::Interference => "interference",
            RibbonAction::SetSelectFilter(0) => "select_all",
            RibbonAction::SetSelectFilter(1) => "select_body",
            RibbonAction::SetSelectFilter(2) => "select_face",
            RibbonAction::SetSelectFilter(_) => "select_edge",
        }
    }
}

pub struct RibbonGroup {
    pub name: &'static str,
    pub buttons: Vec<RibbonButton>,
}

pub struct RibbonTab {
    pub name: &'static str,
    pub groups: Vec<RibbonGroup>,
}

const TAB_ORDER: &[&str] = &["File", "Solid", "Field", "Inspect", "View", "Examples"];
const GROUP_ORDER: &[&str] = &[
    "Document",
    "Export",
    "Parts",
    "Workbook",
    "Casting samples",
    "Create",
    "Modify",
    "Construct",
    "Inspect",
    "Insert",
    "Casting",
    "Manage",
    "Camera",
    "Display",
    "Settings",
    "Select",
];

/// Commands pinned as icons, panel by panel, in the order shown. An id is
/// a feature id, or the icon id of an app action. At most four a panel.
const PINNED: &[(&str, &str, &[&str])] = &[
    ("File", "Document", &["new_document", "load", "save", "save_as"]),
    ("File", "Export", &["export", "export_gcode"]),
    ("Solid", "Create", &["sketch", "extrude", "revolve", "hole"]),
    ("Solid", "Modify", &["press_pull", "fillet", "chamfer", "combine"]),
    ("Solid", "Construct", &["offset_plane", "midplane"]),
    ("Solid", "Insert", &["mesh"]),
    ("Solid", "Casting", &["sprue", "runner", "riser", "casting_check"]),
    ("Field", "Fill", &["lattice_fill"]),
    ("Field", "Generate", &["aircraft", "density_body"]),
    ("Inspect", "Inspect", &["measure", "section_analysis", "interference", "center_of_mass"]),
    ("Inspect", "Manage", &["bill_of_materials", "compute_all"]),
    ("View", "Camera", &["fit_view", "view_iso", "view_quad", "projection"]),
    ("View", "Display", &["toggle_edges"]),
    ("View", "Settings", &["settings"]),
    ("Examples", "Parts", &["demo_part", "sample_card"]),
    ("Examples", "Casting samples", &["kettle", "kettle_gated", "kettle_mold"]),
];

/// Keyboard shortcuts, as the menus show them. The keys work in the model
/// view while no text field has the keyboard.
const SHORTCUTS: &[(&str, &str)] =
    &[("extrude", "E"), ("hole", "H"), ("press_pull", "Q"), ("fillet", "F"), ("delete_feature", "Del")];

/// The key that runs a command, by feature id or action icon id.
pub fn shortcut_of(id: &str) -> Option<&'static str> {
    SHORTCUTS.iter().find(|(i, _)| *i == id).map(|(_, k)| *k)
}

fn app_actions() -> Vec<(&'static str, &'static str, RibbonButton)> {
    use ButtonKind::Action as A;
    use RibbonAction::*;
    vec![
        ("File", "Document", RibbonButton::new("New", "Start an empty document", 0, A(NewDocument))),
        (
            "File",
            "Document",
            RibbonButton::new("Open", "Open an Anvil document, or import a STEP, 3MF or STL file", 1, A(Load)),
        ),
        ("File", "Document", RibbonButton::new("Save", "Save to the .anvil path in the status bar", 2, A(Save))),
        ("File", "Document", RibbonButton::new("Save As", "Choose where to save the .anvil document", 3, A(SaveAs))),
        (
            "File",
            "Export",
            RibbonButton::new(
                "Export",
                "Choose a format (STL, 3MF, OBJ, PLY, OFF, AMF, glTF, STEP) and where to save",
                0,
                A(Export),
            ),
        ),
        ("Examples", "Parts", RibbonButton::new("Demo part", "Load a sample part", 0, A(DemoPart))),
        (
            "Solid",
            "Create",
            RibbonButton::new(
                "Edit Sketch",
                "Open the selected sketch in the editor (or double-click it)",
                1,
                A(EditSketch),
            ),
        ),
        (
            "Inspect",
            "Inspect",
            RibbonButton::new("Measure", "Volume and bounding box of the selected feature's bodies", 0, A(Measure)),
        ),
        (
            "Inspect",
            "Inspect",
            RibbonButton::new(
                "Center of Mass",
                "Centre of mass of the selected feature's bodies, shown in the viewport",
                1,
                A(CenterOfMass),
            ),
        ),
        (
            "Solid",
            "Modify",
            RibbonButton::new("Delete", "Delete the selected feature (Del in model mode)", 90, A(DeleteFeature)),
        ),
        ("Inspect", "Manage", RibbonButton::new("Compute All", "Regenerate the whole history", 0, A(ComputeAll))),
        (
            "Inspect",
            "Manage",
            RibbonButton::new(
                "Bill of Materials",
                "List bodies with volume and mass in the status bar and log",
                1,
                A(BillOfMaterials),
            ),
        ),
        ("Inspect", "Manage", RibbonButton::new("Units mm/in", "Toggle the display unit", 2, A(ToggleUnits))),
        (
            "Examples",
            "Parts",
            RibbonButton::new(
                "Business card",
                "Credit-card blank with two fillet radii, embossed name, and QR code",
                1,
                A(SampleCard),
            ),
        ),
        (
            "Solid",
            "Modify",
            RibbonButton::new("Press Pull", "Extrude the selected face into a new body (Q)", 0, A(PressPull)),
        ),
        (
            "Inspect",
            "Inspect",
            RibbonButton::new(
                "Section analysis",
                "Area, perimeter, and centre of the face the section plane cuts (turn Section on first)",
                3,
                A(SectionAnalysis),
            ),
        ),
        (
            "Inspect",
            "Inspect",
            RibbonButton::new(
                "Interference",
                "Overlap volume between two selected features (Ctrl+click the second)",
                2,
                A(Interference),
            ),
        ),
        (
            "File",
            "Export",
            RibbonButton::new("Contour G-code", "Contour the first sketch profile and write G-code", 0, A(ExportGcode)),
        ),
        ("View", "Camera", RibbonButton::new("Fit", "Fit all bodies in the viewport", 0, A(FitView))),
        ("View", "Camera", RibbonButton::new("Iso", "Isometric view", 1, A(ViewIso))),
        (
            "View",
            "Camera",
            RibbonButton::new(
                "Ortho/Persp",
                "Orthographic (like a drawing) or perspective for the view under the pointer",
                6,
                A(ToggleProjection),
            ),
        ),
        (
            "View",
            "Camera",
            RibbonButton::new(
                "Four views",
                "Front, Right, Top, and an angled view in four panes",
                5,
                A(ToggleQuadView),
            ),
        ),
        ("View", "Camera", RibbonButton::new("Top", "Look down the Z axis", 2, A(ViewTop))),
        ("View", "Camera", RibbonButton::new("Front", "Look along the Y axis", 3, A(ViewFront))),
        ("View", "Camera", RibbonButton::new("Right", "Look along the X axis", 4, A(ViewRight))),
        ("View", "Display", RibbonButton::new("Edges", "Toggle model edges", 0, A(ToggleEdges))),
        (
            "View",
            "Settings",
            RibbonButton::new(
                "Performance",
                "Show or hide CPU, memory, and fps in the corner of the view",
                0,
                A(TogglePerf),
            ),
        ),
        (
            "View",
            "Settings",
            RibbonButton::new(
                "Benchmark",
                "Spin the view and report the frame time with the GPU and with the software renderer",
                3,
                A(Benchmark),
            ),
        ),
        (
            "View",
            "Settings",
            RibbonButton::new(
                "GPU viewport",
                "Draw the model with OpenGL instead of the software rasterizer",
                1,
                A(ToggleGpu),
            ),
        ),
        (
            "View",
            "Settings",
            RibbonButton::new("Invert scroll", "Reverse the mouse wheel zoom direction", 2, A(ToggleScrollDir)),
        ),
        ("View", "Settings", RibbonButton::new("Settings", "Set the UI scale and text size", 2, A(ToggleSettings))),
    ]
}

/// Which axis stays vertical on screen. The buttons sit in View > Camera.
const UP_AXES: [(&str, &str); 4] = [
    ("X up", "Keep the X axis vertical on screen"),
    ("Y up", "Keep the Y axis vertical on screen"),
    ("Z up", "Keep the Z axis vertical on screen (the default)"),
    ("Free orbit", "Hold no axis vertical: the view tumbles freely"),
];

/// The selection filter, on the Solid tab's Select panel.
const SELECT_FILTERS: [(&str, &str); 4] = [
    ("All", "A click picks an edge, a face, or a body"),
    ("Body", "A click picks a whole body"),
    ("Face", "A click picks a face"),
    ("Edge", "A click picks an edge, with a wider catch radius"),
];

pub fn build_ribbon() -> Vec<RibbonTab> {
    let mut tabs: Vec<RibbonTab> = Vec::new();
    let mut place = |tab: &'static str, group: &'static str, b: RibbonButton| {
        let t = match tabs.iter_mut().position(|t| t.name == tab) {
            Some(i) => &mut tabs[i],
            None => {
                tabs.push(RibbonTab { name: tab, groups: Vec::new() });
                tabs.last_mut().unwrap()
            }
        };
        let g = match t.groups.iter_mut().position(|g| g.name == group) {
            Some(i) => &mut t.groups[i],
            None => {
                t.groups.push(RibbonGroup { name: group, buttons: Vec::new() });
                t.groups.last_mut().unwrap()
            }
        };
        g.buttons.push(b);
    };
    for (tab, group, b) in app_actions() {
        place(tab, group, b);
    }
    const WB: [&str; 6] = ["WB 1 Plate", "WB 2 Bracket", "WB 3 Shaft", "WB 4 Nut", "WB 5 Elbow", "WB 6 Adapter"];
    for (i, label) in WB.iter().enumerate() {
        place(
            "Examples",
            "Workbook",
            RibbonButton::new(
                label,
                "Workbook exercise, see docs/WORKBOOK.md",
                10 + i as u32,
                ButtonKind::Action(RibbonAction::Workbook(i as u8)),
            ),
        );
    }
    place(
        "Examples",
        "Casting samples",
        RibbonButton::new(
            "Kettle",
            "Cast iron kettle with hobnail dots, see docs/KETTLE.md",
            20,
            ButtonKind::Action(RibbonAction::Kettle),
        ),
    );
    place(
        "Examples",
        "Casting samples",
        RibbonButton::new(
            "Kettle + gating",
            "The kettle with a sprue, runner, ingate, and riser",
            21,
            ButtonKind::Action(RibbonAction::KettleGated),
        ),
    );
    place(
        "Examples",
        "Casting samples",
        RibbonButton::new(
            "Kettle mold",
            "Pattern halves, core, and core box for the kettle body",
            22,
            ButtonKind::Action(RibbonAction::KettleMold),
        ),
    );
    for (i, (label, tooltip)) in UP_AXES.iter().enumerate() {
        place(
            "View",
            "Camera",
            RibbonButton::new(label, tooltip, 10 + i as u32, ButtonKind::Action(RibbonAction::SetUpAxis(i as u8))),
        );
    }
    for d in descriptors() {
        place(d.tab, d.group, RibbonButton::new(d.label, d.tooltip, d.order, ButtonKind::Feature(d.id)));
    }
    for (i, (label, tooltip)) in SELECT_FILTERS.iter().enumerate() {
        place(
            "Solid",
            "Select",
            RibbonButton::new(label, tooltip, i as u32, ButtonKind::Action(RibbonAction::SetSelectFilter(i as u8))),
        );
    }
    for t in &mut tabs {
        for g in &mut t.groups {
            g.buttons.sort_by_key(|b| (b.order, b.label));
            let pins = PINNED.iter().find(|(tab, panel, _)| *tab == t.name && *panel == g.name).map(|(_, _, ids)| *ids);
            for b in &mut g.buttons {
                let id = b.kind.icon_id();
                b.pinned = pins.is_some_and(|p| p.contains(&id));
                b.shortcut = shortcut_of(id);
            }
            // Pinned icons show in the order the table gives.
            if let Some(p) = pins {
                g.buttons.sort_by_key(|b| p.iter().position(|id| *id == b.kind.icon_id()).unwrap_or(usize::MAX));
            }
        }
        t.groups.sort_by_key(|g| GROUP_ORDER.iter().position(|n| *n == g.name).unwrap_or(usize::MAX));
    }
    tabs.sort_by_key(|t| TAB_ORDER.iter().position(|n| *n == t.name).unwrap_or(usize::MAX));
    tabs
}
