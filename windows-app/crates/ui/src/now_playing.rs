//! The always-present floating now-playing bar + the fullscreen Now Playing overlay host,
//! ported from `TrackPreviewBar` and the fullscreen presentation in
//! `Sources/WalkmanStudio/Views/TrackPreviewBar.swift` / `FullScreenNowPlayingView.swift`.
//!
//! Spec task **4.4**:
//! - an **always-present** floating bar pinned to the bottom-center of the window,
//!   constrained to [`sizing::PLAYER_MAX_WIDTH`] (1040pt) and centered (requirement 1.9);
//! - the bar shows an artwork thumbnail, title/artist, transport controls
//!   (previous / play-pause / next), a scrubber, an open-inspector button, and an
//!   open-fullscreen button (requirement 7.2);
//! - a **fullscreen overlay host**: when [`AppState::is_fullscreen_player`] is set, a
//!   viewport-covering overlay stands in for `FullScreenNowPlayingView` (task 15.2) with a
//!   collapse button that clears the flag.
//!
//! ## Placeholder player binding (task 13 / 15)
//!
//! The real player service (`player.rs`) and the player-bound `TrackPreviewBar` /
//! `FullScreenNowPlayingView` land in tasks 13 and 15. For task 4.4 the bar is driven by a
//! minimal in-`AppState` placeholder, [`NowPlayingPlaceholder`], and the transport/scrubber
//! controls only raise one-shot request flags on [`AppState`] (consumed by the player
//! wiring later) — **no audio playback happens here**. When nothing is playing the bar
//! renders an idle state (the bar is always present, per requirement 1.9).
//!
//! ## Pure logic vs. rendering
//!
//! The geometry (max-width + centering) and the time formatting are factored into
//! `Ui`/`Context`-free helpers ([`player_bar_width`], [`player_bar_rect`], [`format_time`])
//! so they unit-test without a live egui context. [`render_now_playing_bar`] and
//! [`render_fullscreen_overlay`] paint into the context and raise the request flags.

use eframe::egui;

use crate::shell::AppState;
use crate::theme::{button, color, font, glass, sizing};

/// Minimal placeholder for the track the now-playing bar displays, standing in for the
/// real `player.currentTrack` + progress until the player service lands (task 13). Only
/// the fields the bar renders are modelled here.
#[derive(Debug, Clone, PartialEq)]
pub struct NowPlayingPlaceholder {
    /// Track title (shown bold). Empty renders as `Unknown Title`, matching macOS.
    pub title: String,
    /// Track artist (shown muted after a bullet). Empty hides the artist run.
    pub artist: String,
    /// Total track duration in seconds (drives the scrubber range + the trailing time).
    pub duration_secs: f64,
    /// Current playback position in seconds (the scrubber thumb + the leading time).
    pub position_secs: f64,
    /// Whether playback is currently running (play vs. pause glyph).
    pub is_playing: bool,
    /// Whether the track is Hi-Res (shows the gold `HR` badge + tints the scrubber gold),
    /// mirroring `track.bitsPerSample > 16 || track.sampleRate > 48000`.
    pub is_hi_res: bool,
}

impl NowPlayingPlaceholder {
    /// A small demo track used by the headless smoke test and any preview. Not used at
    /// runtime (the real player populates `AppState::now_playing` in task 13).
    pub fn sample() -> Self {
        Self {
            title: "Blue Light".to_owned(),
            artist: "Mazzy Star".to_owned(),
            duration_secs: 214.0,
            position_secs: 73.0,
            is_playing: true,
            is_hi_res: true,
        }
    }
}

/// Transport / scrubber intents raised by the now-playing bar for the player to consume.
/// Kept as one-shot request flags on [`AppState`] (like the toolbar commands) so task 4.4
/// stays free of the player service; the real handlers arrive in task 13.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayerRequests {
    /// `Previous track` pressed.
    pub previous: bool,
    /// `Play/Pause` toggled.
    pub toggle_play: bool,
    /// `Next track` pressed.
    pub next: bool,
    /// A seek was requested; the target position (seconds) is stored separately.
    pub seek: bool,
}

