use egui::{FontId, TextEdit, Ui};

use crate::tab::Tab;

/// Renders the text editor area for the given tab.
/// Returns `true` if the content was changed this frame.
pub fn render_editor(ui: &mut Ui, tab: &mut Tab) -> bool {
    let available = ui.available_size();
    let output = ui.add_sized(
        available,
        TextEdit::multiline(&mut tab.content)
            .font(FontId::monospace(14.0))
            .desired_width(f32::INFINITY)
            .hint_text("Comece a digitar…"),
    );
    output.changed()
}
