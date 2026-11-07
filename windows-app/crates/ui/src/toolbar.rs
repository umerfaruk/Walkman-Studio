//! The top toolbar + menu commands, ported from the `ContentView` toolbar and the
//! `WalkmanStudioApp` `.commands { … }` block (`Sources/WalkmanStudio/Views/ContentView.swift`,
//! `Sources/WalkmanStudio/WalkmanStudioApp.swift`).
//!
//! Spec task **4.3**:
//! - a top toolbar panel at the 38pt control-height baseline with **Refresh**, **Eject**
//!   (a spinner replaces the glyph while ejecting; the button is disabled when no Walkman
//!   is connected), and an **Inspector** toggle, using the theme button look + the
//!   toolbar icon mapping (task 3.4);
//! - an in-app **menu bar** (`File` / `Help`) exposing `Manage Local Folders…` and
//!   `About Walkman Studio` with their shortcut labels, matching the macOS command set;
//! - the keyboard shortcuts **Ctrl+I** (toggle inspector), **Ctrl+Shift+L** and
//!   **Ctrl+Shift+O** (both open Manage Local Folders, as in the macOS `CommandGroup`s);
//! - the inspector **widen-on-open** window resize, replicating the macOS
//!   `handleInspectorVisibilityChanged` logic (grow by [`INSPECTOR_DELTA`] when the screen
//!   has room, otherwise overlay; shrink back on close).
//!
//! ## Pure logic vs. rendering / platform
//!
//! The *decisions* are factored into `Ui`/`Context`-free helpers so they unit-test without
//! a live egui context:
//! - [`Command`] + [`match_shortcut`] map a key+modifier combo to a toolbar/menu command.
//! - [`plan_inspector_resize`] turns the current window width, the monitor width, and the
//!   saved pre-open width into the new window width (or `None` to leave the window be and
//!   let the inspector overlay).
//!
//! [`render`] paints the toolbar + menu and raises the [`AppState`] request flags;
//! [`apply_inspector_resize`] runs the resize once per open/close transition by sending an
//! eframe viewport command. Nothing here performs the real refresh/eject/sheet work — that
//! lands in the service-wiring tasks (9.4/16/17.7/18); the toolbar only raises flags.

use eframe::egui;

use crate::shell::AppState;
use crate::theme::icon::Icon;
use crate::theme::{button, color, font, sizing};

/// The inspector width the window grows by when the inspector opens. Matches the macOS
/// `inspectorWidth: CGFloat = 300.0` in `handleInspectorVisibilityChanged` (the window
/// gains exactly the inspector pane's width; the 1px divider is absorbed by the detail
/// pane, as on macOS).
pub const INSPECTOR_DELTA: f32 = sizing::INSPECTOR_WIDTH;

// ============================ Menu / toolbar commands ============================

/// A toolbar or menu command the user can trigger. Keeps the keyboard mapping and the
/// click handlers reasoning about one closed set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Reload connected drives + library (toolbar `Refresh`).
    Refresh,
    /// Clean macOS/Windows junk and safely unmount (toolbar `Eject`).
    Eject,
    /// Toggle the metadata/artwork inspector (toolbar `Inspector`, Ctrl+I).
    ToggleInspector,
    /// Open the About sheet (Help menu / bottom sidebar button).
    About,
    /// Open `Manage Local Folders…` (File menu, Ctrl+Shift+L / Ctrl+Shift+O).
    ManageLocalFolders,
}

/// Resolve a pressed key + modifiers to a [`Command`], mirroring the macOS keyboard
/// shortcuts with `Cmd`→`Ctrl` (the Windows convention):
/// - `Ctrl+I` → [`Command::ToggleInspector`] (`keyboardShortcut("i", .command)`);
/// - `Ctrl+Shift+L` and `Ctrl+Shift+O` → [`Command::ManageLocalFolders`]
///   (the two `CommandGroup`s that both post `.manageLocalFolders`).
///
/// Returns `None` for any other combo. Pure so the mapping is unit-tested without input
/// plumbing.
pub fn match_shortcut(key: egui::Key, modifiers: egui::Modifiers) -> Option<Command> {
    // Primary modifier is Ctrl on Windows; `command` is kept matching for mac dev hosts.
    let ctrl = modifiers.ctrl || modifiers.command;
    if !ctrl {
        return None;
    }
    match (key, modifiers.shift) {
        // Ctrl+I toggles the inspector (no Shift).
        (egui::Key::I, false) => Some(Command::ToggleInspector),
        // Ctrl+Shift+L and Ctrl+Shift+O both open Manage Local Folders.
        (egui::Key::L, true) | (egui::Key::O, true) => Some(Command::ManageLocalFolders),
        _ => None,
    }
}