/// Height of the floating bar (matches the macOS `frame(height: 56)` TrackPreviewBar).
pub const PLAYER_BAR_HEIGHT: f32 = 56.0;
/// Corner radius of the bar's glass capsule-ish card.
const PLAYER_BAR_RADIUS: f32 = PLAYER_BAR_HEIGHT * 0.5;
/// Horizontal margin kept between the bar and the window edges on narrow windows.
pub const PLAYER_BAR_SIDE_MARGIN: f32 = 24.0;
/// Bottom margin between the bar and the window's bottom edge.
pub const PLAYER_BAR_BOTTOM_MARGIN: f32 = 16.0;

/// The effective width of the floating now-playing bar for an `available_width` window:
/// `min(available - 2*margin, PLAYER_MAX_WIDTH)`, floored at 0.
///
/// Pure (no `Ui`), so the max-width/centering rule (requirement 1.9) is unit-tested
/// directly. On a wide window the bar is capped at [`sizing::PLAYER_MAX_WIDTH`] (1040);
/// on a narrow window it shrinks to leave [`PLAYER_BAR_SIDE_MARGIN`] on each side.
pub fn player_bar_width(available_width: f32) -> f32 {
    let usable = (available_width - PLAYER_BAR_SIDE_MARGIN * 2.0).max(0.0);
    usable.min(sizing::PLAYER_MAX_WIDTH)
}

/// The bottom-centered rect for the floating bar within `available`, constrained to
/// [`player_bar_width`] and [`PLAYER_BAR_HEIGHT`], sitting [`PLAYER_BAR_BOTTOM_MARGIN`]
/// above the bottom edge. Pure so the placement is unit-tested without a live context.
pub fn player_bar_rect(available: egui::Rect) -> egui::Rect {
    let width = player_bar_width(available.width());
    let center_x = available.center().x;
    let left = center_x - width * 0.5;
    let bottom = available.bottom() - PLAYER_BAR_BOTTOM_MARGIN;
    let top = bottom - PLAYER_BAR_HEIGHT;
    egui::Rect::from_min_max(
        egui::pos2(left, top),
        egui::pos2(left + width, bottom),
    )
}

/// Formats `seconds` as `m:ss` (or `h:mm:ss` past an hour), matching the macOS
/// `TrackScrubberRow.formatTime`. Negative / non-finite inputs render as `0:00`.
pub fn format_time(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "0:00".to_owned();
    }
    let total = seconds as u64;
    let hours = total / 3600;
    let mins = (total % 3600) / 60;
    let secs = total % 60;
    if hours > 0 {
        format!("{hours}:{mins:02}:{secs:02}")
    } else {
        format!("{mins}:{secs:02}")
    }
}

/// Renders the always-present floating now-playing bar pinned to the bottom-center of the
/// window, raising any transport/scrubber requests onto `state` (requirements 1.9, 7.2).
///
/// Call once per frame from [`crate::shell::layout_shell`] *after* the panels so the bar
/// floats above the detail content. Rendered as a top-level [`egui::Area`] (not a panel)
/// so it overlays rather than reserving layout space — the 88pt table bottom inset
/// (task 3.4) already keeps table rows clear of the bar.
///
/// When `state.now_playing` is `None` the bar shows an idle state (the bar is always
/// present per requirement 1.9).
pub fn render_now_playing_bar(ctx: &egui::Context, state: &mut AppState) {
    let screen = ctx.screen_rect();
    let rect = player_bar_rect(screen);
    if rect.width() <= 1.0 {
        return; // Window too narrow to show the bar this frame.
    }

    egui::Area::new(egui::Id::new("walkman_now_playing_bar"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            ui.set_min_size(rect.size());
            ui.set_max_size(rect.size());

            // Glass capsule chassis for the bar. Reuses the theme's liquid-glass surface
            // (the macOS bar is an ultra-thin material capsule; `liquid_glass` is the
            // closest flat-obsidian analog in the design system).
            let frame = glass::liquid_glass(glass::State::Normal)
                .rounding(egui::Rounding::same(PLAYER_BAR_RADIUS))
                .inner_margin(egui::Margin::symmetric(14.0, 7.0));
            frame.show(ui, |ui| {
                ui.set_min_size(rect.size() - egui::vec2(28.0, 14.0));
                // Top specular sheen along the bar's top edge (the design's only surface
                // gradient), matching the macOS TrackPreviewBar sheen.
                glass::paint_top_sheen(
                    ui.painter(),
                    ui.max_rect(),
                    glass::Sheen::Studio,
                    PLAYER_BAR_RADIUS,
                );

                ui.horizontal_centered(|ui| {
                    // LEFT: transport controls (previous / play-pause / next).
                    transport_controls(ui, state);

                    // CENTER: artwork + title/artist + scrubber, filling the middle.
                    ui.add_space(10.0);
                    center_display(ui, state);
                    ui.add_space(10.0);

                    // RIGHT: open-inspector + open-fullscreen, right-aligned.
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            fullscreen_button(ui, state);
                            ui.add_space(6.0);
                            inspector_button(ui, state);
                        },
                    );
                });
            });
        });
}

