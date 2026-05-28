mod app;
mod cache;
mod shortcuts;
mod tab;
mod ui;

fn main() {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("memory-work-editor")
            .with_inner_size([1024.0, 768.0]),
        persist_window: true,
        ..Default::default()
    };

    eframe::run_native(
        "memory-work-editor",
        options,
        Box::new(|cc| Ok(Box::new(app::EditorApp::new(cc)))),
    )
    .expect("Failed to start eframe");
}
