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
//! ## Player binding (task 13 / 15)
//!
//! The bar is still driven by the [`NowPlayingPlaceholder`] shape introduced in task 4.4,
//! but [`sync_now_playing_from_player`] now refreshes it from the real `walkman_services`
//! [`walkman_services::player::Player`] (task 13) each frame when [`AppState::player`] is
//! bound, and the transport/scrubber handlers call the real player methods directly. The
//! one-shot [`PlayerRequests`] flags are still *also* raised alongside those calls so the
//! existing flag-based tests keep passing; [`AppState::player`] stays `None` (and the bar
//! falls back to flags only) until task 18 constructs a `Player` and wires it in. When
//! nothing is playing the bar renders an idle state (the bar is always present, per
//! requirement 1.9).
//!
//! ## Pure logic vs. rendering
//!
//! The geometry (max-width + centering) and the time formatting are factored into
//! `Ui`/`Context`-free helpers ([`player_bar_width`], [`player_bar_rect`], [`format_time`])
//! so they unit-test without a live egui context. [`render_now_playing_bar`] and
//! [`render_fullscreen_overlay`] paint into the context and raise the request flags.

use eframe::egui;

use walkman_services::player::PlaybackState;
use walkman_services::walkman_engine::active_lyric_line_index;

use crate::shell::AppState;
use crate::theme::{button, color, font, glass, gradient, sizing};

/// Refreshes [`AppState::now_playing`] from the real player (task 13) when one is bound
/// (task 18 constructs it), and lets the player auto-advance (R7.5) by polling it once per
/// frame. A no-op while `state.player` is `None`, so the task 4.4 placeholder keeps working
/// until the real player is wired in.
///
/// Call once per frame, before the bar/overlay render (see [`crate::shell::layout_shell`]).
pub fn sync_now_playing_from_player(state: &mut AppState) {
    let Some(player) = state.player.clone() else { return };
    player.poll();
    let status = player.status();
    let track = status.current_index.and_then(|i| player.queue().get(i).cloned());
    state.now_playing = track.map(|t| NowPlayingPlaceholder {
        title: t.title,
        artist: t.artist,
        duration_secs: status.duration_seconds,
        position_secs: status.position_seconds,
        is_playing: status.state == PlaybackState::Playing,
        is_hi_res: t.bits_per_sample > 16 || t.sample_rate > 48_000,
    });
}

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
            if let Some(player) = &state.player {
                let _ = player.previous();
            }
            state.player_requests.previous = true;
        }
        ui.add_space(8.0);

        // Play / Pause (prominent circular silver button).
        let play_glyph = if playing { "\u{23F8}" } else { "\u{25B6}" };
        if play_pause_button(ui, play_glyph, enabled).clicked() {
            if let Some(player) = &state.player {
                player.toggle_play_pause();
            }
            state.player_requests.toggle_play = true;
        }
        ui.add_space(8.0);

        // Next.
        if glyph_button(ui, "\u{23ED}", 30.0, enabled, false).clicked() {
            if let Some(player) = &state.player {
                let _ = player.next();
            }
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
            if let Some(player) = &state.player {
                let _ = player.seek(pos);
            }
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

/// Renders the fullscreen Now Playing overlay (`FullScreenNowPlayingView`, task 15.2) when
/// [`AppState::is_fullscreen_player`] is set: a viewport-covering overlay with a tinted
/// ambient background, large artwork, title/artist, real transport controls, and
/// karaoke-style synced lyrics (requirement 7.3), plus a collapse button that clears the
/// flag.
///
/// ## Honest simplifications
/// - **Artwork**: still a placeholder tile (a note glyph over [`color::INPUT`]) — no cover
///   art texture pipeline exists yet; real artwork lands with the cache wiring in task 18.
/// - **Ambient background**: egui's 2D immediate-mode painter has no blur filter, so the
///   macOS "blurred artwork" backdrop is approximated here as a flat top-to-bottom tint
///   wash ([`gradient::Direction::TopToBottom`]) over the obsidian base, using the theme's
///   existing gradient tokens rather than sampling the (placeholder) artwork's dominant
///   color.
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

            // Obsidian base + a subtle top-to-bottom tint wash standing in for the blurred
            // ambient artwork background (see "Honest simplifications" above).
            ui.painter().rect_filled(screen, 0.0, color::OBSIDIAN);
            let wash = gradient::LinearGradient {
                start: glass::white_alpha(0.05),
                end: glass::white_alpha(0.0),
                direction: gradient::Direction::TopToBottom,
            };
            let band = egui::Rect::from_min_max(
                screen.min,
                egui::pos2(screen.max.x, screen.min.y + screen.height() * 0.45),
            );
            ui.painter().rect_filled(band, 0.0, wash.sample(0.0));
            ui.painter().rect_filled(
                egui::Rect::from_min_max(band.left_bottom(), band.right_bottom() + egui::vec2(0.0, screen.height() * 0.2)),
                0.0,
                wash.sample(0.6),
            );

            // Top-right collapse button.
            let collapse_size = egui::vec2(36.0, 36.0);
            let collapse_rect = egui::Rect::from_min_size(
                egui::pos2(screen.right() - 16.0 - collapse_size.x, screen.top() + 16.0),
                collapse_size,
            );
            let mut collapse_ui = ui.new_child(egui::UiBuilder::new().max_rect(collapse_rect));
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

            let track = state.now_playing.clone();
            let mut content = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(screen)
                    .layout(egui::Layout::top_down(egui::Align::Center)),
            );
            content.add_space(screen.height() * 0.08);

            match &track {
                Some(t) => {
                    fullscreen_artwork(&mut content, t);
                    content.add_space(18.0);
                    fullscreen_title_block(&mut content, t);
                    content.add_space(28.0);
                    fullscreen_lyrics(&mut content, state, t);
                    content.add_space(24.0);
                }
                None => {
                    content.add_space(screen.height() * 0.2);
                    content.label(
                        egui::RichText::new("Nothing playing")
                            .font(font::title1())
                            .color(color::SILVER_MUTED),
                    );
                    content.add_space(24.0);
                }
            }

            // Transport + scrubber, pinned near the bottom (reuses the bar's own widgets
            // so fullscreen playback control matches the floating bar exactly).
            content.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.add_space(32.0);
                ui.horizontal(|ui| {
                    transport_controls(ui, state);
                });
                if let Some(t) = &track {
                    ui.add_space(14.0);
                    ui.allocate_ui(egui::vec2(480.0_f32.min(screen.width() - 80.0), 24.0), |ui| {
                        scrubber_row(ui, state, t);
                    });
                }
            });
        });
}