/// The left transport cluster: previous, play/pause, next. Each raises a one-shot request
/// flag the player wiring (task 13) consumes; there is no playback here.
fn transport_controls(ui: &mut egui::Ui, state: &mut AppState) {
    let playing = state.now_playing.as_ref().map(|t| t.is_playing).unwrap_or(false);
    // Transport is enabled only when a (placeholder) track is loaded.
    let enabled = state.now_playing.is_some();

    ui.horizontal(|ui| {
        // Previous. Glyph via the text symbol (no matching bundled `Icon` yet; the player
        // UI in task 15 maps these to the real transport glyphs).
        if glyph_button(ui, "\u{23EE}", 30.0, enabled, false).clicked() {
            // TODO(task 13): player.playPrevious()
            state.player_requests.previous = true;
        }
        ui.add_space(8.0);

        // Play / Pause (prominent circular silver button).
        let play_glyph = if playing { "\u{23F8}" } else { "\u{25B6}" };
        if play_pause_button(ui, play_glyph, enabled).clicked() {
            // TODO(task 13): player.togglePlayPause() / start the first queued track.
            state.player_requests.toggle_play = true;
        }
        ui.add_space(8.0);

        // Next.
        if glyph_button(ui, "\u{23ED}", 30.0, enabled, false).clicked() {
            // TODO(task 13): player.playNext()
            state.player_requests.next = true;
        }
    });
}

/// The center display pod: artwork thumbnail, title/artist (+ HR badge), and the scrubber.
/// Shows an idle label when nothing is playing.
fn center_display(ui: &mut egui::Ui, state: &mut AppState) {
    // Reserve the flexible middle region.
    let avail = ui.available_size();
    let pod_width = (avail.x - 2.0).max(120.0);
    ui.allocate_ui(egui::vec2(pod_width, PLAYER_BAR_HEIGHT - 14.0), |ui| {
        match state.now_playing.clone() {
            Some(track) => active_display(ui, state, &track),
            None => idle_display(ui),
        }
    });
}

/// Idle center pod — a muted waveform glyph + the app name, matching the macOS idle state.
fn idle_display(ui: &mut egui::Ui) {
    ui.centered_and_justified(|ui| {
        ui.label(
            egui::RichText::new("Walkman Studio")
                .font(font::callout_medium())
                .color(color::SILVER_MUTED),
        );
    });
}

