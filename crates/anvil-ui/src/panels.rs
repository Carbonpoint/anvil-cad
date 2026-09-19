//! Part navigator, property panel, expression table.

use anvil_feature::param::ParamKind;
use anvil_feature::{Document, ParamValue};
use std::collections::HashMap;

#[derive(Default)]
pub struct PanelState {
    pub selected: Option<usize>,
    /// Set when the user double-clicks a feature in the navigator.
    pub open_requested: Option<usize>,
    /// Filter text in the font dropdown.
    pub font_search: String,
    /// Custom font file path typed in the font dropdown.
    pub font_path_draft: String,
    /// Last navigator message, for example a refused move.
    pub message: String,
    /// Text being edited, keyed by (feature index, param name).
    pub drafts: HashMap<(usize, &'static str), String>,
    pub new_expr_name: String,
    pub new_expr_value: String,
    pub expr_drafts: HashMap<String, String>,
}

/// Returns true if the document changed.
pub fn part_navigator(ui: &mut egui::Ui, doc: &mut Document, st: &mut PanelState) -> bool {
    let mut changed = false;
    ui.heading("Part Navigator");
    if !st.message.is_empty() {
        ui.colored_label(egui::Color32::from_rgb(200, 90, 40), &st.message);
    }
    ui.separator();
    let mut to_remove = None;
    let mut to_toggle = None;
    let mut to_move: Option<(usize, usize)> = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, node) in doc.features.iter().enumerate() {
            // Buttons on the right, the name truncated to what is left, so a
            // long feature name never widens the panel.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("x").on_hover_text("Delete").clicked() {
                    to_remove = Some(i);
                }
                if ui.small_button("v").on_hover_text("Move later in the history").clicked() {
                    to_move = Some((i, i + 1));
                }
                if ui.small_button("^").on_hover_text("Move earlier in the history").clicked() && i > 0 {
                    to_move = Some((i, i - 1));
                }
                let (txt, tip) = if node.suppressed { ("on", "Unsuppress") } else { ("off", "Suppress") };
                if ui.small_button(txt).on_hover_text(tip).clicked() {
                    to_toggle = Some(i);
                }
                let mark = if node.error.is_some() {
                    "!"
                } else if node.suppressed {
                    "-"
                } else {
                    "*"
                };
                let label = format!("{mark} {i}: {}", node.feature.name());
                let sel = st.selected == Some(i);
                let resp = ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add(egui::Button::selectable(sel, label.clone()).truncate())
                });
                let resp = resp.inner;
                if resp.clicked() {
                    st.selected = Some(i);
                }
                if resp.double_clicked() {
                    st.selected = Some(i);
                    st.open_requested = Some(i);
                }
                match &node.error {
                    Some(e) => resp.on_hover_text(format!("{label}\n{e}")),
                    None => resp.on_hover_text(label),
                };
            });
        }
    });
    if let Some(i) = to_toggle {
        let s = doc.features[i].suppressed;
        doc.set_suppressed(i, !s);
        changed = true;
    }
    if let Some((from, to)) = to_move {
        if to < doc.features.len() {
            match doc.move_feature(from, to) {
                Ok(()) => {
                    st.selected = Some(to);
                    changed = true;
                }
                Err(e) => st.message = e,
            }
        }
    }
    if let Some(i) = to_remove {
        doc.remove_feature(i);
        st.selected = None;
        changed = true;
    }
    changed
}

