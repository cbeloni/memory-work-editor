use std::{cell::RefCell, fs, path::PathBuf, rc::Rc, time::Duration};

use gtk4::{gdk, glib, prelude::*};

use crate::cache::{self, CacheWriter};

// ── State ────────────────────────────────────────────────────────────────────

struct TabEntry {
    id: usize,
    file_path: PathBuf,
    /// Hash portion of the filename, used as default title when content is empty.
    hash: String,
    text_view: gtk4::TextView,
    tab_label: gtk4::Label,
    /// Horizontal box that wraps the title label + close button (the Notebook tab widget).
    tab_box: gtk4::Box,
    /// The ScrolledWindow that is the actual Notebook page child.
    page_widget: gtk4::ScrolledWindow,
    dirty: bool,
}

struct AppState {
    tabs: Vec<TabEntry>,
    writer: Option<CacheWriter>,
    next_id: usize,
    window: Option<gtk4::ApplicationWindow>,
    notebook: Option<gtk4::Notebook>,
    search_bar: Option<gtk4::SearchBar>,
    search_entry: Option<gtk4::SearchEntry>,
}

impl AppState {
    fn new() -> Self {
        Self {
            tabs: vec![],
            writer: Some(CacheWriter::spawn()),
            next_id: 0,
            window: None,
            notebook: None,
            search_bar: None,
            search_entry: None,
        }
    }

    fn flush_dirty(&mut self) {
        let writes: Vec<(PathBuf, String)> = self
            .tabs
            .iter_mut()
            .filter(|t| t.dirty)
            .map(|t| {
                t.dirty = false;
                let content = buffer_text(&t.text_view);
                let title = display_title(&content, &t.hash, false);
                t.tab_label.set_text(&title);
                (t.file_path.clone(), content)
            })
            .collect();

        if let Some(w) = &self.writer {
            for (path, content) in writes {
                w.write(path, content);
            }
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn buffer_text(tv: &gtk4::TextView) -> String {
    let buf = tv.buffer();
    buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string()
}

fn extract_hash(path: &PathBuf) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.split('-').nth(1))
        .unwrap_or("tab")
        .to_string()
}

fn display_title(content: &str, hash: &str, dirty: bool) -> String {
    let base = content
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| {
            let t = l.trim();
            let chars: Vec<char> = t.chars().take(22).collect();
            if t.chars().count() > 22 {
                format!("{}…", chars.iter().collect::<String>())
            } else {
                t.to_string()
            }
        })
        .unwrap_or_else(|| hash.to_string());

    if dirty { format!("{}*", base) } else { base }
}

fn status_text(content: &str) -> String {
    let chars = content.chars().count();
    let lines = content.lines().count().max(1);
    format!("  {} chars  │  {} linhas", chars, lines)
}

// ── Window size persistence ───────────────────────────────────────────────────

fn size_file() -> PathBuf {
    cache::cache_dir().join("window.cfg")
}

fn save_window_size(win: &gtk4::ApplicationWindow) {
    let (w, h) = win.default_size();
    let _ = cache::ensure_cache_dir();
    let _ = fs::write(size_file(), format!("{}x{}", w, h));
}

fn restore_window_size(win: &gtk4::ApplicationWindow) {
    if let Ok(s) = fs::read_to_string(size_file()) {
        if let Some((w, h)) = s.trim().split_once('x') {
            if let (Ok(w), Ok(h)) = (w.parse::<i32>(), h.parse::<i32>()) {
                win.set_default_size(w, h);
                return;
            }
        }
    }
    win.set_default_size(1024, 768);
}

// ── Tab management ────────────────────────────────────────────────────────────

