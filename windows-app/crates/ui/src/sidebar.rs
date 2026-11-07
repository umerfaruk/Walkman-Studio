//! The sidebar section list, ported from `SidebarView`
//! (`Sources/WalkmanStudio/Views/SidebarView.swift`).
//!
//! Spec task **4.2**: render the `NavTab` sidebar sections in the exact order/labels of
//! requirement 1.4, with their visibility rules — the Device-section modes for 0 / 1 /
//! >1 attached devices, the Walkman Library group shown only when a Walkman is connected,
//! and `Backup & Restore` disabled (greyed, with a tooltip) when disconnected — plus the
//! per-section count badges (requirement 1.5 shares each row's title + an equivalent icon
//! with its macOS SF Symbol), and the bottom `About Walkman Studio` button.
//!
//! ## Pure model vs. rendering
//!
//! The *decision* logic (which Device-section header to show, whether the Walkman Library
//! group is visible, whether `Backup & Restore` is enabled, and what each badge reads) is
//! factored into the pure, `Ui`-free [`SidebarModel`] / [`DeviceSectionMode`] types so it
//! can be unit-tested without a live egui context. [`render`] walks that model and paints
//! each row, reusing the theme's icon mapping ([`theme::icon`]) and the counter-pill /
//! Hi-Res badge widgets ([`theme::widgets`]). Nothing here reinvents a widget the theme
//! already provides.
//!
//! ## Placeholder state fields
//!
//! The counts, device count, and drive labels are driven by simple scalar fields on
//! [`AppState`](crate::shell::AppState) (added by this task). Richer domain types
//! (`WalkmanDevice`, `WalkmanDrive`, `Track`, the precomputed caches) arrive with the
//! model/service ports (tasks 5 and 9); until then these scalars stand in so the sidebar's
//! visibility rules and badges are fully exercised.

use eframe::egui;

use crate::shell::AppState;
use crate::theme::{color, font, widgets};
use crate::theme::icon::{self, Icon};
use crate::NavTab;

/// Which Device-section header/content to render, selected purely from the attached
/// device count (requirement 1.4 — the Device section has three modes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceSectionMode {
    /// No Walkman attached: header `Device`, a single "disconnected" row.
    Disconnected,
    /// Exactly one Walkman: header `Device`, the connected device card + one row per drive.
    SingleDevice,
    /// More than one Walkman: header `Connected Devices (n)`, device cards + drive rows.
    MultiDevice(usize),
}

impl DeviceSectionMode {
    /// Select the mode from `(connected, device_count)`.
    ///
    /// Mirrors the `if !isWalkmanConnected { … } else if devices.count > 1 { … } else { … }`
    /// branch in `SidebarView.body`: disconnected wins first, then >1 devices, otherwise a
    /// single device. A `device_count` of 0 while "connected" still reads as a single
    /// device (the Swift fallback path), so visibility never shows an empty device group.
    pub fn select(connected: bool, device_count: usize) -> Self {
        if !connected {
            DeviceSectionMode::Disconnected
        } else if device_count > 1 {
            DeviceSectionMode::MultiDevice(device_count)
        } else {
            DeviceSectionMode::SingleDevice
        }
    }

    /// The section header label for this mode (`Device` or `Connected Devices (n)`).
    pub fn header(self) -> String {
        match self {
            DeviceSectionMode::Disconnected | DeviceSectionMode::SingleDevice => {
                "Device".to_owned()
            }
            DeviceSectionMode::MultiDevice(n) => format!("Connected Devices ({n})"),
        }
    }
}

/// A single drive row in the Device section (`Internal` / `MicroSD`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveRow {
    /// The drive's display name (e.g. `"Internal Storage"`, `"microSD Card"`).
    pub name: String,
    /// Whether this is the active library drive (gets the active beacon).
    pub is_current: bool,
    /// Whether the drive belongs to the SD card slot (selects the SD icon vs. the
    /// internal-drive icon).
    pub is_sd: bool,
}