/// Generic property editor built from `Feature::params()`.
pub fn property_panel(ui: &mut egui::Ui, doc: &mut Document, st: &mut PanelState) -> bool {
    ui.heading("Properties");
    ui.separator();
    let Some(idx) = st.selected else {
        ui.label("Select a feature in the Part Navigator.");
        return false;
    };
    if idx >= doc.features.len() {
        st.selected = None;
        return false;
    }
    let params = doc.features[idx].feature.params();
    ui.label(doc.features[idx].feature.name());
    if let Some(note) = doc.features[idx].output.as_ref().and_then(|o| o.note.as_ref()) {
        let warn = note.contains("needs about");
        let color = if warn { egui::Color32::from_rgb(200, 110, 20) } else { egui::Color32::from_rgb(60, 120, 70) };
        ui.colored_label(color, note);
    }
    if let Some(e) = &doc.features[idx].error {
        ui.colored_label(egui::Color32::from_rgb(220, 80, 60), e);
    }
    let mut pending: Option<(&'static str, ParamValue)> = None;
    egui::Grid::new("props").num_columns(2).show(ui, |ui| {
        for p in &params {
            ui.label(p.label);
            match (&p.kind, &p.value) {
                (ParamKind::Length | ParamKind::Angle | ParamKind::Text, ParamValue::Expr(cur)) => {
                    let draft = st.drafts.entry((idx, p.name)).or_insert_with(|| cur.clone());
                    let resp = ui.text_edit_singleline(draft);
                    if resp.lost_focus() && draft != cur {
                        pending = Some((p.name, ParamValue::Expr(draft.clone())));
                    }
                }
                (ParamKind::Font, ParamValue::Expr(cur)) => {
                    let current = anvil_feature::fonts::display_name(cur);
                    egui::ComboBox::from_id_salt((idx, p.name, "font"))
                        .width(180.0)
                        .height(380.0)
                        .selected_text(current)
                        .show_ui(ui, |ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut st.font_search)
                                    .hint_text("Search fonts")
                                    .desired_width(220.0),
                            );
                            let cat = anvil_feature::fonts::catalogue();
                            let q = st.font_search.to_lowercase();
                            let hits: Vec<&anvil_feature::fonts::FontEntry> =
                                cat.iter().filter(|f| q.is_empty() || f.label().to_lowercase().contains(&q)).collect();
                            ui.label(
                                egui::RichText::new(format!("{} of {} fonts", hits.len(), cat.len())).small().weak(),
                            );
                            ui.separator();
                            let mut shown_system = false;
                            for f in hits.iter().take(500) {
                                if !f.builtin && !shown_system {
                                    ui.label(egui::RichText::new("Installed on this computer").small().weak());
                                    shown_system = true;
                                }
                                let selected =
                                    f.source == *cur || (cur.trim().is_empty() && f.source == "builtin:DejaVu Sans");
                                let text = if f.builtin { format!("{}  (bundled)", f.label()) } else { f.label() };
                                let r = ui.selectable_label(selected, text);
                                if r.clicked() {
                                    pending = Some((p.name, ParamValue::Expr(f.source.clone())));
                                }
                                r.on_hover_text(if f.builtin {
                                    "Bundled with Anvil: opens the same on every computer".to_string()
                                } else {
                                    f.source.clone()
                                });
                            }
                            ui.separator();
                            ui.label(egui::RichText::new("Or a font file (.ttf or .otf)").small().weak());
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut st.font_path_draft)
                                        .hint_text("C:\\path\\font.ttf")
                                        .desired_width(170.0),
                                );
                                if ui.button("Use").clicked() && !st.font_path_draft.trim().is_empty() {
                                    pending = Some((p.name, ParamValue::Expr(st.font_path_draft.trim().to_string())));
                                }
                            });
                        });
                }
                (ParamKind::Regions { .. }, ParamValue::Expr(cur)) => {
                    let counts =
                        crate::sketch_view::region_pick(doc, Some(idx)).map(|pk| (pk.used_count(), pk.regions.len()));
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(match counts {
                                Some((u, n)) => format!("{u} of {n} regions used"),
                                None => "No closed regions".to_string(),
                            });
                            if !cur.trim().is_empty() && ui.small_button("Default").clicked() {
                                pending = Some((p.name, ParamValue::Expr(String::new())));
                            }
                        });
                        ui.label(egui::RichText::new("Click regions in the view to add or remove them").small().weak());
                    });
                }
                (ParamKind::Bool, ParamValue::Bool(b)) => {
                    let mut v = *b;
                    if ui.checkbox(&mut v, "").changed() {
                        pending = Some((p.name, ParamValue::Bool(v)));
                    }
                }
                (ParamKind::FeatureRef { accepts }, ParamValue::FeatureRef(cur)) => {
                    let mut v = *cur;
                    let current_name = doc.features.get(v).map(|n| n.feature.name()).unwrap_or_else(|| "?".into());
                    egui::ComboBox::from_id_salt((idx, p.name)).selected_text(format!("{v}: {current_name}")).show_ui(
                        ui,
                        |ui| {
                            for (i, n) in doc.features.iter().enumerate() {
                                if i < idx && accepts.contains(&n.feature.kind()) {
                                    ui.selectable_value(&mut v, i, format!("{i}: {}", n.feature.name()));
                                }
                            }
                        },
                    );
                    if v != *cur {
                        pending = Some((p.name, ParamValue::FeatureRef(v)));
                    }
                }
                (ParamKind::Choice { options }, ParamValue::Choice(cur)) => {
                    let mut v = cur.clone();
                    egui::ComboBox::from_id_salt((idx, p.name, "c")).selected_text(&v).show_ui(ui, |ui| {
                        for o in options {
                            ui.selectable_value(&mut v, o.to_string(), *o);
                        }
                    });
                    if &v != cur {
                        pending = Some((p.name, ParamValue::Choice(v)));
                    }
                }
                _ => {
                    ui.label("(unsupported)");
                }
            }
            ui.end_row();
        }
    });
    if let Some((name, value)) = pending {
        let mut err = None;
        doc.edit_feature(idx, |f| {
            if let Err(e) = f.set_param(name, value) {
                err = Some(e);
            }
        });
        if let Some(e) = err {
            log::warn!("set_param: {e}");
        }
        st.drafts.retain(|(i, _), _| *i != idx);
        return true;
    }
    false
}

pub fn expression_panel(ui: &mut egui::Ui, doc: &mut Document, st: &mut PanelState) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.heading("Expressions");
        ui.label("name = expression, for example  h = w / 2 + 5");
    });
    let mut edits: Vec<(String, String)> = Vec::new();
    egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for p in doc.exprs.iter() {
                ui.group(|ui| {
                    ui.label(format!("{} = {:.4}", p.name, p.value));
                    let draft = st.expr_drafts.entry(p.name.clone()).or_insert_with(|| p.source.clone());
                    let resp = ui.add(egui::TextEdit::singleline(draft).desired_width(90.0));
                    if resp.lost_focus() && *draft != p.source {
                        edits.push((p.name.clone(), draft.clone()));
                    }
                });
            }
            ui.group(|ui| {
                ui.add(egui::TextEdit::singleline(&mut st.new_expr_name).hint_text("name").desired_width(60.0));
                ui.add(egui::TextEdit::singleline(&mut st.new_expr_value).hint_text("value").desired_width(90.0));
                if ui.button("Add").clicked() && !st.new_expr_name.trim().is_empty() {
                    edits.push((st.new_expr_name.trim().to_string(), st.new_expr_value.clone()));
                    st.new_expr_name.clear();
                    st.new_expr_value.clear();
                }
            });
        });
    });
    for (name, src) in edits {
        match doc.set_expression(&name, &src) {
            Ok(()) => changed = true,
            Err(e) => log::warn!("expression {name}: {e}"),
        }
        st.expr_drafts.remove(&name);
    }
    changed
}
