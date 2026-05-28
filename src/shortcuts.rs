use egui::{Context, Key};

pub struct ShortcutAction {
    pub new_tab: bool,
    pub close_tab: bool,
    pub next_tab: bool,
    pub prev_tab: bool,
}

pub fn check_shortcuts(ctx: &Context) -> ShortcutAction {
    let mut action = ShortcutAction {
        new_tab: false,
        close_tab: false,
        next_tab: false,
        prev_tab: false,
    };

    ctx.input(|i| {
        let ctrl = i.modifiers.ctrl || i.modifiers.mac_cmd;
        let shift = i.modifiers.shift;

        if ctrl && !shift && i.key_pressed(Key::T) {
            action.new_tab = true;
        }
        if ctrl && !shift && i.key_pressed(Key::W) {
            action.close_tab = true;
        }
        if ctrl && !shift && i.key_pressed(Key::Tab) {
            action.next_tab = true;
        }
        if ctrl && shift && i.key_pressed(Key::Tab) {
            action.prev_tab = true;
        }
    });

    action
}