/// The pure, render-free description of the sidebar for a given [`AppState`]. [`render`]
/// consumes this; the unit tests assert against it without a live `Ui`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarModel {
    /// The Device-section mode (header + which rows to draw).
    pub device_mode: DeviceSectionMode,
    /// Whether the `Walkman Library` group is shown (only when a Walkman is connected).
    pub show_walkman_library: bool,
    /// Whether `Backup & Restore` is enabled (disabled/greyed when disconnected).
    pub backup_restore_enabled: bool,
}

impl SidebarModel {
    /// Derive the model from app state.
    pub fn from_state(state: &AppState) -> Self {
        let connected = state.walkman_connected;
        Self {
            device_mode: DeviceSectionMode::select(connected, state.device_count),
            // Both the Walkman Library group and Backup & Restore follow the single
            // `isWalkmanConnected` flag in `SidebarView`.
            show_walkman_library: connected,
            backup_restore_enabled: connected,
        }
    }
}

/// The tooltip shown on `Backup & Restore`, matching the macOS `.help(…)` text.
pub const BACKUP_ENABLED_TOOLTIP: &str = "Backup and restore Walkman flash storage";
pub const BACKUP_DISABLED_TOOLTIP: &str =
    "Backup & Restore requires a connected Sony Walkman DAP";

/// The `About Walkman Studio` bottom button label (macOS `AboutSidebarButton`).
pub const ABOUT_LABEL: &str = "About Walkman Studio";

/// Format a count into the badge string for `tab`, or `None` when the row shows no badge.
///
/// Mirrors the `badge:` arguments in `SidebarView`: a count renders only when `> 0`, and
/// `SenseMe™ Channels` / `Playlists` never show a numeric badge in the source.
pub fn badge_for(tab: NavTab, state: &AppState) -> Option<String> {
    let non_zero = |n: usize| (n > 0).then(|| thousands(n));
    match tab {
        NavTab::LocalLibrary => non_zero(state.local_track_count),
        NavTab::Library => non_zero(state.track_count),
        NavTab::RecentlyAdded => non_zero(state.recently_added_count),
        NavTab::Albums => non_zero(state.album_count),
        NavTab::Artists => non_zero(state.artist_count),
        NavTab::HiRes => non_zero(state.hi_res_count),
        NavTab::LanguageStudy => non_zero(state.language_study_count),
        NavTab::Duplicates => non_zero(state.duplicate_count),
        // The queue counter is small and shown without thousands grouping in the source.
        NavTab::Transfer => (state.queue_count > 0).then(|| state.queue_count.to_string()),
        // No numeric badge on these rows.
        NavTab::SenseMe | NavTab::Playlists | NavTab::BackupRestore | NavTab::Overview => None,
    }
}

/// Group thousands with a comma, matching Swift's `Int.formatted()` in an en-US locale
/// (e.g. `1234` → `"1,234"`). Kept dependency-free since the counts are plain `usize`.
fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let bytes = digits.as_bytes();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    let len = bytes.len();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}

/// Resolve the bundled [`Icon`] for a `NavTab` via the theme's SF-Symbol→glyph mapping
/// (task 3.4). Every `NavTab` symbol is in the sidebar table, so this is always `Some`.
fn tab_icon(tab: NavTab) -> Option<Icon> {
    icon::sidebar_icon(tab.sf_symbol())
}

// ============================ Rendering ============================

/// Vertical row inset between sidebar rows (macOS `listRowInsets` top/bottom 3.5).
const ROW_SPACING: f32 = 3.5;
/// Row corner radius (macOS `RoundedRectangle(cornerRadius: 8)`).
const ROW_RADIUS: f32 = 8.0;
/// Horizontal padding inside a row.
const ROW_PAD_X: f32 = 9.0;
/// Width reserved for the leading icon slot.
const ICON_SLOT: f32 = 20.0;

