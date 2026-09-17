//! Data-driven ribbon.
//!
//! Tabs and groups come from two sources:
//! 1. `anvil_feature::descriptors()`: every registered feature adds a button.
//! 2. `app_actions()` below for app-level commands (save, fit, measure, ...).
//!
//! The Sketch tab is contextual. It is drawn by the app while the sketch
//! editor is open and is not part of this list.

use anvil_feature::descriptors;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RibbonAction {
    Undo,
    Redo,
    NewDocument,
    Save,
    Load,
    ExportStl,
    FitView,
    DemoPart,
    ExportGcode,
    EditSketch,
    Measure,
    ToggleEdges,
    ViewIso,
    ViewTop,
    ViewFront,
    ViewRight,
    DeleteFeature,
    ComputeAll,
    CenterOfMass,
    BillOfMaterials,
    ToggleUnits,
    Export3mf,
    ExportStlParts,
    SampleCard,
    PressPull,
    Workbook(u8),
    /// Load the kettle sample, see docs/KETTLE.md.
    Kettle,
    /// The kettle with a sprue, runner, ingate, and riser.
    KettleGated,
    Interference,
}

pub struct RibbonButton {
    pub label: &'static str,
    pub tooltip: &'static str,
    pub order: u32,
    pub kind: ButtonKind,
}

