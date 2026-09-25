//! The ribbon must read like Fusion's: one row of panels, a few pinned
//! icons per panel, and every command reachable from that panel's menu.
//!
//! This file is the contract for task 1. It is checksummed by
//! `nova/run_task.sh`; a change to it fails the job.

use anvil_ui::ribbon::{build_ribbon, ButtonKind, RibbonTab};

/// The tabs, in order. Fewer, wider tabs, the way Fusion splits them.
const WANT_TABS: &[&str] = &["File", "Solid", "Field", "Inspect", "View", "Examples"];

fn tabs() -> Vec<RibbonTab> {
    build_ribbon()
}

fn tab<'a>(tabs: &'a [RibbonTab], name: &str) -> &'a RibbonTab {
    tabs.iter().find(|t| t.name == name).unwrap_or_else(|| {
        panic!("no tab named {name}; tabs are {:?}", tabs.iter().map(|t| t.name).collect::<Vec<_>>())
    })
}

#[test]
fn the_tabs_are_the_fusion_like_set() {
    let t = tabs();
    let names: Vec<&str> = t.iter().map(|x| x.name).collect();
    assert_eq!(names, WANT_TABS, "tab names or order changed");
}

#[test]
fn every_panel_pins_at_most_four_icons() {
    for t in tabs() {
        for g in &t.groups {
            let pinned = g.buttons.iter().filter(|b| b.pinned).count();
            assert!(pinned <= 4, "{}/{} pins {pinned} icons, the limit is four", t.name, g.name);
            assert!(!g.buttons.is_empty(), "{}/{} has no commands", t.name, g.name);
        }
    }
}

#[test]
fn a_pinned_command_is_also_in_the_panel_menu() {
    // The menu lists every command of the panel, pinned or not. This is
    // one list with a flag, so a pinned command cannot go missing.
    for t in tabs() {
        for g in &t.groups {
            let pinned: Vec<&str> = g.buttons.iter().filter(|b| b.pinned).map(|b| b.label).collect();
            for p in pinned {
                let count = g.buttons.iter().filter(|b| b.label == p).count();
                assert_eq!(count, 1, "{}/{}: {p} appears {count} times", t.name, g.name);
            }
        }
    }
}

#[test]
fn every_feature_is_on_the_ribbon_exactly_once() {
    let t = tabs();
    for d in anvil_feature::descriptors() {
        let mut seen = Vec::new();
        for tb in &t {
            for g in &tb.groups {
                for b in &g.buttons {
                    if matches!(b.kind, ButtonKind::Feature(id) if id == d.id) {
                        seen.push(format!("{}/{}", tb.name, g.name));
                    }
                }
            }
        }
        assert_eq!(seen.len(), 1, "feature {} is on the ribbon {} times: {seen:?}", d.id, seen.len());
    }
}

#[test]
fn the_implicit_commands_moved_to_the_field_tab() {
    let t = tabs();
    let field = tab(&t, "Field");
    let ids: Vec<&str> = field
        .groups
        .iter()
        .flat_map(|g| g.buttons.iter())
        .filter_map(|b| match b.kind {
            ButtonKind::Feature(id) => Some(id),
            _ => None,
        })
        .collect();
    for want in ["lattice", "aircraft", "density"] {
        assert!(ids.iter().any(|id| id.contains(want)), "the Field tab has no {want} command; it has {ids:?}");
    }
}

#[test]
fn the_inspect_tab_holds_the_measuring_commands() {
    let t = tabs();
    let inspect = tab(&t, "Inspect");
    let labels: Vec<&str> = inspect.groups.iter().flat_map(|g| g.buttons.iter()).map(|b| b.label).collect();
    for want in ["Measure", "Interference", "Section analysis", "Center of Mass", "Bill of Materials"] {
        assert!(labels.contains(&want), "the Inspect tab has no {want}; it has {labels:?}");
    }
}

#[test]
fn the_solid_tab_has_a_select_panel() {
    let t = tabs();
    let solid = tab(&t, "Solid");
    let select =
        solid.groups.iter().find(|g| g.name == "Select").unwrap_or_else(|| panic!("the Solid tab has no Select panel"));
    let labels: Vec<&str> = select.buttons.iter().map(|b| b.label).collect();
    for want in ["All", "Body", "Face", "Edge"] {
        assert!(labels.contains(&want), "the Select panel has no {want}; it has {labels:?}");
    }
}

#[test]
fn shortcuts_are_shown_and_are_unique_within_a_tab() {
    for t in tabs() {
        let mut used: Vec<(&str, &str)> = Vec::new();
        for g in &t.groups {
            for b in &g.buttons {
                if let Some(k) = b.shortcut {
                    assert!(
                        !used.iter().any(|(_, s)| *s == k),
                        "{}: shortcut {k} is on {} and on {:?}",
                        t.name,
                        b.label,
                        used.iter().find(|(_, s)| *s == k)
                    );
                    used.push((b.label, k));
                }
            }
        }
    }
    // The letters the application already obeys must be shown.
    assert_eq!(anvil_ui::ribbon::shortcut_of("extrude"), Some("E"));
    assert_eq!(anvil_ui::ribbon::shortcut_of("hole"), Some("H"));
}

#[test]
fn no_panel_is_empty_and_no_tab_is_empty() {
    for t in tabs() {
        assert!(!t.groups.is_empty(), "tab {} has no panels", t.name);
    }
}
