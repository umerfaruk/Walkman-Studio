//! Modal editor sheets (spec task 16), one module per sheet.

use eframe::egui;

use crate::theme::{color, font, glass};

pub mod album_metadata;
pub mod song_metadata;
pub mod transfer_item;

/// A centered modal: a dimmed full-screen backdrop plus a [`glass::modal_glass_chassis`]
/// panel sized `size`. The same `Area`-based overlay pattern `now_playing.rs`'s fullscreen
/// player and `views/local_library.rs`'s sheets already established, shared here so every
/// task-16 editor sheet uses one implementation.
pub(crate) fn modal(ctx: &egui::Context, id: &str, size: egui::Vec2, content: impl FnOnce(&mut egui::Ui)) {
    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new(id)).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
        ui.set_min_size(screen.size());
        ui.painter().rect_filled(screen, 0.0, egui::Color32::from_black_alpha(140));

        let panel_rect = egui::Rect::from_center_size(screen.center(), size);
        let mut panel_ui = ui.new_child(egui::UiBuilder::new().max_rect(panel_rect));
        glass::modal_glass_chassis().show(&mut panel_ui, |ui| {
            ui.set_min_size(size - egui::vec2(32.0, 32.0));
            content(ui);
        });
    });
}

/// A labeled single-line text field, shared by every task-16 editor sheet.
pub(crate) fn text_field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.label(egui::RichText::new(label).font(font::caption()).color(color::SILVER_MUTED));
    ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY).font(font::body()));
    ui.add_space(8.0);
}