/// Renders the full sidebar (section groups, visibility rules, badges, About button) for
/// `state` into `ui`. Clicking a selectable row sets [`AppState::nav_tab`]; clicking the
/// About button sets [`AppState::about_requested`] (wired to the real About sheet in task
/// 4.3). The disabled `Backup & Restore` row ignores clicks and shows its tooltip.
pub fn render(ui: &mut egui::Ui, state: &mut AppState) {
    let model = SidebarModel::from_state(state);

    ui.spacing_mut().item_spacing.y = ROW_SPACING;

    // The About button is pinned to the bottom; the section list fills the rest and
    // scrolls if it overflows.
    egui::TopBottomPanel::bottom("walkman_sidebar_about")
        .frame(egui::Frame::none().fill(color::BACKGROUND))
        .show_inside(ui, |ui| {
            // 1px top divider (macOS `Divider().overlay(white @ 6%)`).
            let full = ui.max_rect();
            let divider = egui::Rect::from_min_max(
                full.min,
                egui::pos2(full.max.x, full.min.y + 1.0),
            );
            ui.painter().rect_filled(divider, 0.0, color::BORDER);
            ui.add_space(8.0);
            about_button(ui, state);
            ui.add_space(8.0);
        });

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(8.0);

            // --- Device section (modes: 0 / 1 / >1) ---
            section_header(ui, &model.device_mode.header());
            device_section(ui, state, model.device_mode);

            // --- Local Music Library ---
            section_header(ui, "Local Music Library");
            nav_row(ui, state, NavTab::LocalLibrary, "Local Library", true);
            nav_row(ui, state, NavTab::SenseMe, "SenseMe\u{2122} Channels", true);

            // --- Walkman Library (only when connected) ---
            if model.show_walkman_library {
                section_header(ui, "Walkman Library");
                nav_row(ui, state, NavTab::Library, "All Songs", true);
                nav_row(ui, state, NavTab::RecentlyAdded, "Recently Added", true);
                nav_row(ui, state, NavTab::Albums, "Albums", true);
                nav_row(ui, state, NavTab::Artists, "Artists", true);
                nav_row(ui, state, NavTab::HiRes, "Hi-Res Audio", true);
                nav_row(ui, state, NavTab::LanguageStudy, "Language Study", true);
                nav_row(ui, state, NavTab::Playlists, "Playlists", true);
            }

            // --- Tools ---
            section_header(ui, "Tools");
            nav_row(ui, state, NavTab::Duplicates, "Duplicates", true);
            nav_row(ui, state, NavTab::Transfer, "Transfer & Convert", true);
            // Backup & Restore is greyed + ignores clicks when no Walkman is connected.
            nav_row(
                ui,
                state,
                NavTab::BackupRestore,
                "Backup & Restore",
                model.backup_restore_enabled,
            );

            ui.add_space(4.0);
        });
}

/// A dim, semibold uppercase-less section header (macOS `sidebarSectionHeader`).
fn section_header(ui: &mut egui::Ui, title: &str) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(title)
                .font(font::section_header())
                .color(color::SILVER_MUTED.gamma_multiply(0.82)),
        );
    });
    ui.add_space(2.0);
}

/// Draws the Device section's content for `mode`: the disconnected tile, the connected
/// device card, or the multi-device cards — each followed by one row per drive.
fn device_section(ui: &mut egui::Ui, state: &mut AppState, mode: DeviceSectionMode) {
    match mode {
        DeviceSectionMode::Disconnected => {
            device_tile(ui, state, Icon::Cable, "Sony Walkman", "Not Connected");
        }
        DeviceSectionMode::SingleDevice => {
            device_tile(ui, state, Icon::Speaker, "Sony Walkman", "Device Overview");
            for drive in &state.drives.clone() {
                drive_row(ui, drive);
            }
        }
        DeviceSectionMode::MultiDevice(_) => {
            // One card per device (labelled generically until `WalkmanDevice` lands in
            // task 5), each followed by its drive rows. The placeholder drive list is
            // shared across devices for now.
            for i in 0..state.device_count {
                device_tile(
                    ui,
                    state,
                    Icon::Speaker,
                    &format!("Sony Walkman {}", i + 1),
                    "Device Overview",
                );
            }
            for drive in &state.drives.clone() {
                drive_row(ui, drive);
            }
        }
    }
}