/// Apply a resolved [`Command`] to `state` by raising the matching request flag / toggling
/// the inspector. Pure over `AppState` (no `Ui`), so command dispatch is unit-tested
/// directly. The actual work (rescan, eject, sheets) is performed later off these flags.
pub fn dispatch(state: &mut AppState, command: Command) {
    match command {
        Command::Refresh => state.refresh_requested = true,
        Command::Eject => {
            // Ignore while a Walkman is absent or an eject is already running — matches
            // the macOS `guard !drives.isEmpty, !isEjecting` in `performCleanAndEject`.
            if state.walkman_connected && !state.is_ejecting {
                state.eject_requested = true;
            }
        }
        Command::ToggleInspector => state.show_inspector = !state.show_inspector,
        Command::About => state.about_requested = true,
        Command::ManageLocalFolders => state.manage_folders_requested = true,
    }
}

// ============================ Inspector widen-on-open resize ============================

/// The pure decision for the inspector window resize, replicating the macOS
/// `handleInspectorVisibilityChanged` math.
///
/// - `opening`: `true` when the inspector just became visible, `false` when it closed.
/// - `current_width`: the window's current inner width (points).
/// - `monitor_width`: the usable monitor width (points), or `None` when unknown — the
///   macOS code falls back to the current frame, i.e. never grows past what it already is.
/// - `min_width`: the enforced minimum window width ([`sizing::WINDOW_MIN`]`.0`).
/// - `saved_width`: the width captured just before the inspector opened (used to restore
///   on close); `None` when nothing was saved.
///
/// Returns `Some(new_width)` to resize the window, or `None` to leave it unchanged (which
/// makes the inspector *overlay* the detail pane, exactly as macOS does when there is no
/// room to grow).
///
/// Growing is clamped to the monitor width, so on a screen with no spare room the window
/// stays put and the inspector overlays. Shrinking restores the saved pre-open width (or,
/// lacking one, subtracts the inspector width), never going below `min_width`.
pub fn plan_inspector_resize(
    opening: bool,
    current_width: f32,
    monitor_width: Option<f32>,
    min_width: f32,
    saved_width: Option<f32>,
) -> Option<f32> {
    if opening {
        // Grow by the inspector width, clamped to the monitor so we never exceed the
        // screen (macOS `min(current + inspectorWidth, visibleFrame.width)`).
        let wanted = current_width + INSPECTOR_DELTA;
        let target = match monitor_width {
            Some(mw) => wanted.min(mw),
            // Unknown monitor: don't grow past the current width (overlay instead).
            None => return None,
        };
        // Only resize if we actually gained room; otherwise overlay.
        if target > current_width + 0.5 {
            Some(target)
        } else {
            None
        }
    } else {
        // Closing: restore the saved pre-open width, else subtract the inspector width.
        // Never shrink below the minimum window width.
        let target = match saved_width {
            Some(saved) => saved.max(min_width),
            None => (current_width - INSPECTOR_DELTA).max(min_width),
        };
        if (target - current_width).abs() > 0.5 {
            Some(target)
        } else {
            None
        }
    }
}