/// The active center pod: a placeholder artwork chip (clickable → inspector), the
/// title/artist line with an optional HR badge, and the scrubber row.
fn active_display(ui: &mut egui::Ui, state: &mut AppState, track: &NowPlayingPlaceholder) {
    ui.horizontal_centered(|ui| {
        // Artwork thumbnail placeholder (real cover art lands with the player, task 13/15).
        // Clicking it opens the inspector (same affordance as the macOS artwork button).
        let art_size = egui::vec2(34.0, 34.0);
        let (art_rect, art_resp) =
            ui.allocate_exact_size(art_size, egui::Sense::click());
        if ui.is_rect_visible(art_rect) {
            ui.painter().rect_filled(
                art_rect,
                egui::Rounding::same(5.0),
                color::INPUT,
            );
            ui.painter().rect_stroke(
                art_rect,
                egui::Rounding::same(5.0),
                egui::Stroke::new(0.8, glass::white_alpha(0.12)),
            );
            // A tiny centered note glyph stands in for missing cover art.
            let galley = ui.painter().layout_no_wrap(
                "\u{266A}".to_owned(),
                font::body(),
                color::SILVER_MUTED,
            );
            ui.painter()
                .galley(art_rect.center() - galley.size() * 0.5, galley, color::SILVER_MUTED);
        }
        if art_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if art_resp.clicked() {
            state.show_inspector = true;
        }

        ui.add_space(8.0);

        ui.vertical(|ui| {
            ui.add_space(2.0);
            // Title • Artist [HR]
            ui.horizontal(|ui| {
                let title = if track.title.trim().is_empty() {
                    "Unknown Title"
                } else {
                    track.title.as_str()
                };
                ui.label(
                    egui::RichText::new(title)
                        .font(font::callout_bold())
                        .color(if track.is_playing {
                            color::SILVER_LIGHT
                        } else {
                            color::SILVER
                        }),
                );
                if !track.artist.trim().is_empty() {
                    ui.label(
                        egui::RichText::new("\u{2022}")
                            .font(font::caption())
                            .color(color::SILVER_MUTED),
                    );
                    ui.label(
                        egui::RichText::new(track.artist.as_str())
                            .font(font::caption())
                            .color(color::SILVER_MUTED),
                    );
                }
                if track.is_hi_res {
                    crate::theme::widgets::hi_res_badge(ui);
                }
            });

            // Scrubber row: leading time, slider, trailing time.
            scrubber_row(ui, state, track);
        });
    });
}

/// The scrubber: `m:ss` current, a progress slider over `0..=duration`, and `m:ss` total.
/// Dragging raises a seek request (consumed by the player in task 13); there is no audio.
fn scrubber_row(ui: &mut egui::Ui, state: &mut AppState, track: &NowPlayingPlaceholder) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format_time(track.position_secs))
                .font(font::mono_small())
                .color(color::SILVER_MUTED),
        );

        let duration = track.duration_secs.max(1.0);
        let mut pos = state
            .player_seek_target
            .unwrap_or(track.position_secs)
            .clamp(0.0, duration);
        let slider = ui.add(
            egui::Slider::new(&mut pos, 0.0..=duration)
                .show_value(false),
        );
        if slider.changed() {
            // Stash the in-progress drag target; the player consumes it on release.
            state.player_seek_target = Some(pos);
        }
        if slider.drag_stopped() || slider.lost_focus() {
            // TODO(task 13): player.seek(to: pos)
            state.player_requests.seek = true;
        }

        ui.label(
            egui::RichText::new(format_time(track.duration_secs))
                .font(font::mono_small())
                .color(color::SILVER_MUTED),
        );
    });
}

/// The open-inspector button on the bar (opens the metadata/artwork inspector). Sets
/// `show_inspector`, reusing the same flag the toolbar/Ctrl+I toggle drives.
fn inspector_button(ui: &mut egui::Ui, state: &mut AppState) {
    if glyph_button(ui, "\u{2139}", 30.0, true, state.show_inspector)
        .on_hover_text("View full metadata details & artwork")
        .clicked()
    {
        state.show_inspector = true;
    }
}

/// The open-fullscreen button (enlarge). Sets [`AppState::is_fullscreen_player`]; the real
/// fullscreen content is task 15.2.
fn fullscreen_button(ui: &mut egui::Ui, state: &mut AppState) {
    if glyph_button(ui, "\u{26F6}", 30.0, true, state.is_fullscreen_player)
        .on_hover_text("Full Screen Player (Artwork & Live Lyrics)")
        .clicked()
    {
        state.is_fullscreen_player = true;
    }
}

/// A small square glass glyph button used by the bar's transport / inspector / fullscreen
/// controls. Reuses the theme button surface (Glass variant, Gold when `active`), mirroring
/// the toolbar's icon buttons. `active` gives the pressed/accent look.
fn glyph_button(
    ui: &mut egui::Ui,
    glyph: &str,
    size: f32,
    enabled: bool,
    active: bool,
) -> egui::Response {
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), sense);

    let state = if !enabled {
        button::State::Disabled
    } else if active || response.is_pointer_button_down_on() {
        button::State::Pressed
    } else if response.hovered() {
        button::State::Hovered
    } else {
        button::State::Normal
    };
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
        button::paint(ui, rect, size * 0.5, &st);
        let tint = if !enabled {
            color::SILVER_MUTED.gamma_multiply(0.4)
        } else if active {
            egui::Color32::from_rgb(28, 20, 0)
        } else {
            color::SILVER_LIGHT
        };
        let galley = ui
            .painter()
            .layout_no_wrap(glyph.to_owned(), font::body(), tint);
        ui.painter()
            .galley(rect.center() - galley.size() * 0.5, galley, tint);
    }
    response
}

