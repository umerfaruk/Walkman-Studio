//! Walkman Studio for Windows — application entry point.
//!
//! Spec tasks 1 (bootstrap) & 2 (open a dark window at the enforced size). The full
//! shell (sidebar, toolbar, inspector, now-playing bar) is built in task 4.

// On Windows, suppress the console window in release builds (GUI subsystem).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod dispatch;
mod lifecycle;
mod services;

use eframe::egui;
use walkman_ui::theme::{color, sizing};
use walkman_ui::AppState;

use lifecycle::Lifecycle;
use services::AppServices;

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
            let services = AppServices::new()?;
            Ok(Box::new(WalkmanStudioApp {
                state: AppState::default(),
                services,
                lifecycle: Lifecycle::default(),
                dispatch: dispatch::Dispatch::default(),
            }))
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

/// Root application. Owns the shared [`AppState`], the real [`AppServices`] it's wired
/// against (task 18), and renders the three-column shell each frame.
struct WalkmanStudioApp {
    state: AppState,
    services: AppServices,
    lifecycle: Lifecycle,
    dispatch: dispatch::Dispatch,
}

impl eframe::App for WalkmanStudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Device attach/detach + library lifecycle (task 18): runs before the commands
        // below so a just-detected device/just-finished scan is reflected in the same
        // frame's command handling (e.g. a `Play` command resolving against fresh tracks).
        self.lifecycle.tick(&mut self.services, &mut self.state);

        // Three-column shell: fixed 260pt sidebar | flexible detail | optional 300pt
        // inspector. Default section on launch is Local Music Library (set in AppState).
        walkman_ui::layout_shell(ctx, &mut self.state);

        // Drains every view's emitted commands against the real services (task 18).
        self.dispatch.tick(&self.services, &mut self.state);
        lifecycle::refresh_sidebar_counts(&mut self.state);

        // The device-event/async-scan lifecycle above is poll-based, not event-driven from
        // egui's perspective, so keep repainting even with no user input (R3.6/18.4).
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }
}