/// Detects the inspector open/close transition since the previous frame and, when one
/// occurred, resizes the window via an eframe viewport command — the runtime counterpart
/// of [`plan_inspector_resize`].
///
/// This is guarded so the resize fires **only on the transition** (not every frame): it
/// compares `state.show_inspector` against `state.prev_show_inspector`, acts once, then
/// records the new value. On open it stashes the pre-open width in
/// [`AppState::saved_width_before_inspector`]; on close it restores and clears it.
///
/// Call once per frame (e.g. at the top of the shell layout). In macOS native fullscreen
/// the resize is skipped; the eframe analog is to skip when the viewport reports maximized
/// or fullscreen, so a maximized window is never shrunk out from under the user.
pub fn apply_inspector_resize(ctx: &egui::Context, state: &mut AppState) {
    let now = state.show_inspector;
    let was = state.prev_show_inspector;
    if now == was {
        return; // No transition this frame.
    }
    // Record the new state up front so a skipped resize (fullscreen/maximized) still
    // clears the pending transition and doesn't retrigger next frame.
    state.prev_show_inspector = now;

    // Skip frame resizing when the window is fullscreen/maximized (macOS guards on
    // `.fullScreen`); shrinking/growing a maximized window would be jarring.
    let (current_width, monitor_width, blocked) = ctx.input(|i| {
        let vp = i.viewport();
        let blocked = vp.fullscreen.unwrap_or(false) || vp.maximized.unwrap_or(false);
        let current = i.screen_rect().width();
        let monitor = vp.monitor_size.map(|m| m.x);
        (current, monitor, blocked)
    });

    if now {
        // Opening: remember the pre-open width to restore later.
        state.saved_width_before_inspector = Some(current_width);
    }
    let saved = if now {
        None
    } else {
        state.saved_width_before_inspector.take()
    };

    if blocked {
        return;
    }

    let min_width = sizing::WINDOW_MIN.0;
    if let Some(new_width) =
        plan_inspector_resize(now, current_width, monitor_width, min_width, saved)
    {
        let height = ctx.input(|i| i.screen_rect().height());
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
            new_width, height,
        )));
    }
}

// ============================ Rendering ============================

/// Toolbar panel vertical padding so the 38pt controls sit centered with breathing room.
const TOOLBAR_PAD_Y: f32 = 7.0;
/// Horizontal inset at the toolbar edges.
const TOOLBAR_PAD_X: f32 = 12.0;
/// Width of a square icon-only toolbar button (38pt control baseline).
const TOOLBAR_BTN: f32 = sizing::CONTROL_BASELINE;

/// Renders the top toolbar panel (menu bar + Refresh/Eject/Inspector controls), handles
/// the keyboard shortcuts, and raises the matching [`AppState`] flags. Call inside
/// [`crate::shell::layout_shell`] before the three columns so the toolbar spans the top.
///
/// The inspector widen-on-open resize ([`apply_inspector_resize`]) is driven separately
/// from the shell so it runs exactly once per frame regardless of panel ordering.
pub fn render(ctx: &egui::Context, state: &mut AppState) {
    // --- Global keyboard shortcuts (Ctrl+I / Ctrl+Shift+L / Ctrl+Shift+O). ---
    handle_shortcuts(ctx, state);

    egui::TopBottomPanel::top("walkman_toolbar")
        .exact_height(TOOLBAR_BTN + TOOLBAR_PAD_Y * 2.0)
        .frame(
            egui::Frame::none()
                .fill(color::BACKGROUND)
                .inner_margin(egui::Margin::symmetric(TOOLBAR_PAD_X, TOOLBAR_PAD_Y)),
        )
        .show(ctx, |ui| {
            // 1px bottom hairline divider (matches the sidebar/inspector rims).
            let full = ui.max_rect();
            let divider = egui::Rect::from_min_max(
                egui::pos2(full.min.x - TOOLBAR_PAD_X, full.max.y + TOOLBAR_PAD_Y - 1.0),
                egui::pos2(full.max.x + TOOLBAR_PAD_X, full.max.y + TOOLBAR_PAD_Y),
            );
            ui.painter().rect_filled(divider, 0.0, color::BORDER);

            ui.horizontal_centered(|ui| {
                menu_bar(ui, state);

                // Toolbar actions are right-aligned, matching the trailing toolbar on
                // macOS (the title area sits leading, the actions trail).
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    inspector_button(ui, state);
                    ui.add_space(6.0);
                    eject_button(ui, state);
                    ui.add_space(6.0);
                    refresh_button(ui, state);
                });
            });
        });
}

/// Handles the global keyboard shortcuts once per frame via [`match_shortcut`] + [`dispatch`].
fn handle_shortcuts(ctx: &egui::Context, state: &mut AppState) {
    // Collect the commands triggered this frame, then dispatch (avoids borrowing input
    // while mutating state).
    let commands: Vec<Command> = ctx.input(|i| {
        i.events
            .iter()
            .filter_map(|ev| match ev {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => match_shortcut(*key, *modifiers),
                _ => None,
            })
            .collect()
    });
    for command in commands {
        dispatch(state, command);
    }
}

