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
}

impl AppState {
    fn new() -> Self {
        Self {
            tabs: vec![],
            writer: Some(CacheWriter::spawn()),
            next_id: 0,
            window: None,
            notebook: None,
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

    // ── Botão de nova aba (barra nativa será mantida)
    let new_tab_btn = gtk4::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text("Nova aba (Ctrl+T)")
        .build();

    // ── Layout ──
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    window.set_child(Some(&root));

    // Top internal bar with the new-tab button (keep native titlebar)
    // let top_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    // top_bar.set_margin_top(6);
    // top_bar.set_margin_start(6);
    // top_bar.append(&new_tab_btn);
    // root.append(&top_bar);

    let notebook = gtk4::Notebook::new();
    notebook.set_show_tabs(true);
    notebook.set_vexpand(true);
    root.append(&notebook);

    state.borrow_mut().notebook = Some(notebook.clone());

    // Separator between notebook and status bar
    root.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    let status = gtk4::Label::new(Some("  Pronto"));
    status.set_xalign(0.0);
    status.set_margin_top(3);
    status.set_margin_bottom(3);
    root.append(&status);

    // ── Connect new-tab button ──
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

        let ctrl = gtk4::EventControllerKey::new();
        ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture);
        ctrl.connect_key_pressed(move |_, key, _, modifiers| {
            if !modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
                return glib::Propagation::Proceed;
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
