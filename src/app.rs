use std::fs;
use std::time::{Duration, Instant};

use egui::Context;

use crate::cache::{self, CacheWriter};
use crate::shortcuts;
use crate::tab::Tab;
use crate::ui::{editor, tab_bar};

const FLUSH_INTERVAL: Duration = Duration::from_secs(2);

pub struct EditorApp {
    tabs: Vec<Tab>,
    active_tab: usize,
    last_flush: Instant,
    writer: Option<CacheWriter>,
    next_id: usize,
}

impl EditorApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Tema claro
        _cc.egui_ctx.set_visuals(egui::Visuals::light());

        let writer = CacheWriter::spawn();
        let mut app = Self {
            tabs: Vec::new(),
            active_tab: 0,
            last_flush: Instant::now(),
            writer: Some(writer),
            next_id: 0,
        };
        app.restore_or_init();
        app
    }

    fn restore_or_init(&mut self) {
        match cache::scan_cache() {
            Ok(entries) if !entries.is_empty() => {
                for (path, content) in entries {
                    let tab = Tab::new(self.next_id, path, content);
                    self.next_id += 1;
                    self.tabs.push(tab);
                }
            }
            _ => {
                self.create_new_tab();
            }
        }
    }

    pub fn create_new_tab(&mut self) {
        match cache::create_tab_file() {
            Ok(path) => {
                let tab = Tab::new(self.next_id, path, String::new());
                self.next_id += 1;
                self.active_tab = self.tabs.len();
                self.tabs.push(tab);
            }
            Err(e) => log::error!("Failed to create tab file: {}", e),
        }
    }

    pub fn close_tab(&mut self, idx: usize) {
        if self.tabs.is_empty() {
            return;
        }

        let tab = self.tabs.remove(idx);
        if let Some(writer) = &self.writer {
            writer.delete(tab.file_path);
        }

        if self.tabs.is_empty() {
            self.active_tab = 0;
            self.create_new_tab();
            return;
        }

        // Adjust active_tab: if we removed a tab before it, shift back by one.
        if idx < self.active_tab {
            self.active_tab -= 1;
        }
        // Clamp to valid range.
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
    }

    fn flush_dirty_tabs(&mut self) {
        let dirty: Vec<(std::path::PathBuf, String)> = self
            .tabs
            .iter_mut()
            .filter(|t| t.dirty)
            .map(|t| {
                t.dirty = false;
                (t.file_path.clone(), t.content.clone())
            })
            .collect();

        if let Some(writer) = &self.writer {
            for (path, content) in dirty {
                writer.write(path, content);
            }
        }
    }

    fn handle_shortcuts(&mut self, action: &shortcuts::ShortcutAction) {
        if action.new_tab {
            self.create_new_tab();
        }
        if action.close_tab && !self.tabs.is_empty() {
            let idx = self.active_tab;
            self.close_tab(idx);
        }
        if action.next_tab && !self.tabs.is_empty() {
            self.active_tab = (self.active_tab + 1) % self.tabs.len();
        }
        if action.prev_tab && !self.tabs.is_empty() {
            self.active_tab = (self.active_tab + self.tabs.len() - 1) % self.tabs.len();
        }
    }
}

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Ensure we wake up every 2s even with no user input (for auto-save).
        ctx.request_repaint_after(FLUSH_INTERVAL);

        // Auto-save flush.
        if self.last_flush.elapsed() >= FLUSH_INTERVAL {
            self.flush_dirty_tabs();
            self.last_flush = Instant::now();
        }

        // Keyboard shortcuts (before UI so tabs are up to date when rendering).
        let shortcuts = shortcuts::check_shortcuts(ctx);
        self.handle_shortcuts(&shortcuts);

        // ── Tab bar ──────────────────────────────────────────────────────────
        egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            let result = tab_bar::render_tab_bar(ui, &self.tabs, self.active_tab);
            if let Some(idx) = result.clicked_tab {
                self.active_tab = idx;
            }
            if let Some(idx) = result.closed_tab {
                self.close_tab(idx);
            }
            if result.new_tab {
                self.create_new_tab();
            }
        });

        // ── Status bar ───────────────────────────────────────────────────────
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(tab) = self.tabs.get(self.active_tab) {
                    let chars = tab.content.chars().count();
                    let lines = tab.content.lines().count().max(1);
                    ui.label(format!("{} chars  │  {} linhas", chars, lines));
                    ui.separator();
                    if tab.dirty {
                        ui.label("● salvando…");
                    } else {
                        ui.label("✓ salvo");
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(tab.file_path.to_string_lossy().as_ref())
                                .small()
                                .color(egui::Color32::from_gray(120)),
                        );
                    });
                }
            });
        });

        // ── Editor area ──────────────────────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                let changed = editor::render_editor(ui, tab);
                if changed {
                    tab.dirty = true;
                }
            }
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Synchronously flush all dirty tabs before shutdown so no edits are lost.
        for tab in &self.tabs {
            if tab.dirty {
                let tmp = tab.file_path.with_extension("tmp");
                if fs::write(&tmp, tab.content.as_bytes()).is_ok() {
                    #[cfg(unix)]
                    let _ = fs::rename(&tmp, &tab.file_path);
                    #[cfg(not(unix))]
                    {
                        let _ = fs::remove_file(&tab.file_path);
                        let _ = fs::rename(&tmp, &tab.file_path);
                    }
                }
            }
        }

        // Shutdown writer thread: drains queued commands and exits cleanly.
        if let Some(writer) = self.writer.take() {
            writer.shutdown();
        }
    }
}