/// The in-app `File` / `Help` menu bar. egui has no OS menu bar, so an `egui::menu::bar`
/// cluster exposes the same commands the macOS `.commands { … }` block does, with their
/// shortcut labels shown inline.
fn menu_bar(ui: &mut egui::Ui, state: &mut AppState) {
    egui::menu::bar(ui, |ui| {
        ui.menu_button(rich("File"), |ui| {
            if menu_item(ui, "Manage Local Folders…", "Ctrl+Shift+L") {
                dispatch(state, Command::ManageLocalFolders);
                ui.close_menu();
            }
        });
        ui.menu_button(rich("Help"), |ui| {
            if menu_item(ui, "About Walkman Studio", "") {
                dispatch(state, Command::About);
                ui.close_menu();
            }
        });
    });
}

/// A menu row with an optional right-aligned shortcut label. Returns `true` when clicked.
fn menu_item(ui: &mut egui::Ui, label: &str, shortcut: &str) -> bool {
    let resp = ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .font(font::body())
                .color(color::SILVER_LIGHT),
        )
        .shortcut_text(if shortcut.is_empty() {
            egui::RichText::new("")
        } else {
            egui::RichText::new(shortcut)
                .font(font::caption())
                .color(color::SILVER_MUTED)
        })
        .frame(false),
    );
    resp.clicked()
}

/// A menu-title rich-text label in the toolbar's silver palette.
fn rich(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .font(font::body_medium())
        .color(color::SILVER)
}

/// The `Refresh` toolbar button (rotate glyph). Raises [`AppState::refresh_requested`].
fn refresh_button(ui: &mut egui::Ui, state: &mut AppState) {
    let resp = icon_button(ui, Icon::RotateCw, true, false)
        .on_hover_text("Refresh connected drives and reload library");
    if resp.clicked() {
        dispatch(state, Command::Refresh);
    }
}

/// The `Eject` toolbar button. Disabled when no Walkman is connected; shows a spinner in
/// place of the glyph (and stays disabled) while [`AppState::is_ejecting`].
fn eject_button(ui: &mut egui::Ui, state: &mut AppState) {
    let enabled = state.walkman_connected && !state.is_ejecting;

    if state.is_ejecting {
        // Spinner replaces the eject glyph while a Clean & Eject runs (macOS shows a
        // `ProgressView` in the toolbar slot).
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(TOOLBAR_BTN, TOOLBAR_BTN), egui::Sense::hover());
        let mut child =
            ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
            ));
        child.add(egui::Spinner::new().size(16.0).color(color::SILVER));
        return;
    }

    let resp = icon_button(ui, Icon::Eject, enabled, false);
    let resp = if enabled {
        resp.on_hover_text("Clean junk files and safely unmount Walkman")
    } else {
        resp.on_hover_text("Connect a Sony Walkman DAP to eject")
    };
    if enabled && resp.clicked() {
        dispatch(state, Command::Eject);
    }
}

/// The `Inspector` toggle button. Highlighted (gold rim) when the inspector is open.
fn inspector_button(ui: &mut egui::Ui, state: &mut AppState) {
    let resp = icon_button(ui, Icon::PanelRight, true, state.show_inspector)
        .on_hover_text("Toggle Metadata & Artwork Inspector (Ctrl+I)");
    if resp.clicked() {
        dispatch(state, Command::ToggleInspector);
    }
}

/// Draws a square, icon-only toolbar button at the 38pt baseline using the theme's glass
/// button surface + the toolbar icon glyph (or its text fallback). `active` gives the
/// pressed/selected look (used for the inspector toggle when open).
fn icon_button(ui: &mut egui::Ui, glyph: Icon, enabled: bool, active: bool) -> egui::Response {
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(TOOLBAR_BTN, TOOLBAR_BTN), sense);

    // Resolve the button paint state (active → pressed look; else hover/normal/disabled).
    let state = if !enabled {
        button::State::Disabled
    } else if active || response.is_pointer_button_down_on() {
        button::State::Pressed
    } else if response.hovered() {
        button::State::Hovered
    } else {
        button::State::Normal
    };
    // The inspector-open state reads best on the gold variant (a subtle accent); the rest
    // use the translucent glass variant to stay quiet in the chrome.
    let variant = if active {
        button::Variant::Gold
    } else {
        button::Variant::Glass
    };

    if enabled && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        let st = button::style(variant, state);
        button::paint(ui, rect, button::CORNER_RADIUS, &st);
        paint_glyph(ui, rect, glyph, enabled, active);
    }
    response
}