/// Adjusts each tab's width so a single tab takes half the window,
/// two tabs fill the full width, and three or more share equally.
fn update_tab_sizes(state: &Rc<RefCell<AppState>>, win_w: i32) {
    let s = state.borrow();
    let count = s.tabs.len();
    if count == 0 {
        return;
    }

    let notebook = s.notebook.clone();

    if count == 1 {
        let target = win_w / 2;
        for t in &s.tabs {
            t.tab_box.set_width_request(target);
            if let Some(nb) = &notebook {
                nb.page(&t.page_widget).set_tab_expand(false);
            }
        }
    } else {
        // Enforce that two or more tabs occupy all available space.
        // `tab-expand = true` enables sharing the space equally.
        for t in &s.tabs {
            // we set width_request = -1 to reset minimum requested width,
            // so we don't interfere with natural distribution for 3+ tabs.
            t.tab_box.set_width_request(-1);
            if let Some(nb) = &notebook {
                nb.page(&t.page_widget).set_tab_expand(true);
            }
        }
    }
}

/// Convenience wrapper: pulls the window out of `AppState` and updates sizes.
fn refresh_tab_sizes(state: &Rc<RefCell<AppState>>) {
    let window = state.borrow().window.clone();
    if let Some(w) = window {
        let mut win_w = w.width();
        if win_w <= 0 {
            win_w = w.default_size().0;
        }
        if win_w <= 0 {
            win_w = 1024;
        }
        update_tab_sizes(state, win_w);
    }
}

/// Schedules `refresh_tab_sizes` to run on the next GTK idle tick. Useful right
/// after `window.present()` so the window has time to allocate its real width.
fn schedule_tab_size_refresh(state: &Rc<RefCell<AppState>>) {
    let state_c = Rc::clone(state);
    glib::idle_add_local_once(move || {
        refresh_tab_sizes(&state_c);
    });
}

fn add_tab(
    state: &Rc<RefCell<AppState>>,
    notebook: &gtk4::Notebook,
    status: &gtk4::Label,
    file_path: PathBuf,
    content: String,
) {
    let hash = extract_hash(&file_path);
    let title = display_title(&content, &hash, false);

    let tab_id = {
        let mut s = state.borrow_mut();
        let id = s.next_id;
        s.next_id += 1;
        id
    };

    // ── Editor ──
    let text_view = gtk4::TextView::new();
    text_view.set_monospace(true);
    text_view.set_wrap_mode(gtk4::WrapMode::WordChar);
    text_view.set_left_margin(10);
    text_view.set_right_margin(10);
    text_view.set_top_margin(8);
    text_view.set_bottom_margin(8);
    text_view.buffer().set_text(&content);

    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_hexpand(true);
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&text_view));

    // ── Tab label: [title label] [close button] ──
    let tab_label = gtk4::Label::new(Some(&title));
    tab_label.set_hexpand(true);
    tab_label.set_xalign(0.5);
    tab_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);

    let close_btn = gtk4::Button::builder()
        .icon_name("window-close-symbolic")
        .has_frame(false)
        .tooltip_text("Fechar aba (Ctrl+W)")
        .build();
    close_btn.add_css_class("flat");
    close_btn.set_valign(gtk4::Align::Center);

    let tab_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    tab_box.append(&tab_label);
    tab_box.append(&close_btn);
    tab_box.show();

    let page_num = notebook.append_page(&scrolled, Some(&tab_box));
    notebook.set_tab_reorderable(&scrolled, true);
    notebook.page(&scrolled).set_tab_expand(true);
    notebook.set_current_page(Some(page_num));

    // ── Register in state ──
    {
        let mut s = state.borrow_mut();
        s.tabs.push(TabEntry {
            id: tab_id,
            file_path: file_path.clone(),
            hash: hash.clone(),
            text_view: text_view.clone(),
            tab_label: tab_label.clone(),
            tab_box: tab_box.clone(),
            page_widget: scrolled.clone(),
            dirty: false,
        });
    }

    refresh_tab_sizes(state);

    // ── Text change signal: mark dirty + update labels ──
    {
        let state_c = Rc::clone(state);
        let tab_label_c = tab_label.clone();
        let status_c = status.clone();
        let hash_c = hash.clone();

        text_view.buffer().connect_changed(move |buf| {
            let text = buf.text(&buf.start_iter(), &buf.end_iter(), false);
            let text = text.as_str();

            {
                let mut s = state_c.borrow_mut();
                if let Some(t) = s.tabs.iter_mut().find(|t| t.id == tab_id) {
                    if !t.dirty {
                        t.dirty = true;
                    }
                }
            }

            tab_label_c.set_text(&display_title(text, &hash_c, true));
            status_c.set_text(&status_text(text));
        });
    }

    // ── Close button signal ──
    {
        let state_c = Rc::clone(state);
        let notebook_c = notebook.clone();
        let status_c = status.clone();
        let scrolled_c = scrolled.clone();

        close_btn.connect_clicked(move |_| {
            close_tab_by_widget(&state_c, &notebook_c, &status_c, &scrolled_c, tab_id);
        });
    }

    // Focus the editor
    text_view.grab_focus();
}