/// The prominent circular play/pause button (silver). Larger than the glyph buttons and
/// always the Prominent variant, matching the macOS `PlayPauseCircleButton`.
fn play_pause_button(ui: &mut egui::Ui, glyph: &str, enabled: bool) -> egui::Response {
    let size = 36.0;
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), sense);

    let state = if !enabled {
        button::State::Disabled
    } else if response.is_pointer_button_down_on() {
        button::State::Pressed
    } else if response.hovered() {
        button::State::Hovered
    } else {
        button::State::Normal
    };

    if enabled && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if ui.is_rect_visible(rect) {
        let st = button::style(button::Variant::Prominent, state);
        // Circular: corner radius = half the size.
        button::paint(ui, rect, size * 0.5, &st);
        // Near-black ink on the bright silver fill (matches the macOS play glyph color).
        let ink = egui::Color32::from_rgb(18, 18, 22);
        let galley = ui
            .painter()
            .layout_no_wrap(glyph.to_owned(), font::body_bold(), ink);
        ui.painter()
            .galley(rect.center() - galley.size() * 0.5, galley, ink);
    }
    response
}

/// Renders the fullscreen Now Playing overlay host when [`AppState::is_fullscreen_player`]
/// is set: a viewport-covering obsidian overlay standing in for `FullScreenNowPlayingView`
/// (task 15.2), with a collapse button that clears the flag. The real content (large
/// artwork, blurred ambient background, karaoke synced lyrics) lands in task 15.2.
///
/// Call once per frame from [`crate::shell::layout_shell`]. No-op when not in fullscreen.
pub fn render_fullscreen_overlay(ctx: &egui::Context, state: &mut AppState) {
    if !state.is_fullscreen_player {
        return;
    }
    let screen = ctx.screen_rect();

    egui::Area::new(egui::Id::new("walkman_fullscreen_player"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            ui.set_min_size(screen.size());
            ui.set_max_size(screen.size());
            // Opaque obsidian backdrop covering the whole viewport. The dynamic blurred
            // ambient background from the artwork is task 15.2.
            ui.painter().rect_filled(screen, 0.0, color::OBSIDIAN);

            // Top-right collapse button.
            let collapse_size = egui::vec2(36.0, 36.0);
            let collapse_rect = egui::Rect::from_min_size(
                egui::pos2(screen.right() - 16.0 - collapse_size.x, screen.top() + 16.0),
                collapse_size,
            );
            let mut collapse_ui = ui.new_child(
                egui::UiBuilder::new().max_rect(collapse_rect),
            );
            if glyph_button(&mut collapse_ui, "\u{2715}", 36.0, true, false)
                .on_hover_text("Collapse full screen player (Esc)")
                .clicked()
            {
                state.is_fullscreen_player = false;
            }
            // Esc also collapses, matching the macOS `onExitCommand`.
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                state.is_fullscreen_player = false;
            }

            // Centered placeholder for the real fullscreen view (task 15.2).
            let title = state
                .now_playing
                .as_ref()
                .map(|t| {
                    if t.title.trim().is_empty() {
                        "Unknown Title".to_owned()
                    } else {
                        t.title.clone()
                    }
                })
                .unwrap_or_else(|| "Nothing playing".to_owned());
            let mut center = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(screen)
                    .layout(egui::Layout::centered_and_justified(
                        egui::Direction::TopDown,
                    )),
            );
            center.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(title)
                        .font(font::title1())
                        .color(color::SILVER_LIGHT),
                );
                ui.label(
                    egui::RichText::new("Full Screen Now Playing (task 15.2)")
                        .font(font::callout())
                        .color(color::SILVER_MUTED),
                );
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Max-width + centering (R1.9) ----

    #[test]
    fn bar_width_is_capped_at_player_max_width() {
        // On a wide window the bar caps at 1040 regardless of extra width.
        assert_eq!(player_bar_width(2560.0), sizing::PLAYER_MAX_WIDTH);
        assert_eq!(player_bar_width(1440.0), sizing::PLAYER_MAX_WIDTH);
        // Exactly at max + margins: still capped at the max.
        let exact = sizing::PLAYER_MAX_WIDTH + PLAYER_BAR_SIDE_MARGIN * 2.0;
        assert_eq!(player_bar_width(exact), sizing::PLAYER_MAX_WIDTH);
    }

    #[test]
    fn bar_width_shrinks_with_side_margins_on_narrow_windows() {
        // Narrower than max: the bar leaves a side margin on each edge.
        let w = 800.0;
        assert_eq!(player_bar_width(w), w - PLAYER_BAR_SIDE_MARGIN * 2.0);
        // Degenerate widths never go negative.
        assert_eq!(player_bar_width(0.0), 0.0);
        assert_eq!(player_bar_width(10.0), 0.0);
    }

    #[test]
    fn bar_rect_is_bottom_centered_with_max_width() {
        // A 1440-wide, 880-tall window: the bar is 1040 wide, horizontally centered, and
        // sits one bottom-margin above the bottom edge.
        let screen = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1440.0, 880.0));
        let rect = player_bar_rect(screen);

        assert_eq!(rect.width(), sizing::PLAYER_MAX_WIDTH);
        assert_eq!(rect.height(), PLAYER_BAR_HEIGHT);
        // Centered: equal gap on both sides.
        assert!((rect.left() - (1440.0 - sizing::PLAYER_MAX_WIDTH) / 2.0).abs() < 0.01);
        assert!((rect.center().x - screen.center().x).abs() < 0.01);
        // Bottom-pinned.
        assert!((rect.bottom() - (880.0 - PLAYER_BAR_BOTTOM_MARGIN)).abs() < 0.01);
    }

    #[test]
    fn bar_rect_is_centered_within_offset_screen() {
        // A screen not anchored at the origin stays centered around its own center.
        let screen = egui::Rect::from_min_max(egui::pos2(260.0, 50.0), egui::pos2(1700.0, 930.0));
        let rect = player_bar_rect(screen);
        assert!((rect.center().x - screen.center().x).abs() < 0.01);
    }

    // ---- Time formatting (R7.2 scrubber) ----

    #[test]
    fn format_time_minutes_and_seconds() {
        assert_eq!(format_time(0.0), "0:00");
        assert_eq!(format_time(5.0), "0:05");
        assert_eq!(format_time(65.0), "1:05");
        assert_eq!(format_time(214.0), "3:34");
        assert_eq!(format_time(599.0), "9:59");
    }

    #[test]
    fn format_time_hours() {
        assert_eq!(format_time(3600.0), "1:00:00");
        assert_eq!(format_time(3661.0), "1:01:01");
        assert_eq!(format_time(7265.0), "2:01:05");
    }

    #[test]
    fn format_time_handles_invalid_input() {
        assert_eq!(format_time(-1.0), "0:00");
        assert_eq!(format_time(f64::NAN), "0:00");
        assert_eq!(format_time(f64::INFINITY), "0:00");
    }

    // ---- Headless render smoke test (R1.9, R7.2) ----

    /// Drive the now-playing bar + the fullscreen overlay through a headless egui frame for
    /// the idle / playing / fullscreen combinations. egui lays out and paints during
    /// `Context::run`, so a bad rect, font, or widget would panic here.
    #[test]
    fn now_playing_and_fullscreen_render_in_headless_frame() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);

        let cases = [
            // (now_playing, is_fullscreen_player)
            (None, false),
            (Some(NowPlayingPlaceholder::sample()), false),
            (None, true),
            (Some(NowPlayingPlaceholder::sample()), true),
        ];

        for (track, fullscreen) in cases {
            let mut state = AppState::default();
            state.now_playing = track;
            state.is_fullscreen_player = fullscreen;

            let _ = ctx.run(Default::default(), |ctx| {
                // A CentralPanel gives the frame a complete base layout, then the bar +
                // overlay float on top (exactly how the shell composes them).
                egui::CentralPanel::default().show(ctx, |_ui| {});
                render_now_playing_bar(ctx, &mut state);
                render_fullscreen_overlay(ctx, &mut state);
            });
        }
    }
}