/// Paints a toolbar [`Icon`] centered in `rect` — the bundled glyph when the icon font is
/// present, otherwise the short text fallback (mirroring the sidebar's icon slot).
fn paint_glyph(ui: &egui::Ui, rect: egui::Rect, glyph: Icon, enabled: bool, active: bool) {
    let tint = if !enabled {
        color::SILVER_MUTED.gamma_multiply(0.4)
    } else if active {
        // Near-black ink reads on the gold active fill.
        egui::Color32::from_rgb(28, 20, 0)
    } else {
        color::SILVER_LIGHT
    };
    let label = if crate::theme::bundled_icon_font().is_some() {
        glyph.glyph_name().to_owned()
    } else {
        glyph.fallback_text().to_owned()
    };
    let galley = ui
        .painter()
        .layout_no_wrap(label, font::caption_bold(), tint);
    let pos = rect.center() - galley.size() * 0.5;
    ui.painter().galley(pos, galley, tint);
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Keyboard shortcut mapping (R1.7, R1.8) ----

    fn ctrl() -> egui::Modifiers {
        egui::Modifiers {
            ctrl: true,
            ..Default::default()
        }
    }
    fn ctrl_shift() -> egui::Modifiers {
        egui::Modifiers {
            ctrl: true,
            shift: true,
            ..Default::default()
        }
    }

    #[test]
    fn ctrl_i_toggles_inspector() {
        assert_eq!(
            match_shortcut(egui::Key::I, ctrl()),
            Some(Command::ToggleInspector)
        );
        // Ctrl+Shift+I is not the toggle (Shift changes the combo).
        assert_eq!(match_shortcut(egui::Key::I, ctrl_shift()), None);
        // Bare I with no modifier does nothing.
        assert_eq!(match_shortcut(egui::Key::I, egui::Modifiers::default()), None);
    }

    #[test]
    fn ctrl_shift_l_and_o_open_manage_local_folders() {
        assert_eq!(
            match_shortcut(egui::Key::L, ctrl_shift()),
            Some(Command::ManageLocalFolders)
        );
        assert_eq!(
            match_shortcut(egui::Key::O, ctrl_shift()),
            Some(Command::ManageLocalFolders)
        );
        // Without Shift these are not the Manage Local Folders shortcut.
        assert_eq!(match_shortcut(egui::Key::L, ctrl()), None);
        assert_eq!(match_shortcut(egui::Key::O, ctrl()), None);
    }

    #[test]
    fn unmapped_combos_return_none() {
        assert_eq!(match_shortcut(egui::Key::S, ctrl()), None);
        assert_eq!(match_shortcut(egui::Key::I, egui::Modifiers::default()), None);
        assert_eq!(match_shortcut(egui::Key::A, ctrl_shift()), None);
    }

    // ---- Command dispatch over AppState ----

    #[test]
    fn dispatch_raises_request_flags() {
        let mut state = AppState::default();

        dispatch(&mut state, Command::Refresh);
        assert!(state.refresh_requested);

        dispatch(&mut state, Command::About);
        assert!(state.about_requested);

        dispatch(&mut state, Command::ManageLocalFolders);
        assert!(state.manage_folders_requested);
    }

    #[test]
    fn dispatch_toggle_inspector_flips_flag() {
        let mut state = AppState::default();
        assert!(!state.show_inspector);
        dispatch(&mut state, Command::ToggleInspector);
        assert!(state.show_inspector);
        dispatch(&mut state, Command::ToggleInspector);
        assert!(!state.show_inspector);
    }

    #[test]
    fn eject_requires_connected_walkman_and_not_already_ejecting() {
        // Disconnected: Eject is ignored (matches `guard !drives.isEmpty`).
        let mut state = AppState::default();
        state.walkman_connected = false;
        dispatch(&mut state, Command::Eject);
        assert!(!state.eject_requested);

        // Connected + idle: Eject is requested.
        state.walkman_connected = true;
        dispatch(&mut state, Command::Eject);
        assert!(state.eject_requested);

        // Already ejecting: a second request is ignored.
        let mut busy = AppState::default();
        busy.walkman_connected = true;
        busy.is_ejecting = true;
        dispatch(&mut busy, Command::Eject);
        assert!(!busy.eject_requested);
    }

    // ---- Inspector widen-on-open resize decision (R1.8) ----

    const MIN_W: f32 = sizing::WINDOW_MIN.0; // 1320

    #[test]
    fn opening_grows_by_inspector_width_when_space_allows() {
        // 1440 window on a 2560 monitor: grows to 1440 + 300 = 1740.
        let new = plan_inspector_resize(true, 1440.0, Some(2560.0), MIN_W, None);
        assert_eq!(new, Some(1740.0));
    }

    #[test]
    fn opening_clamps_to_monitor_and_overlays_when_no_room() {
        // Window already fills the monitor: no room to grow → overlay (None).
        assert_eq!(
            plan_inspector_resize(true, 1920.0, Some(1920.0), MIN_W, None),
            None
        );
        // Partial room: clamps to the monitor width rather than exceeding it.
        let new = plan_inspector_resize(true, 1800.0, Some(1920.0), MIN_W, None);
        assert_eq!(new, Some(1920.0));
    }

    #[test]
    fn opening_with_unknown_monitor_overlays() {
        // Unknown monitor width → don't grow (overlay), never exceed the screen blindly.
        assert_eq!(
            plan_inspector_resize(true, 1440.0, None, MIN_W, None),
            None
        );
    }

    #[test]
    fn closing_restores_saved_width() {
        // Opened from 1440 (grew to 1740); closing restores the saved 1440.
        let new = plan_inspector_resize(false, 1740.0, Some(2560.0), MIN_W, Some(1440.0));
        assert_eq!(new, Some(1440.0));
    }

    #[test]
    fn closing_without_saved_width_subtracts_inspector_width() {
        // No saved width: subtract the inspector width, floored at the minimum.
        let new = plan_inspector_resize(false, 1740.0, Some(2560.0), MIN_W, None);
        assert_eq!(new, Some(1440.0));
    }

    #[test]
    fn closing_never_shrinks_below_min_width() {
        // Saved width below the minimum is clamped up to the minimum.
        let new = plan_inspector_resize(false, 1500.0, Some(2560.0), MIN_W, Some(1000.0));
        assert_eq!(new, Some(MIN_W));
        // Subtract-path also floors at the minimum.
        let new2 = plan_inspector_resize(false, MIN_W + 100.0, Some(2560.0), MIN_W, None);
        assert_eq!(new2, Some(MIN_W));
    }

    #[test]
    fn no_resize_when_target_equals_current() {
        // Closing when already at the saved width is a no-op.
        assert_eq!(
            plan_inspector_resize(false, 1440.0, Some(2560.0), MIN_W, Some(1440.0)),
            None
        );
    }

    // ---- Headless render smoke test ----

    /// Drive the toolbar renderer + the resize transition through a headless egui frame
    /// for each eject/inspector state. egui lays out and paints during `Context::run`, so
    /// a bad rect, font, or viewport command would panic here.
    #[test]
    fn toolbar_renders_in_headless_frame() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);

        for (connected, ejecting, inspector) in [
            (false, false, false),
            (true, false, false),
            (true, true, false),
            (true, false, true),
        ] {
            let mut state = AppState::default();
            state.walkman_connected = connected;
            state.is_ejecting = ejecting;
            state.show_inspector = inspector;

            let _ = ctx.run(Default::default(), |ctx| {
                render(ctx, &mut state);
                apply_inspector_resize(ctx, &mut state);
                // CentralPanel so the frame has a complete layout.
                egui::CentralPanel::default().show(ctx, |_ui| {});
            });

            // The transition is consumed: prev matches current after a frame.
            assert_eq!(state.prev_show_inspector, state.show_inspector);
        }
    }
}
