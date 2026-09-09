//! Data-driven ribbon.
//!
//! Tabs and groups come from two sources:
//! 1. `anvil_feature::descriptors()`: every registered feature adds a button.
//! 2. `RibbonAction` entries below for app-level commands (undo, save, ...).
//!
//! Nothing here needs editing to add a feature. Add app commands to
//! `app_actions()`.

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

const TAB_ORDER: &[&str] = &["File", "Home", "CAM", "View"];

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
                tooltip: "Open an .anvil file (path in status bar)",
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
            RibbonButton {
                label: "Demo part",
                tooltip: "Load a sample sketch + extrude + revolve",
                order: 0,
                kind: A(DemoPart),
            },
        ),
        ("Home", "Edit", RibbonButton { label: "Undo", tooltip: "Ctrl+Z", order: 0, kind: A(Undo) }),
        ("Home", "Edit", RibbonButton { label: "Redo", tooltip: "Ctrl+Y", order: 1, kind: A(Redo) }),
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
    }
    tabs.sort_by_key(|t| TAB_ORDER.iter().position(|n| *n == t.name).unwrap_or(usize::MAX));
    tabs
}