/// A device/overview tile. Clicking selects `NavTab::Overview`. (The rich device card —
/// model name, storage, active beacon — arrives with `WalkmanDevice` in task 5/9; this is
/// the glyph + two-line label skeleton.)
fn device_tile(
    ui: &mut egui::Ui,
    state: &mut AppState,
    glyph: Icon,
    title: &str,
    subtitle: &str,
) {
    let selected = state.nav_tab == NavTab::Overview;
    let resp = row_frame(ui, selected, true, |ui| {
        icon_slot(ui, glyph, selected, false);
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(title)
                    .font(font::body_bold())
                    .color(if selected { color::SILVER_LIGHT } else { color::SILVER }),
            );
            ui.label(
                egui::RichText::new(subtitle)
                    .font(font::caption())
                    .color(color::SILVER_MUTED),
            );
        });
    });
    if resp.clicked() {
        state.nav_tab = NavTab::Overview;
    }
}

/// A drive row (Internal / microSD) under the Device section.
fn drive_row(ui: &mut egui::Ui, drive: &DriveRow) {
    let glyph = if drive.is_sd { Icon::SdCard } else { Icon::HardDrive };
    row_frame(ui, drive.is_current, true, |ui| {
        icon_slot(ui, glyph, drive.is_current, false);
        ui.label(
            egui::RichText::new(&drive.name)
                .font(font::body_medium())
                .color(if drive.is_current { color::SILVER_LIGHT } else { color::SILVER }),
        );
        if drive.is_current {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Active beacon dot (clean tactile gold dot, no glow).
                let (rect, _) = ui.allocate_exact_size(egui::vec2(6.0, 6.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 3.0, color::HIRES_GOLD);
            });
        }
    });
}

/// A selectable navigation row. `enabled == false` greys the row, ignores clicks, and
/// shows the disabled tooltip (used for `Backup & Restore` when disconnected).
fn nav_row(ui: &mut egui::Ui, state: &mut AppState, tab: NavTab, title: &str, enabled: bool) {
    let selected = state.nav_tab == tab;
    let glyph = tab_icon(tab);
    let badge = badge_for(tab, state);
    let is_hi_res = tab == NavTab::HiRes;

    let resp = row_frame(ui, selected && enabled, enabled, |ui| {
        if let Some(g) = glyph {
            icon_slot(ui, g, selected, !enabled);
        } else {
            ui.add_space(ICON_SLOT);
        }

        let title_color = if !enabled {
            color::SILVER_MUTED.gamma_multiply(0.4)
        } else if selected {
            color::SILVER_LIGHT
        } else {
            color::SILVER
        };
        ui.label(
            egui::RichText::new(title)
                .font(if selected { font::body_bold() } else { font::body() })
                .color(title_color),
        );

        if let Some(b) = badge {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if is_hi_res {
                    hi_res_badge_pill(ui, &b);
                } else {
                    counter_pill(ui, &b, selected);
                }
            });
        }
    });

    // Tooltip: enabled rows get the Backup help text only on that row (matching macOS,
    // where only Backup & Restore carries a `.help`); disabled Backup shows its reason.
    if tab == NavTab::BackupRestore {
        let tip = if enabled { BACKUP_ENABLED_TOOLTIP } else { BACKUP_DISABLED_TOOLTIP };
        resp.clone().on_hover_text(tip);
    }

    if enabled && resp.clicked() {
        state.nav_tab = tab;
    }
}

/// Lays out a row frame (selection/hover background + rim) and runs `add_contents`
/// inside it. Returns the whole-row [`Response`] (click/hover) for selectable rows.
///
/// The row is laid out in a child [`egui::Ui`] so we know its exact rect, then the
/// selection/hover plate + rim are painted *behind* the content (a background shape
/// reserved before the content, moved to the correct z-order after measuring). This keeps
/// the flat-obsidian look: depth comes only from a crisp 1px white rim, with no glow.
fn row_frame(
    ui: &mut egui::Ui,
    selected: bool,
    enabled: bool,
    add_contents: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let margin = egui::vec2(ROW_PAD_X, 7.0);

    // Reserve a paint index so the background sits behind the content once we know the rect.
    let bg = ui.painter().add(egui::Shape::Noop);
    let rim = ui.painter().add(egui::Shape::Noop);

    let full_width = ui.available_width();
    let inner = egui::Frame::none()
        .inner_margin(egui::Margin::symmetric(margin.x, margin.y))
        .show(ui, |ui| {
            ui.set_width(full_width - margin.x * 2.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 9.0;
                add_contents(ui);
            });
        });

    let rect = inner.response.rect;
    let response = ui.interact(rect, response_id(ui, rect), sense);
    let hovered = enabled && response.hovered();

    // Background + rim reflecting state.
    if selected {
        ui.painter().set(
            bg,
            egui::Shape::rect_filled(rect, ROW_RADIUS, crate::theme::glass::white_alpha(0.10)),
        );
        ui.painter().set(
            rim,
            egui::Shape::rect_stroke(
                rect,
                ROW_RADIUS,
                egui::Stroke::new(1.0, crate::theme::glass::white_alpha(0.40)),
            ),
        );
    } else if hovered {
        ui.painter().set(
            bg,
            egui::Shape::rect_filled(rect, ROW_RADIUS, crate::theme::glass::white_alpha(0.045)),
        );
    }

    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    response
}

