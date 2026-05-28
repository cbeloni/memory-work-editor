use egui::{RichText, ScrollArea, Ui};

use crate::tab::Tab;

pub struct TabBarAction {
    pub clicked_tab: Option<usize>,
    pub closed_tab: Option<usize>,
    pub new_tab: bool,
}

pub fn render_tab_bar(ui: &mut Ui, tabs: &[Tab], active_tab: usize) -> TabBarAction {
    let mut action = TabBarAction {
        clicked_tab: None,
        closed_tab: None,
        new_tab: false,
    };

    let sel_bg = ui.visuals().selection.bg_fill;
    let inactive_bg = ui.visuals().widgets.inactive.bg_fill;
    let inactive_weak = ui.visuals().widgets.inactive.weak_bg_fill;

    ScrollArea::horizontal().id_source("tab_scroll").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;

            for (i, tab) in tabs.iter().enumerate() {
                let is_active = i == active_tab;
                let label = tab.display_title();
                let tab_bg = if is_active { sel_bg } else { inactive_bg };
                let close_bg = if is_active { sel_bg } else { inactive_weak };

                // Tab label button
                let tab_btn = ui.add(
                    egui::Button::new(RichText::new(&label).size(13.0))
                        .fill(tab_bg)
                        .min_size(egui::vec2(70.0, 26.0)),
                );
                if tab_btn.clicked() {
                    action.clicked_tab = Some(i);
                }
                tab_btn.on_hover_text(tab.file_path.to_string_lossy().as_ref());

                // Close button — directly after the tab button, same height, no inner layout
                let close_btn = ui.add(
                    egui::Button::new(RichText::new("x").size(11.0))
                        .fill(close_bg)
                        .min_size(egui::vec2(20.0, 26.0)),
                );
                if close_btn.clicked() {
                    action.closed_tab = Some(i);
                }
                close_btn.on_hover_text("Fechar aba (Ctrl+W)");

                ui.add_space(4.0);
            }

            // New tab button
            let new_btn = ui.add(
                egui::Button::new(RichText::new("+").size(16.0))
                    .fill(inactive_bg)
                    .min_size(egui::vec2(28.0, 26.0)),
            );
            if new_btn.clicked() {
                action.new_tab = true;
            }
            new_btn.on_hover_text("Nova aba (Ctrl+T)");
        });
    });

    action
}