/// Large (220x220) artwork tile for the fullscreen view. Still a placeholder (see the
/// "Honest simplifications" note on [`render_fullscreen_overlay`]) — a bigger version of
/// the bar's own note-glyph chip, plus the HR badge when the track is Hi-Res.
fn fullscreen_artwork(ui: &mut egui::Ui, track: &NowPlayingPlaceholder) {
    let size = egui::vec2(220.0, 220.0);
    ui.vertical_centered(|ui| {
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        ui.painter().rect_filled(rect, egui::Rounding::same(14.0), color::INPUT);
        ui.painter().rect_stroke(
            rect,
            egui::Rounding::same(14.0),
            egui::Stroke::new(1.0, glass::white_alpha(0.12)),
        );
        let galley = ui.painter().layout_no_wrap(
            "\u{266A}".to_owned(),
            egui::FontId::proportional(48.0),
            color::SILVER_MUTED,
        );
        ui.painter().galley(rect.center() - galley.size() * 0.5, galley, color::SILVER_MUTED);
        if track.is_hi_res {
            ui.add_space(10.0);
            crate::theme::widgets::hi_res_badge(ui);
        }
    });
}

/// Title + artist block centered below the fullscreen artwork.
fn fullscreen_title_block(ui: &mut egui::Ui, track: &NowPlayingPlaceholder) {
    let title = if track.title.trim().is_empty() {
        "Unknown Title"
    } else {
        track.title.as_str()
    };
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new(title).font(font::title2()).color(color::SILVER_LIGHT));
        if !track.artist.trim().is_empty() {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(track.artist.as_str())
                    .font(font::callout())
                    .color(color::SILVER_MUTED),
            );
        }
    });
}

/// How many lyric lines surround the active one are shown at once (the active line plus
/// this many before/after), matching a focused karaoke-style window rather than the whole
/// transcript.
const LYRICS_CONTEXT_LINES: usize = 2;

