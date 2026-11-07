//! Walkman Studio for Windows — application entry point.
//!
//! Spec tasks 1 (bootstrap) & 2 (open a dark window at the enforced size). The full
//! shell (sidebar, toolbar, inspector, now-playing bar) is built in task 4.

// On Windows, suppress the console window in release builds (GUI subsystem).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use eframe::egui;
use walkman_ui::theme::{color, sizing};
use walkman_ui::AppState;

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Walkman Studio")
            .with_inner_size([sizing::WINDOW_DEFAULT.0, sizing::WINDOW_DEFAULT.1])
            .with_min_inner_size([sizing::WINDOW_MIN.0, sizing::WINDOW_MIN.1]),
        ..Default::default()
    };

    eframe::run_native(
        "Walkman Studio",
        options,
        Box::new(|cc| {
            walkman_ui::theme::install_fonts(&cc.egui_ctx);
            apply_obsidian_visuals(&cc.egui_ctx);
            Ok(Box::new(WalkmanStudioApp::default()))
        }),
    )
}

/// Applies the flat obsidian base look (no glow, dark surfaces). The full design
/// system lands in tasks 3.1–3.3.
fn apply_obsidian_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = color::BACKGROUND;
    visuals.window_fill = color::CARD;
    visuals.extreme_bg_color = color::INPUT;
    ctx.set_visuals(visuals);
}

/// Root application. Owns the shared [`AppState`] and renders the three-column shell
/// each frame (task 4.1). The sidebar/toolbar/detail-router/now-playing content is filled
/// in by tasks 4.2–4.5.
#[derive(Default)]
struct WalkmanStudioApp {
    state: AppState,
}

impl eframe::App for WalkmanStudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Three-column shell: fixed 260pt sidebar | flexible detail | optional 300pt
        // inspector. Default section on launch is Local Music Library (set in AppState).
        walkman_ui::layout_shell(ctx, &mut self.state);
    }
}