/// A stable-ish interaction id for a row at `rect` (rows are positional, so key off the
/// top-left corner within this sidebar frame).
fn response_id(ui: &egui::Ui, rect: egui::Rect) -> egui::Id {
    ui.id().with((rect.min.x.to_bits(), rect.min.y.to_bits()))
}

/// Draws the leading icon glyph (or its text fallback when no glyph font is bundled) in a
/// fixed [`ICON_SLOT`]-wide slot, tinted for selection / disabled / Hi-Res gold.
fn icon_slot(ui: &mut egui::Ui, glyph: Icon, selected: bool, disabled: bool) {
    let tint = if disabled {
        color::SILVER_MUTED.gamma_multiply(0.4)
    } else if glyph == Icon::Waveform {
        // Hi-Res gold is reserved for the Hi-Res waveform accent.
        if selected { color::HIRES_GOLD_LIGHT } else { color::HIRES_GOLD }
    } else if selected {
        color::SILVER_LIGHT
    } else {
        color::SILVER_MUTED
    };

    // No bundled glyph font yet (task 19.1): render the short text fallback so the row is
    // still legible. Once the Lucide font is vendored a glyph lookup replaces this.
    let label = if crate::theme::bundled_icon_font().is_some() {
        glyph.glyph_name().to_owned()
    } else {
        glyph.fallback_text().to_owned()
    };

    let (rect, _) = ui.allocate_exact_size(egui::vec2(ICON_SLOT, 18.0), egui::Sense::hover());
    let galley = ui
        .painter()
        .layout_no_wrap(label, font::caption_bold(), tint);
    let pos = rect.center() - galley.size() * 0.5;
    ui.painter().galley(pos, galley, tint);
}

/// A translucent counter pill (`LiquidGlassActivePill`/counter), reusing the theme pill
/// widget. Brighter when the row is selected.
fn counter_pill(ui: &mut egui::Ui, text: &str, selected: bool) {
    let f = font::pill_counter();
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), f.clone(), color::SILVER_LIGHT);
    let size = galley.size() + egui::vec2(14.0, 5.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    widgets::paint_pill(
        ui.painter(),
        rect,
        widgets::Pill::Active { hi_res: false, hovered: selected },
    );
    let pos = rect.center() - galley.size() * 0.5;
    ui.painter().galley(pos, galley, color::SILVER_LIGHT);
}

/// The Hi-Res gold counter badge (`waveform` glyph + count), reusing the theme HR badge
/// look via the gold [`widgets::Pill`] plus the Hi-Res waveform accent.
fn hi_res_badge_pill(ui: &mut egui::Ui, text: &str) {
    let f = font::pill_counter();
    let ink = egui::Color32::from_rgb(22, 20, 18);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), f.clone(), ink);
    let size = galley.size() + egui::vec2(16.0, 5.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    widgets::paint_pill(
        ui.painter(),
        rect,
        widgets::Pill::Active { hi_res: true, hovered: false },
    );
    // Gold fill tint under the ink for the Hi-Res accent.
    ui.painter()
        .rect_filled(rect, rect.height() * 0.5, color::HIRES_GOLD.gamma_multiply(0.85));
    let pos = rect.center() - galley.size() * 0.5;
    ui.painter().galley(pos, galley, ink);
}