/// Karaoke-style lyrics centered under the title: the active line (per requirement 7.3's
/// ~0.3s lead, [`AppState::lyrics_lead_seconds`]) highlighted in silver, the surrounding
/// lines muted. Renders nothing when [`AppState::lyrics_lines`] is empty (no `.lrc` loaded
/// for this track — task 18 populates it from a real file).
fn fullscreen_lyrics(ui: &mut egui::Ui, state: &AppState, track: &NowPlayingPlaceholder) {
    if state.lyrics_lines.is_empty() {
        return;
    }
    let active = active_lyric_line_index(
        &state.lyrics_lines,
        track.position_secs,
        state.lyrics_lead_seconds,
    );
    let Some(active) = active else { return };
    let start = active.saturating_sub(LYRICS_CONTEXT_LINES);
    let end = (active + LYRICS_CONTEXT_LINES + 1).min(state.lyrics_lines.len());

    ui.vertical_centered(|ui| {
        for (i, line) in state.lyrics_lines[start..end].iter().enumerate() {
            let index = start + i;
            let (font_id, color) = if index == active {
                (font::title3(), color::SILVER_LIGHT)
            } else {
                (font::callout(), color::SILVER_MUTED.gamma_multiply(0.7))
            };
            ui.label(egui::RichText::new(line.text.as_str()).font(font_id).color(color));
            ui.add_space(6.0);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use walkman_services::walkman_engine::LrcLine;

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

    /// The fullscreen overlay's karaoke lyrics render without panicking for every position
    /// relative to the loaded `.lrc` lines (before the first line, mid-song, after the
    /// last line), including the empty-lyrics case already covered above.
    #[test]
    fn fullscreen_lyrics_render_at_every_playback_position() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);

        let lines = vec![
            LrcLine { timestamp_seconds: 0.0, text: "First line".to_owned() },
            LrcLine { timestamp_seconds: 10.0, text: "Second line".to_owned() },
            LrcLine { timestamp_seconds: 20.0, text: "Third line".to_owned() },
        ];

        for position in [-1.0, 0.0, 10.5, 20.0, 999.0] {
            let mut state = AppState::default();
            state.lyrics_lines = lines.clone();
            state.is_fullscreen_player = true;
            state.now_playing = Some(NowPlayingPlaceholder {
                position_secs: position,
                ..NowPlayingPlaceholder::sample()
            });

            let _ = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |_ui| {});
                render_fullscreen_overlay(ctx, &mut state);
            });
        }
    }

    // ---- Player binding (task 15.1) ----

    fn sample_player_track(path: &str) -> walkman_domain::models::Track {
        walkman_domain::models::Track {
            filename: "a.wav".into(),
            relative_path: "a.wav".into(),
            full_path: path.into(),
            format: "WAV".into(),
            title: "Bound Title".into(),
            artist: "Bound Artist".into(),
            album: "Bound Album".into(),
            track_num: "1".into(),
            year: "2024".into(),
            genre: String::new(),
            duration_seconds: 10.0,
            bitrate_kbps: 900,
            sample_rate: 44_100,
            bits_per_sample: 16,
            has_artwork: false,
            size_bytes: 1,
            date_added: chrono::Utc::now(),
            is_senseme_analyzed: false,
            senseme_bpm: None,
            senseme_valence: None,
            senseme_arousal: None,
            senseme_channels: Vec::new(),
            senseme_highlight_sec: None,
        }
    }

    fn write_tiny_wav(path: &std::path::Path, num_samples: usize) {
        let samples: Vec<i16> = (0..num_samples).map(|i| ((i % 100) as i16) * 100).collect();
        let data_len = (samples.len() * 2) as u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&8_000u32.to_le_bytes());
        bytes.extend_from_slice(&16_000u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            bytes.extend_from_slice(&s.to_le_bytes());
        }
        std::fs::write(path, bytes).unwrap();
    }

    /// `sync_now_playing_from_player` mirrors a real, bound [`walkman_services::player::Player`]
    /// into `AppState::now_playing` each frame. Skipped on a host with no default audio output
    /// device (an environment property, not something this code controls), matching the skip
    /// pattern `player.rs`'s own tests use.
    #[test]
    fn sync_now_playing_from_player_mirrors_a_bound_player() {
        use walkman_services::player::Player;

        let player = match Player::new(None) {
            Ok(p) => std::sync::Arc::new(p),
            Err(_) => {
                eprintln!("skipping: no default audio output device on this host");
                return;
            }
        };

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        write_tiny_wav(&path, 80_000); // long enough not to finish mid-test

        player.set_queue(vec![sample_player_track(path.to_str().unwrap())]);
        player.play_index(0).unwrap();

        let mut state = AppState::default();
        state.player = Some(player.clone());

        sync_now_playing_from_player(&mut state);

        let now_playing = state.now_playing.expect("expected now_playing to be populated from the player");
        assert_eq!(now_playing.title, "Bound Title");
        assert_eq!(now_playing.artist, "Bound Artist");
        assert!(now_playing.is_playing);
        assert!(!now_playing.is_hi_res); // 16-bit / 44.1kHz is not Hi-Res.
    }

    /// With no player bound, syncing is a no-op — the task 4.4 placeholder in
    /// `AppState::now_playing` is left exactly as the caller set it.
    #[test]
    fn sync_now_playing_from_player_is_a_noop_without_a_bound_player() {
        let mut state = AppState::default();
        state.now_playing = Some(NowPlayingPlaceholder::sample());

        sync_now_playing_from_player(&mut state);

        assert_eq!(state.now_playing, Some(NowPlayingPlaceholder::sample()));
    }
}