fn close_tab_by_widget(
    state: &Rc<RefCell<AppState>>,
    notebook: &gtk4::Notebook,
    status: &gtk4::Label,
    page_widget: &gtk4::ScrolledWindow,
    tab_id: usize,
) {
    // Remove page from notebook
    if let Some(pn) = notebook.page_num(page_widget) {
        notebook.remove_page(Some(pn));
    }

    // Remove from state and queue file deletion
    let is_empty = {
        let mut s = state.borrow_mut();
        if let Some(idx) = s.tabs.iter().position(|t| t.id == tab_id) {
            let removed = s.tabs.remove(idx);
            if let Some(w) = &s.writer {
                w.delete(removed.file_path);
            }
        }
        s.tabs.is_empty()
    };

    if is_empty {
        if let Ok(path) = cache::create_tab_file() {
            add_tab(state, notebook, status, path, String::new());
        }
    } else {
        refresh_tab_sizes(state);
    }
}

fn close_current_tab(
    state: &Rc<RefCell<AppState>>,
    notebook: &gtk4::Notebook,
    status: &gtk4::Label,
) {
    let Some(pn) = notebook.current_page() else { return };
    let Some(child) = notebook.nth_page(Some(pn)) else { return };

    let (page_widget, tab_id) = {
        let s = state.borrow();
        s.tabs
            .iter()
            .find(|t| t.page_widget.upcast_ref::<gtk4::Widget>() == &child)
            .map(|t| (t.page_widget.clone(), t.id))
            .unzip()
    };

    if let (Some(pw), Some(id)) = (page_widget, tab_id) {
        close_tab_by_widget(state, notebook, status, &pw, id);
    }
}

fn archive_current_tab(
    state: &Rc<RefCell<AppState>>,
    notebook: &gtk4::Notebook,
    status: &gtk4::Label,
) {
    let Some(pn) = notebook.current_page() else { return };
    let Some(child) = notebook.nth_page(Some(pn)) else { return };

    let (page_widget, tab_id, file_path, content) = {
        let s = state.borrow();
        if let Some(tab) = s.tabs.iter().find(|t| t.page_widget.upcast_ref::<gtk4::Widget>() == &child) {
            let content = buffer_text(&tab.text_view);
            (Some(tab.page_widget.clone()), Some(tab.id), Some(tab.file_path.clone()), content)
        } else {
            (None, None, None, String::new())
        }
    };

    if let (Some(pw), Some(id), Some(path)) = (page_widget, tab_id, file_path) {
        // Archive the file on disk (creates archive-*.txt and removes active tab-*.txt)
        if let Err(e) = cache::archive_tab(&path, &content) {
            log::error!("Failed to archive tab {:?}: {}", path, e);
            status.set_text("  Erro ao arquivar nota");
            return;
        }

        // Remove page from notebook
        if let Some(pn) = notebook.page_num(&pw) {
            notebook.remove_page(Some(pn));
        }

        // Remove from state without deleting (archive_tab already handled removal of tab-*.txt)
        let is_empty = {
            let mut s = state.borrow_mut();
            if let Some(idx) = s.tabs.iter().position(|t| t.id == id) {
                s.tabs.remove(idx);
            }
            s.tabs.is_empty()
        };

        status.set_text("  Nota arquivada com sucesso");

        if is_empty {
            if let Ok(new_path) = cache::create_tab_file() {
                add_tab(state, notebook, status, new_path, String::new());
            }
        } else {
            refresh_tab_sizes(state);
        }
    }
}