pub enum ButtonKind {
    Feature(&'static str),
    Action(RibbonAction),
}

pub struct RibbonGroup {
    pub name: &'static str,
    pub buttons: Vec<RibbonButton>,
}

pub struct RibbonTab {
    pub name: &'static str,
    pub groups: Vec<RibbonGroup>,
}

const TAB_ORDER: &[&str] = &["File", "Solid", "CAM", "View"];
const GROUP_ORDER: &[&str] = &[
    "Document",
    "Export",
    "Samples",
    "Create",
    "Modify",
    "Construct",
    "Inspect",
    "Insert",
    "Manage",
    "Output",
    "Camera",
    "Display",
];

fn app_actions() -> Vec<(&'static str, &'static str, RibbonButton)> {
    use ButtonKind::Action as A;
    use RibbonAction::*;
    vec![
        (
            "File",
            "Document",
            RibbonButton { label: "New", tooltip: "Start an empty document", order: 0, kind: A(NewDocument) },
        ),
        (
            "File",
            "Document",
            RibbonButton {
                label: "Open",
                tooltip: "Open the .anvil file named in the status bar",
                order: 1,
                kind: A(Load),
            },
        ),
        (
            "File",
            "Document",
            RibbonButton {
                label: "Save",
                tooltip: "Save to the .anvil path in the status bar",
                order: 2,
                kind: A(Save),
            },
        ),
        (
            "File",
            "Export",
            RibbonButton { label: "STL", tooltip: "Export all bodies as binary STL", order: 0, kind: A(ExportStl) },
        ),
        (
            "File",
            "Samples",
            RibbonButton { label: "Demo part", tooltip: "Load a sample part", order: 0, kind: A(DemoPart) },
        ),
        (
            "Solid",
            "Create",
            RibbonButton {
                label: "Edit Sketch",
                tooltip: "Open the selected sketch in the editor (or double-click it)",
                order: 1,
                kind: A(EditSketch),
            },
        ),
        (
            "Solid",
            "Inspect",
            RibbonButton {
                label: "Measure",
                tooltip: "Volume and bounding box of the selected feature's bodies",
                order: 0,
                kind: A(Measure),
            },
        ),
        (
            "Solid",
            "Inspect",
            RibbonButton {
                label: "Center of Mass",
                tooltip: "Centre of mass of the selected feature's bodies, shown in the viewport",
                order: 1,
                kind: A(CenterOfMass),
            },
        ),
        (
            "Solid",
            "Modify",
            RibbonButton {
                label: "Delete",
                tooltip: "Delete the selected feature (Del in model mode)",
                order: 90,
                kind: A(DeleteFeature),
            },
        ),
        (
            "Solid",
            "Manage",
            RibbonButton {
                label: "Compute All",
                tooltip: "Regenerate the whole history",
                order: 0,
                kind: A(ComputeAll),
            },
        ),
        (
            "Solid",
            "Manage",
            RibbonButton {
                label: "Bill of Materials",
                tooltip: "List bodies with volume and mass in the status bar and log",
                order: 1,
                kind: A(BillOfMaterials),
            },
        ),
        (
            "Solid",
            "Manage",
            RibbonButton { label: "Units mm/in", tooltip: "Toggle the display unit", order: 2, kind: A(ToggleUnits) },
        ),
        (
            "File",
            "Export",
            RibbonButton {
                label: "3MF (parts)",
                tooltip: "Export a 3MF with one object per feature, for Bambu Studio or PrusaSlicer",
                order: 1,
                kind: A(Export3mf),
            },
        ),
        (
            "File",
            "Export",
            RibbonButton {
                label: "STL per part",
                tooltip: "Export one STL file per feature",
                order: 2,
                kind: A(ExportStlParts),
            },
        ),
        (
            "File",
            "Samples",
            RibbonButton {
                label: "Business card",
                tooltip: "Credit-card blank with two fillet radii, embossed name, and QR code",
                order: 1,
                kind: A(SampleCard),
            },
        ),
        (
            "Solid",
            "Modify",
            RibbonButton {
                label: "Press Pull",
                tooltip: "Extrude the selected face into a new body (Q)",
                order: 0,
                kind: A(PressPull),
            },
        ),
        (
            "Solid",
            "Inspect",
            RibbonButton {
                label: "Interference",
                tooltip: "Overlap volume between two selected features (Ctrl+click the second)",
                order: 2,
                kind: A(Interference),
            },
        ),
        (
            "CAM",
            "Output",
            RibbonButton {
                label: "Contour G-code",
                tooltip: "Contour the first sketch profile and write G-code",
                order: 0,
                kind: A(ExportGcode),
            },
        ),
        (
            "View",
            "Camera",
            RibbonButton { label: "Fit", tooltip: "Fit all bodies in the viewport", order: 0, kind: A(FitView) },
        ),
        ("View", "Camera", RibbonButton { label: "Iso", tooltip: "Isometric view", order: 1, kind: A(ViewIso) }),
        ("View", "Camera", RibbonButton { label: "Top", tooltip: "Look down the Z axis", order: 2, kind: A(ViewTop) }),
        (
            "View",
            "Camera",
            RibbonButton { label: "Front", tooltip: "Look along the Y axis", order: 3, kind: A(ViewFront) },
        ),
        (
            "View",
            "Camera",
            RibbonButton { label: "Right", tooltip: "Look along the X axis", order: 4, kind: A(ViewRight) },
        ),
        (
            "View",
            "Display",
            RibbonButton { label: "Edges", tooltip: "Toggle model edges", order: 0, kind: A(ToggleEdges) },
        ),
    ]
}

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
            "File",
            "Samples",
            RibbonButton {
                label,
                tooltip: "Workbook exercise, see docs/WORKBOOK.md",
                order: 10 + i as u32,
                kind: ButtonKind::Action(RibbonAction::Workbook(i as u8)),
            },
        );
    }
    place(
        "File",
        "Samples",
        RibbonButton {
            label: "Kettle",
            tooltip: "Cast iron kettle with hobnail dots, see docs/KETTLE.md",
            order: 20,
            kind: ButtonKind::Action(RibbonAction::Kettle),
        },
    );
    place(
        "File",
        "Samples",
        RibbonButton {
            label: "Kettle + gating",
            tooltip: "The kettle with a sprue, runner, ingate, and riser",
            order: 21,
            kind: ButtonKind::Action(RibbonAction::KettleGated),
        },
    );
    for d in descriptors() {
        place(
            d.tab,
            d.group,
            RibbonButton { label: d.label, tooltip: d.tooltip, order: d.order, kind: ButtonKind::Feature(d.id) },
        );
    }
    for t in &mut tabs {
        for g in &mut t.groups {
            g.buttons.sort_by_key(|b| (b.order, b.label));
        }
        t.groups.sort_by_key(|g| GROUP_ORDER.iter().position(|n| *n == g.name).unwrap_or(usize::MAX));
    }
    tabs.sort_by_key(|t| TAB_ORDER.iter().position(|n| *n == t.name).unwrap_or(usize::MAX));
    tabs
}
