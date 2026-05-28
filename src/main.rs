mod cache;
mod gtk_app;

use gtk4::prelude::*;

fn main() {
    env_logger::init();

    let app = gtk4::Application::builder()
        .application_id("io.github.memory-work-editor")
        .build();

    app.connect_activate(|a| gtk_app::build_ui(a));

    std::process::exit(app.run().value());
}