fn open_archived_dialog(
    parent_window: &gtk4::ApplicationWindow,
    state: &Rc<RefCell<AppState>>,
    notebook: &gtk4::Notebook,
    status: &gtk4::Label,
) {
    let notes = match cache::scan_archives() {
        Ok(n) => n,
        Err(e) => {
            log::error!("Failed to scan archives: {}", e);
            vec![]
        }
    };

    let dialog = gtk4::Window::builder()
        .title("Notas Arquivadas")
        .transient_for(parent_window)
        .modal(true)
        .default_width(600)
        .default_height(460)
        .build();

    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    root.set_margin_start(16);
    root.set_margin_end(16);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    dialog.set_child(Some(&root));

    // Header title
    let header_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let title_lbl = gtk4::Label::new(Some("Notas Arquivadas"));
    title_lbl.add_css_class("title-2");
    title_lbl.set_xalign(0.0);
    title_lbl.set_hexpand(true);
    header_box.append(&title_lbl);
    root.append(&header_box);

    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_hexpand(true);
    scrolled.set_min_content_height(280);

    let list_box = gtk4::ListBox::new();
    list_box.set_selection_mode(gtk4::SelectionMode::None);
    list_box.add_css_class("boxed-list");
    scrolled.set_child(Some(&list_box));
    root.append(&scrolled);

    let empty_label = gtk4::Label::new(Some("Nenhuma nota arquivada encontrada."));
    empty_label.set_margin_top(40);
    empty_label.set_margin_bottom(40);
    empty_label.add_css_class("dim-label");

    if notes.is_empty() {
        list_box.append(&empty_label);
    }

    // Bottom action bar
    let action_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    action_bar.set_halign(gtk4::Align::End);

    let close_btn = gtk4::Button::builder().label("Fechar").build();
    action_bar.append(&close_btn);
    root.append(&action_bar);

    let dialog_c = dialog.clone();
    close_btn.connect_clicked(move |_| {
        dialog_c.close();
    });

    for note in &notes {
        let row = gtk4::ListBoxRow::new();
        let item_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        item_box.set_margin_start(12);
        item_box.set_margin_end(12);
        item_box.set_margin_top(8);
        item_box.set_margin_bottom(8);

        // Text details (vertical)
        let text_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        text_box.set_hexpand(true);

        // Top line: title + date
        let top_line = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let note_title = gtk4::Label::new(Some(&note.title));
        note_title.add_css_class("heading");
        note_title.set_xalign(0.0);
        note_title.set_hexpand(true);
        note_title.set_ellipsize(gtk4::pango::EllipsizeMode::End);

        let date_lbl = gtk4::Label::new(Some(&note.timestamp_display));
        date_lbl.add_css_class("dim-label");
        date_lbl.add_css_class("caption");
        date_lbl.set_xalign(1.0);

        top_line.append(&note_title);
        top_line.append(&date_lbl);
        text_box.append(&top_line);

        // Preview snippet
        let snippet = if note.preview.trim().is_empty() {
            "(Nota vazia)".to_string()
        } else {
            note.preview.lines().take(2).collect::<Vec<_>>().join(" · ")
        };
        let preview_lbl = gtk4::Label::new(Some(&snippet));
        preview_lbl.set_xalign(0.0);
        preview_lbl.add_css_class("dim-label");
        preview_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        text_box.append(&preview_lbl);

        // Details: chars / lines
        let stats_lbl = gtk4::Label::new(Some(&format!("{} caracteres  │  {} linhas", note.char_count, note.line_count)));
        stats_lbl.set_xalign(0.0);
        stats_lbl.add_css_class("caption");
        stats_lbl.add_css_class("dim-label");
        text_box.append(&stats_lbl);

        item_box.append(&text_box);

        // Action buttons: Restore & Delete
        let btn_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        btn_box.set_valign(gtk4::Align::Center);

        let restore_btn = gtk4::Button::builder()
            .label("Restaurar")
            .tooltip_text("Restaurar e abrir como nota ativa")
            .build();
        restore_btn.add_css_class("suggested-action");

        let delete_btn = gtk4::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Excluir nota arquivada definitivamente")
            .has_frame(false)
            .build();
        delete_btn.add_css_class("flat");

        btn_box.append(&restore_btn);
        btn_box.append(&delete_btn);
        item_box.append(&btn_box);

        row.set_child(Some(&item_box));
        list_box.append(&row);

        // Restore click handler
        {
            let note_path = note.path.clone();
            let state_c = Rc::clone(state);
            let notebook_c = notebook.clone();
            let status_c = status.clone();
            let dialog_c = dialog.clone();

            restore_btn.connect_clicked(move |_| {
                match cache::unarchive_tab(&note_path) {
                    Ok((tab_path, content)) => {
                        add_tab(&state_c, &notebook_c, &status_c, tab_path, content);
                        status_c.set_text("  Nota restaurada do arquivo com sucesso");
                        dialog_c.close();
                    }
                    Err(e) => {
                        log::error!("Failed to unarchive note {:?}: {}", note_path, e);
                        status_c.set_text("  Erro ao restaurar nota arquivada");
                    }
                }
            });
        }

        // Delete click handler
        {
            let note_path = note.path.clone();
            let list_box_c = list_box.clone();
            let row_c = row.clone();
            let status_c = status.clone();
            let empty_label_c = empty_label.clone();

            delete_btn.connect_clicked(move |_| {
                if let Err(e) = cache::delete_archive_file(&note_path) {
                    log::error!("Failed to delete archive {:?}: {}", note_path, e);
                    status_c.set_text("  Erro ao excluir arquivo");
                } else {
                    list_box_c.remove(&row_c);
                    status_c.set_text("  Nota arquivada excluída");
                    if list_box_c.first_child().is_none() {
                        list_box_c.append(&empty_label_c);
                    }
                }
            });
        }
    }

    dialog.present();
}

