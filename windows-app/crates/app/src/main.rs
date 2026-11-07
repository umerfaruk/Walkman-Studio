//! Walkman Studio for Windows — application entry point.
//!
//! Spec tasks 1 (bootstrap) & 2 (open a dark window at the enforced size). The full
//! shell (sidebar, toolbar, inspector, now-playing bar) is built in task 4.

// On Windows, suppress the console window in release builds (GUI subsystem).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use eframe::egui;
use walkman_ui::theme::{color, sizing};
use walkman_ui::NavTab;

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

/// Root application state. Expanded into the full `AppState` in task 4.
struct WalkmanStudioApp {
    nav_tab: NavTab,
}

impl Default for WalkmanStudioApp {
    fn default() -> Self {
        // Default section on launch matches the macOS app: Local Music Library.
        Self { nav_tab: NavTab::LocalLibrary }
    }
}

impl eframe::App for WalkmanStudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Placeholder shell for bootstrap. The three-column layout (sidebar /
        // detail / inspector) is implemented in task 4.1.
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(24.0);
                ui.heading("Walkman Studio");
                ui.label("Windows (Rust) port — workspace bootstrap");
                ui.add_space(12.0);
                ui.label(format!("Current section: {}", self.nav_tab.title()));
            });
        });
    }
}