/// The bottom `About Walkman Studio` capsule button. Clicking sets
/// [`AppState::about_requested`]; the real About sheet + menu command land in task 4.3.
fn about_button(ui: &mut egui::Ui, state: &mut AppState) {
    let resp = row_frame(ui, false, true, |ui| {
        icon_slot(ui, Icon::Info, false, false);
        ui.label(
            egui::RichText::new(ABOUT_LABEL)
                .font(font::body_medium())
                .color(color::SILVER_LIGHT),
        );
    });
    if resp.clicked() {
        // TODO(task 4.3): open the About sheet via the toolbar/menu command wiring.
        state.about_requested = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Device-section mode selection (0 / 1 / >1) ----

    #[test]
    fn device_mode_disconnected_when_not_connected() {
        // Disconnected wins even if a stale device count is present.
        assert_eq!(
            DeviceSectionMode::select(false, 0),
            DeviceSectionMode::Disconnected
        );
        assert_eq!(
            DeviceSectionMode::select(false, 3),
            DeviceSectionMode::Disconnected
        );
        assert_eq!(DeviceSectionMode::Disconnected.header(), "Device");
    }

    #[test]
    fn device_mode_single_for_zero_or_one_connected() {
        // Connected with 0 (fallback) or 1 device → single-device "Device" header.
        assert_eq!(
            DeviceSectionMode::select(true, 0),
            DeviceSectionMode::SingleDevice
        );
        assert_eq!(
            DeviceSectionMode::select(true, 1),
            DeviceSectionMode::SingleDevice
        );
        assert_eq!(DeviceSectionMode::SingleDevice.header(), "Device");
    }

    #[test]
    fn device_mode_multi_for_more_than_one() {
        assert_eq!(
            DeviceSectionMode::select(true, 2),
            DeviceSectionMode::MultiDevice(2)
        );
        assert_eq!(
            DeviceSectionMode::MultiDevice(2).header(),
            "Connected Devices (2)"
        );
        assert_eq!(
            DeviceSectionMode::MultiDevice(5).header(),
            "Connected Devices (5)"
        );
    }

    // ---- Walkman Library + Backup & Restore visibility ----

    #[test]
    fn walkman_library_hidden_and_backup_disabled_when_disconnected() {
        let mut state = AppState::default();
        state.walkman_connected = false;
        let model = SidebarModel::from_state(&state);
        assert!(!model.show_walkman_library);
        assert!(!model.backup_restore_enabled);
        assert_eq!(model.device_mode, DeviceSectionMode::Disconnected);
    }

    #[test]
    fn walkman_library_shown_and_backup_enabled_when_connected() {
        let mut state = AppState::default();
        state.walkman_connected = true;
        state.device_count = 1;
        let model = SidebarModel::from_state(&state);
        assert!(model.show_walkman_library);
        assert!(model.backup_restore_enabled);
        assert_eq!(model.device_mode, DeviceSectionMode::SingleDevice);
    }

    #[test]
    fn multi_device_model_from_state() {
        let mut state = AppState::default();
        state.walkman_connected = true;
        state.device_count = 3;
        let model = SidebarModel::from_state(&state);
        assert_eq!(model.device_mode, DeviceSectionMode::MultiDevice(3));
        assert!(model.show_walkman_library);
    }

    // ---- Count badges ----

    #[test]
    fn badges_hidden_when_counts_zero() {
        let state = AppState::default();
        for tab in NavTab::ALL {
            assert_eq!(badge_for(tab, &state), None, "{tab:?} should have no badge at zero");
        }
    }

    #[test]
    fn badges_render_nonzero_counts_with_grouping() {
        let mut state = AppState::default();
        state.local_track_count = 1234;
        state.track_count = 50;
        state.recently_added_count = 7;
        state.album_count = 120;
        state.artist_count = 42;
        state.hi_res_count = 9;
        state.language_study_count = 3;
        state.duplicate_count = 2;
        state.queue_count = 5;

        assert_eq!(badge_for(NavTab::LocalLibrary, &state).as_deref(), Some("1,234"));
        assert_eq!(badge_for(NavTab::Library, &state).as_deref(), Some("50"));
        assert_eq!(badge_for(NavTab::RecentlyAdded, &state).as_deref(), Some("7"));
        assert_eq!(badge_for(NavTab::Albums, &state).as_deref(), Some("120"));
        assert_eq!(badge_for(NavTab::Artists, &state).as_deref(), Some("42"));
        assert_eq!(badge_for(NavTab::HiRes, &state).as_deref(), Some("9"));
        assert_eq!(badge_for(NavTab::LanguageStudy, &state).as_deref(), Some("3"));
        assert_eq!(badge_for(NavTab::Duplicates, &state).as_deref(), Some("2"));
        // The transfer queue counter is shown ungrouped.
        assert_eq!(badge_for(NavTab::Transfer, &state).as_deref(), Some("5"));
    }

    #[test]
    fn rows_without_numeric_badges_stay_none() {
        let mut state = AppState::default();
        // Even with large counts elsewhere, these rows never carry a numeric badge.
        state.local_track_count = 9999;
        assert_eq!(badge_for(NavTab::SenseMe, &state), None);
        assert_eq!(badge_for(NavTab::Playlists, &state), None);
        assert_eq!(badge_for(NavTab::BackupRestore, &state), None);
        assert_eq!(badge_for(NavTab::Overview, &state), None);
    }

    #[test]
    fn thousands_grouping_matches_en_us() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(42), "42");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(1234), "1,234");
        assert_eq!(thousands(1234567), "1,234,567");
    }

    // ---- Icon association (R1.5: equivalent icon per row) ----

    #[test]
    fn every_nav_tab_resolves_to_a_bundled_icon() {
        for tab in NavTab::ALL {
            assert!(
                tab_icon(tab).is_some(),
                "{tab:?} ({}) has no bundled sidebar glyph",
                tab.sf_symbol()
            );
        }
    }

    #[test]
    fn hi_res_row_maps_to_the_search_waveform_icon() {
        // The Hi-Res Audio row uses the waveform+magnifier symbol (R1.5 parity).
        assert_eq!(tab_icon(NavTab::HiRes), Some(Icon::Search));
        assert_eq!(tab_icon(NavTab::Duplicates), Some(Icon::Copy));
        assert_eq!(tab_icon(NavTab::Overview), Some(Icon::Speaker));
    }

    // ---- Tooltips / labels ----

    #[test]
    fn backup_tooltips_and_about_label_match_macos() {
        assert_eq!(BACKUP_ENABLED_TOOLTIP, "Backup and restore Walkman flash storage");
        assert_eq!(
            BACKUP_DISABLED_TOOLTIP,
            "Backup & Restore requires a connected Sony Walkman DAP"
        );
        assert_eq!(ABOUT_LABEL, "About Walkman Studio");
    }

    // ---- Headless rendering ----

    /// Drive the full sidebar renderer through every visibility mode in a headless egui
    /// frame. egui lays out and paints during `Context::run`, so an out-of-range rect, an
    /// unknown font family, or a bad shape index would panic here. This verifies the
    /// renderer walks each branch (disconnected / single / multi, with and without the
    /// Walkman Library group and the greyed Backup row) without a live window.
    #[test]
    fn sidebar_renders_in_headless_frame_for_every_mode() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);

        for (connected, device_count) in [(false, 0usize), (true, 1), (true, 3)] {
            let mut state = AppState::default();
            state.walkman_connected = connected;
            state.device_count = device_count;
            if connected {
                state.drives = vec![
                    DriveRow { name: "Internal Storage".into(), is_current: true, is_sd: false },
                    DriveRow { name: "microSD Card".into(), is_current: false, is_sd: true },
                ];
            }
            // Non-zero counts so every badge path (incl. the gold Hi-Res pill) paints.
            state.local_track_count = 1234;
            state.track_count = 50;
            state.recently_added_count = 7;
            state.album_count = 120;
            state.artist_count = 42;
            state.hi_res_count = 9;
            state.language_study_count = 3;
            state.duplicate_count = 2;
            state.queue_count = 5;

            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render(ui, &mut state);
                });
            });
        }
    }
}