// ── App builder ───────────────────────────────────────────────────────────────

pub fn build_ui(app: &gtk4::Application) {
    let state = Rc::new(RefCell::new(AppState::new()));

    // ── Window ──
    let window = gtk4::ApplicationWindow::builder()
        .application(app)
        .title("memory-work-editor")
        .build();
    restore_window_size(&window);

    state.borrow_mut().window = Some(window.clone());

    // ── Layout ──
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    window.set_child(Some(&root));

    let notebook = gtk4::Notebook::new();
    notebook.set_show_tabs(true);
    notebook.set_vexpand(true);
    root.append(&notebook);

    // ── Botões de ação nas abas: Nova aba, Arquivar e Abrir arquivados ──
    let actions_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
    actions_box.set_margin_start(4);
    actions_box.set_margin_end(4);

    let new_tab_btn = gtk4::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text("Nova aba (Ctrl+T)")
        .has_frame(false)
        .build();
    new_tab_btn.add_css_class("flat");
    actions_box.append(&new_tab_btn);

    let archive_tab_btn = gtk4::Button::builder()
        .icon_name("folder-download-symbolic")
        .tooltip_text("Arquivar nota atual (Ctrl+Alt+A)")
        .has_frame(false)
        .build();
    archive_tab_btn.add_css_class("flat");
    actions_box.append(&archive_tab_btn);

    let open_archived_btn = gtk4::Button::builder()
        .icon_name("document-open-symbolic")
        .tooltip_text("Abrir notas arquivadas (Ctrl+O)")
        .has_frame(false)
        .build();
    open_archived_btn.add_css_class("flat");
    actions_box.append(&open_archived_btn);

    notebook.set_action_widget(&actions_box, gtk4::PackType::Start);

    state.borrow_mut().notebook = Some(notebook.clone());

    // ── Search bar ──
    let search_entry = gtk4::SearchEntry::new();
    search_entry.set_placeholder_text(Some("Procurar..."));
    search_entry.set_hexpand(true);

    let search_bar = gtk4::SearchBar::new();
    search_bar.set_child(Some(&search_entry));
    search_bar.set_show_close_button(true);
    search_bar.connect_entry(&search_entry);
    root.append(&search_bar);

    state.borrow_mut().search_bar = Some(search_bar.clone());
    state.borrow_mut().search_entry = Some(search_entry.clone());

    // Separator between notebook and status bar
    root.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    let status = gtk4::Label::new(Some("  Pronto"));
    status.set_xalign(0.0);
    status.set_margin_top(3);
    status.set_margin_bottom(3);
    root.append(&status);

    // ── Connect actions buttons ──
    {
        let state_c = Rc::clone(&state);
        let notebook_c = notebook.clone();
        let status_c = status.clone();
        new_tab_btn.connect_clicked(move |_| {
            if let Ok(path) = cache::create_tab_file() {
                add_tab(&state_c, &notebook_c, &status_c, path, String::new());
            }
        });
    }
    {
        let state_c = Rc::clone(&state);
        let notebook_c = notebook.clone();
        let status_c = status.clone();
        archive_tab_btn.connect_clicked(move |_| {
            archive_current_tab(&state_c, &notebook_c, &status_c);
        });
    }
    {
        let state_c = Rc::clone(&state);
        let notebook_c = notebook.clone();
        let status_c = status.clone();
        let win_c = window.clone();
        open_archived_btn.connect_clicked(move |_| {
            open_archived_dialog(&win_c, &state_c, &notebook_c, &status_c);
        });
    }

    // ── Search functionality ──
    {
        let state_c = Rc::clone(&state);
        let notebook_c = notebook.clone();
        search_entry.connect_search_changed(move |entry| {
            let search_text = entry.text();
            
            let s = state_c.borrow();
            if let Some(pn) = notebook_c.current_page() {
                if let Some(child) = notebook_c.nth_page(Some(pn)) {
                    if let Some(tab) = s.tabs.iter().find(|t| t.page_widget.upcast_ref::<gtk4::Widget>() == &child) {
                        let buffer = tab.text_view.buffer();
                        
                        // Get or create search highlight tag
                        let tag_table = buffer.tag_table();
                        let tag = if let Some(existing_tag) = tag_table.lookup("search-match") {
                            existing_tag
                        } else {
                            buffer.create_tag(
                                Some("search-match"),
                                &[("background", &"yellow"), ("foreground", &"black")],
                            ).expect("Failed to create search-match tag")
                        };

                        // Remove previous search highlights
                        buffer.remove_tag(&tag, &buffer.start_iter(), &buffer.end_iter());

                        // If search text is empty, just clear highlights and return
                        if search_text.is_empty() {
                            return;
                        }

                        // Search and highlight all occurrences
                        let mut start = buffer.start_iter();
                        let search_str = search_text.as_str();
                        
                        while let Some((match_start, match_end)) = start.forward_search(
                            search_str,
                            gtk4::TextSearchFlags::CASE_INSENSITIVE,
                            None,
                        ) {
                            buffer.apply_tag(&tag, &match_start, &match_end);
                            start = match_end;
                        }

                        // Move cursor to first match
                        let first_match = buffer.start_iter().forward_search(
                            search_str,
                            gtk4::TextSearchFlags::CASE_INSENSITIVE,
                            None,
                        );
                        if let Some((match_start, _)) = first_match {
                            buffer.place_cursor(&match_start);
                            tab.text_view.scroll_to_iter(&mut match_start.clone(), 0.1, false, 0.0, 0.0);
                        }
                    }
                }
            }
        });

        // Close search bar with Escape
        let search_bar_c = search_bar.clone();
        search_entry.connect_stop_search(move |_| {
            search_bar_c.set_search_mode(false);
        });
    }

    // ── Restore session or open first tab ──
    match cache::scan_cache() {
        Ok(entries) if !entries.is_empty() => {
            for (path, content) in entries {
                add_tab(&state, &notebook, &status, path, content);
            }
        }
        _ => {
            if let Ok(path) = cache::create_tab_file() {
                add_tab(&state, &notebook, &status, path, String::new());
            }
        }
    }

    // ── Update status bar on tab switch ──
    {
        let state_c = Rc::clone(&state);
        let status_c = status.clone();
        notebook.connect_switch_page(move |_nb, child, _pn| {
            let s = state_c.borrow();
            if let Some(t) = s.tabs.iter().find(|t| t.page_widget.upcast_ref::<gtk4::Widget>() == child) {
                let content = buffer_text(&t.text_view);
                status_c.set_text(&status_text(&content));
                // Focus the text view of the activated tab
                t.text_view.grab_focus();
            }
        });
    }

    // ── React to window resize: keep tab widths in sync ──
    {
        let state_c = Rc::clone(&state);
        let win_c = window.clone();
        let mut last_w = 0;
        glib::timeout_add_local(Duration::from_millis(100), move || {
            let curr_w = win_c.width();
            if curr_w > 0 && curr_w != last_w {
                last_w = curr_w;
                update_tab_sizes(&state_c, curr_w);
            }
            glib::ControlFlow::Continue
        });
    }

    // ── Auto-save timer (2s) ──
    {
        let state_c = Rc::clone(&state);
        glib::timeout_add_local(Duration::from_secs(2), move || {
            state_c.borrow_mut().flush_dirty();
            glib::ControlFlow::Continue
        });
    }

    // ── Keyboard shortcuts ──
    {
        let state_c = Rc::clone(&state);
        let notebook_c = notebook.clone();
        let status_c = status.clone();
        let win_c = window.clone();

        let ctrl = gtk4::EventControllerKey::new();
        ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture);
        ctrl.connect_key_pressed(move |_, key, _, modifiers| {
            if !modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
                return glib::Propagation::Proceed;
            }

            // Ctrl+Alt+A or Ctrl+Shift+A -> Arquivar nota atual
            let is_alt = modifiers.contains(gdk::ModifierType::ALT_MASK);
            let is_shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
            if (is_alt || is_shift) && (key == gdk::Key::a || key == gdk::Key::A) {
                archive_current_tab(&state_c, &notebook_c, &status_c);
                return glib::Propagation::Stop;
            }

            match key {
                gdk::Key::t => {
                    if let Ok(path) = cache::create_tab_file() {
                        add_tab(&state_c, &notebook_c, &status_c, path, String::new());
                    }
                    glib::Propagation::Stop
                }
                gdk::Key::w => {
                    close_current_tab(&state_c, &notebook_c, &status_c);
                    glib::Propagation::Stop
                }
                gdk::Key::f => {
                    let s = state_c.borrow();
                    if let Some(search_bar) = &s.search_bar {
                        search_bar.set_search_mode(true);
                        if let Some(search_entry) = &s.search_entry {
                            search_entry.grab_focus();
                        }
                    }
                    glib::Propagation::Stop
                }
                gdk::Key::o => {
                    open_archived_dialog(&win_c, &state_c, &notebook_c, &status_c);
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        window.add_controller(ctrl);
    }

    // ── Window close: flush dirty + shutdown writer ──
    {
        let state_c = Rc::clone(&state);
        window.connect_close_request(move |win| {
            save_window_size(win);

            let mut s = state_c.borrow_mut();
            // Synchronous final flush so nothing is lost on exit
            for t in &s.tabs {
                if t.dirty {
                    let content = buffer_text(&t.text_view);
                    let tmp = t.file_path.with_extension("tmp");
                    if fs::write(&tmp, content.as_bytes()).is_ok() {
                        let _ = fs::rename(&tmp, &t.file_path);
                    }
                }
            }
            if let Some(writer) = s.writer.take() {
                writer.shutdown();
            }
            glib::Propagation::Proceed
        });
    }

    window.present();

    // Once the window has been allocated by the compositor, recompute tab widths
    // using the real window size (initial calls during session restore happen
    // before realization, when `width()` is still 0).
    schedule_tab_size_refresh(&state);
}
