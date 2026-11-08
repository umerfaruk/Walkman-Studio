//! `TrimAndCutSheet`, ported from `Sources/WalkmanStudio/Views/TrimAndCutSheet.swift`.
//!
//! Spec task **17.5** (requirements 14.1-14.5): a waveform scrubber with draggable in/out
//! handles, Trim/Cut modes, millisecond steppers (±0.1/±1/±5s) + lock-to-playhead, channel
//! routing, fades + gain, and preview playback — over the real
//! `walkman_services::trim_cut::{TrimCutMode, ChannelMode, TrimCutOptions, generate_waveform}`
//! built in task 12.1.
//!
//! ## Honest simplification
//! "Gapless preview playback" (R14.1) means actually feeding the trimmed/cut range to the
//! real `Player` (task 13); this sheet only renders a playhead and emits `Preview`/
//! `StopPreview` commands — task 18 is expected to drive an actual `Player` instance from
//! them and write the playhead position back into [`TrimCutSheetState::playhead_ms`] each
//! frame while it plays.

use eframe::egui;

use walkman_services::trim_cut::{ChannelMode, TrimCutMode, TrimCutOptions};

use crate::theme::{button, color, font, glass};

pub const SHEET_SIZE: egui::Vec2 = egui::vec2(760.0, 560.0);

/// R14.3's three stepper increments.
pub const STEP_MS_SMALL: u64 = 100;
pub const STEP_MS_MEDIUM: u64 = 1_000;
pub const STEP_MS_LARGE: u64 = 5_000;

/// R14.5's gain range.
pub const GAIN_MIN_DB: f32 = -6.0;
pub const GAIN_MAX_DB: f32 = 6.0;

#[derive(Debug, Clone, PartialEq)]
pub enum TrimCutCommand {
    Preview,
    StopPreview,
    Apply { track_id: String, options: TrimCutOptions },
    Cancel,
}

#[derive(Debug, Clone)]
pub struct TrimCutSheetState {
    pub open: bool,
    pub track_id: String,
    /// Peak samples for the waveform, normalized 0.0..=1.0. Empty until task 18 runs
    /// `trim_cut::generate_waveform` for the opened track.
    pub waveform: Vec<f32>,
    pub duration_ms: u64,
    pub mode: TrimCutMode,
    pub start_ms: u64,
    pub end_ms: u64,
    pub channel_mode: ChannelMode,
    pub fade_in_ms: u64,
    pub fade_out_ms: u64,
    pub gain_db: f32,
    pub lock_to_playhead: bool,
    pub playhead_ms: u64,
    pub is_previewing: bool,
    pub commands: Vec<TrimCutCommand>,
}

impl Default for TrimCutSheetState {
    fn default() -> Self {
        Self {
            open: false,
            track_id: String::new(),
            waveform: Vec::new(),
            duration_ms: 0,
            mode: TrimCutMode::Trim,
            start_ms: 0,
            end_ms: 0,
            channel_mode: ChannelMode::None,
            fade_in_ms: 0,
            fade_out_ms: 0,
            gain_db: 0.0,
            lock_to_playhead: false,
            playhead_ms: 0,
            is_previewing: false,
            commands: Vec::new(),
        }
    }
}

impl TrimCutSheetState {
    /// Opens the sheet for `track_id`/`duration_ms`, with the selection spanning the whole
    /// track and every other option reset.
    pub fn open_for(&mut self, track_id: &str, duration_ms: u64) {
        *self = Self {
            open: true,
            track_id: track_id.to_owned(),
            duration_ms,
            end_ms: duration_ms,
            ..Self::default()
        };
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// The options to pass to `trim_cut::apply_trim_cut`/`build_ffmpeg_args`.
    pub fn options(&self) -> TrimCutOptions {
        TrimCutOptions {
            mode: self.mode,
            start_ms: self.start_ms,
            end_ms: self.end_ms,
            channel_mode: self.channel_mode,
            fade_in_ms: self.fade_in_ms,
            fade_out_ms: self.fade_out_ms,
            gain_db: self.gain_db,
        }
    }
}

/// Nudges `start_ms` by `delta_ms` (positive or negative), clamped to `0..=end_ms`.
pub fn nudge_start(state: &mut TrimCutSheetState, delta_ms: i64) {
    state.start_ms = clamp_nudge(state.start_ms, delta_ms, 0, state.end_ms);
}

/// Nudges `end_ms` by `delta_ms`, clamped to `start_ms..=duration_ms`.
pub fn nudge_end(state: &mut TrimCutSheetState, delta_ms: i64) {
    state.end_ms = clamp_nudge(state.end_ms, delta_ms, state.start_ms, state.duration_ms);
}

fn clamp_nudge(value: u64, delta_ms: i64, lo: u64, hi: u64) -> u64 {
    let shifted = (value as i64 + delta_ms).max(0) as u64;
    shifted.clamp(lo, hi)
}

/// R14.3's "lock-to-playhead": snaps `start_ms` to the current playhead, clamped below
/// `end_ms`.
pub fn lock_start_to_playhead(state: &mut TrimCutSheetState) {
    state.start_ms = state.playhead_ms.min(state.end_ms);
}

/// Snaps `end_ms` to the current playhead, clamped above `start_ms`.
pub fn lock_end_to_playhead(state: &mut TrimCutSheetState) {
    state.end_ms = state.playhead_ms.max(state.start_ms);
}

/// Maps a timestamp to a horizontal fraction (`0.0..=1.0`) of the waveform width.
pub fn ms_to_fraction(ms: u64, duration_ms: u64) -> f32 {
    if duration_ms == 0 {
        0.0
    } else {
        (ms as f32 / duration_ms as f32).clamp(0.0, 1.0)
    }
}

/// Maps a horizontal fraction back to a millisecond timestamp.
pub fn fraction_to_ms(fraction: f32, duration_ms: u64) -> u64 {
    (fraction.clamp(0.0, 1.0) * duration_ms as f32).round() as u64
}

/// Renders the sheet if [`TrimCutSheetState::open`]; a no-op otherwise. Call once per frame.
pub fn render(ctx: &egui::Context, state: &mut TrimCutSheetState) {
    if !state.open {
        return;
    }
    crate::sheets::modal(ctx, "trim_and_cut_sheet", SHEET_SIZE, |ui| {
        ui.label(egui::RichText::new("Trim & Cut").font(font::title2()).color(color::SILVER_LIGHT));
        ui.add_space(10.0);

        render_mode_toggle(ui, state);
        ui.add_space(10.0);
        render_waveform(ui, state);
        ui.add_space(10.0);
        render_steppers(ui, state);
        ui.add_space(10.0);
        render_channel_routing(ui, state);
        ui.add_space(10.0);
        render_fades_and_gain(ui, state);
        ui.add_space(14.0);
        render_footer(ui, state);
    });
}

fn render_mode_toggle(ui: &mut egui::Ui, state: &mut TrimCutSheetState) {
    ui.horizontal(|ui| {
        for (mode, label) in [(TrimCutMode::Trim, "Trim (Keep Selection)"), (TrimCutMode::Cut, "Cut (Remove Selection)")] {
            let variant = if state.mode == mode { button::Variant::Capsule } else { button::Variant::Glass };
            if ui.add(button::Button::new(label, variant)).clicked() {
                state.mode = mode;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if state.is_previewing {
                if ui.add(button::Button::new("Stop", button::Variant::Destructive)).clicked() {
                    state.is_previewing = false;
                    state.commands.push(TrimCutCommand::StopPreview);
                }
            } else if ui.add(button::Button::new("Preview", button::Variant::Prominent)).clicked() {
                state.is_previewing = true;
                state.commands.push(TrimCutCommand::Preview);
            }
        });
    });
}

const WAVEFORM_HEIGHT: f32 = 100.0;

fn render_waveform(ui: &mut egui::Ui, state: &mut TrimCutSheetState) {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, WAVEFORM_HEIGHT), egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, egui::Rounding::same(6.0), color::INPUT);

    if !state.waveform.is_empty() {
        let n = state.waveform.len();
        let bar_width = (rect.width() / n as f32).max(1.0);
        for (i, peak) in state.waveform.iter().enumerate() {
            let x = rect.left() + i as f32 * bar_width;
            let half_height = (peak.clamp(0.0, 1.0) * rect.height() * 0.5).max(1.0);
            let bar = egui::Rect::from_center_size(egui::pos2(x, rect.center().y), egui::vec2(bar_width.max(1.0), half_height * 2.0));
            painter.rect_filled(bar, 0.0, glass::white_alpha(0.5));
        }
    }

    // Selection overlay + in/out handles.
    let start_x = rect.left() + ms_to_fraction(state.start_ms, state.duration_ms) * rect.width();
    let end_x = rect.left() + ms_to_fraction(state.end_ms, state.duration_ms) * rect.width();
    let selection_rect = egui::Rect::from_min_max(egui::pos2(start_x, rect.top()), egui::pos2(end_x, rect.bottom()));
    painter.rect_filled(selection_rect, 0.0, glass::white_alpha(0.12));
    painter.line_segment([egui::pos2(start_x, rect.top()), egui::pos2(start_x, rect.bottom())], egui::Stroke::new(2.0, color::HIRES_GOLD));
    painter.line_segment([egui::pos2(end_x, rect.top()), egui::pos2(end_x, rect.bottom())], egui::Stroke::new(2.0, color::HIRES_GOLD));

    // Playhead.
    let playhead_x = rect.left() + ms_to_fraction(state.playhead_ms, state.duration_ms) * rect.width();
    painter.line_segment([egui::pos2(playhead_x, rect.top()), egui::pos2(playhead_x, rect.bottom())], egui::Stroke::new(1.5, color::SILVER_LIGHT));

    // Dragging anywhere on the waveform moves whichever handle (start/end) is closer.
    if response.dragged() || response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let fraction = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            let ms = fraction_to_ms(fraction, state.duration_ms);
            if ms.abs_diff(state.start_ms) <= ms.abs_diff(state.end_ms) {
                state.start_ms = ms.min(state.end_ms);
            } else {
                state.end_ms = ms.max(state.start_ms);
            }
        }
    }
}

fn render_steppers(ui: &mut egui::Ui, state: &mut TrimCutSheetState) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("In").font(font::caption()).color(color::SILVER_MUTED));
        ui.label(egui::RichText::new(format_ms(state.start_ms)).font(font::mono_small()).color(color::SILVER_LIGHT));
        for (label, step) in [("-5s", STEP_MS_LARGE), ("-1s", STEP_MS_MEDIUM), ("-0.1s", STEP_MS_SMALL), ("+0.1s", STEP_MS_SMALL), ("+1s", STEP_MS_MEDIUM), ("+5s", STEP_MS_LARGE)] {
            let delta = if label.starts_with('-') { -(step as i64) } else { step as i64 };
            if ui.add(button::Button::new(label, button::Variant::Glass)).clicked() {
                nudge_start(state, delta);
            }
        }
        if ui.add(button::Button::new("Lock In\u{2026}", button::Variant::Glass).enabled(state.lock_to_playhead)).clicked() {
            lock_start_to_playhead(state);
        }
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Out").font(font::caption()).color(color::SILVER_MUTED));
        ui.label(egui::RichText::new(format_ms(state.end_ms)).font(font::mono_small()).color(color::SILVER_LIGHT));
        for (label, step) in [("-5s", STEP_MS_LARGE), ("-1s", STEP_MS_MEDIUM), ("-0.1s", STEP_MS_SMALL), ("+0.1s", STEP_MS_SMALL), ("+1s", STEP_MS_MEDIUM), ("+5s", STEP_MS_LARGE)] {
            let delta = if label.starts_with('-') { -(step as i64) } else { step as i64 };
            if ui.add(button::Button::new(label, button::Variant::Glass)).clicked() {
                nudge_end(state, delta);
            }
        }
        if ui.add(button::Button::new("Lock Out\u{2026}", button::Variant::Glass).enabled(state.lock_to_playhead)).clicked() {
            lock_end_to_playhead(state);
        }
    });
    ui.add_space(6.0);
    ui.checkbox(&mut state.lock_to_playhead, "Lock to Playhead");
}

fn format_ms(ms: u64) -> String {
    format!("{:.1}s", ms as f64 / 1000.0)
}

fn render_channel_routing(ui: &mut egui::Ui, state: &mut TrimCutSheetState) {
    ui.label(egui::RichText::new("Channel Routing").font(font::caption_bold()).color(color::SILVER_MUTED));
    ui.horizontal(|ui| {
        for (mode, label) in [
            (ChannelMode::None, "None"),
            (ChannelMode::SwapLr, "Swap L/R"),
            (ChannelMode::Mono, "Mono"),
            (ChannelMode::Stereo, "Stereo"),
            (ChannelMode::IsolateLeft, "Isolate L"),
            (ChannelMode::IsolateRight, "Isolate R"),
            (ChannelMode::PhaseFlip, "Phase Flip"),
        ] {
            let variant = if state.channel_mode == mode { button::Variant::Capsule } else { button::Variant::Glass };
            if ui.add(button::Button::new(label, variant)).clicked() {
                state.channel_mode = mode;
            }
        }
    });
}

fn render_fades_and_gain(ui: &mut egui::Ui, state: &mut TrimCutSheetState) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Fade In (ms)").font(font::caption()).color(color::SILVER_MUTED));
        ui.add(egui::DragValue::new(&mut state.fade_in_ms).range(0..=u64::MAX));
        ui.add_space(12.0);
        ui.label(egui::RichText::new("Fade Out (ms)").font(font::caption()).color(color::SILVER_MUTED));
        ui.add(egui::DragValue::new(&mut state.fade_out_ms).range(0..=u64::MAX));
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Gain (dB)").font(font::caption()).color(color::SILVER_MUTED));
        ui.add(egui::Slider::new(&mut state.gain_db, GAIN_MIN_DB..=GAIN_MAX_DB));
    });
}

fn render_footer(ui: &mut egui::Ui, state: &mut TrimCutSheetState) {
    ui.horizontal(|ui| {
        if ui.add(button::Button::new("Apply", button::Variant::Prominent)).clicked() {
            state.commands.push(TrimCutCommand::Apply { track_id: state.track_id.clone(), options: state.options() });
            state.close();
        }
        ui.add_space(8.0);
        if ui.add(button::Button::new("Cancel", button::Variant::Glass)).clicked() {
            state.commands.push(TrimCutCommand::Cancel);
            state.close();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- open_for / options ----

    #[test]
    fn open_for_selects_the_whole_track_and_resets_every_option() {
        let mut state = TrimCutSheetState::default();
        state.gain_db = 3.0;
        state.open_for("track.flac", 180_000);

        assert!(state.open);
        assert_eq!(state.track_id, "track.flac");
        assert_eq!(state.start_ms, 0);
        assert_eq!(state.end_ms, 180_000);
        assert_eq!(state.gain_db, 0.0);
    }

    #[test]
    fn options_mirrors_the_current_working_state() {
        let mut state = TrimCutSheetState::default();
        state.open_for("track.flac", 10_000);
        state.mode = TrimCutMode::Cut;
        state.start_ms = 1_000;
        state.end_ms = 2_000;
        state.channel_mode = ChannelMode::Mono;
        state.fade_in_ms = 500;
        state.fade_out_ms = 500;
        state.gain_db = -2.0;

        let options = state.options();
        assert_eq!(options.mode, TrimCutMode::Cut);
        assert_eq!(options.start_ms, 1_000);
        assert_eq!(options.end_ms, 2_000);
        assert_eq!(options.channel_mode, ChannelMode::Mono);
        assert_eq!(options.gain_db, -2.0);
    }

    // ---- Nudging ----

    #[test]
    fn nudge_start_clamps_to_zero_and_to_end_ms() {
        let mut state = TrimCutSheetState::default();
        state.open_for("t", 10_000);
        state.start_ms = 500;
        state.end_ms = 1_000;

        nudge_start(&mut state, -1_000); // would go negative
        assert_eq!(state.start_ms, 0);

        state.start_ms = 900;
        nudge_start(&mut state, 500); // would exceed end_ms
        assert_eq!(state.start_ms, 1_000);
    }

    #[test]
    fn nudge_end_clamps_to_start_ms_and_duration() {
        let mut state = TrimCutSheetState::default();
        state.open_for("t", 10_000);
        state.start_ms = 2_000;
        state.end_ms = 9_000;

        nudge_end(&mut state, 5_000); // would exceed duration_ms
        assert_eq!(state.end_ms, 10_000);

        state.end_ms = 2_100;
        nudge_end(&mut state, -500); // would go below start_ms
        assert_eq!(state.end_ms, 2_000);
    }

    // ---- Lock to playhead ----

    #[test]
    fn lock_start_and_end_snap_to_the_playhead_without_crossing() {
        let mut state = TrimCutSheetState::default();
        state.open_for("t", 10_000);
        state.start_ms = 1_000;
        state.end_ms = 8_000;
        state.playhead_ms = 4_000;

        lock_start_to_playhead(&mut state);
        assert_eq!(state.start_ms, 4_000);

        state.playhead_ms = 500; // below the (now 4000) start
        lock_end_to_playhead(&mut state);
        assert_eq!(state.end_ms, 4_000); // clamped up to start, never crosses it
    }

    // ---- ms/fraction mapping ----

    #[test]
    fn ms_to_fraction_and_back_round_trip() {
        assert_eq!(ms_to_fraction(5_000, 10_000), 0.5);
        assert_eq!(fraction_to_ms(0.5, 10_000), 5_000);
        assert_eq!(ms_to_fraction(0, 0), 0.0); // degenerate duration never divides by zero
    }

    // ---- Headless render smoke test ----

    #[test]
    fn trim_cut_sheet_renders_in_headless_frame_for_every_state() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);

        let mut closed = TrimCutSheetState::default();
        let mut open = TrimCutSheetState::default();
        open.open_for("track.flac", 180_000);
        open.waveform = vec![0.2, 0.8, 0.5, 0.9, 0.1];
        let mut previewing = TrimCutSheetState::default();
        previewing.open_for("track.flac", 180_000);
        previewing.is_previewing = true;
        previewing.playhead_ms = 90_000;

        for state in [&mut closed, &mut open, &mut previewing] {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |_ui| {});
                render(ctx, state);
            });
        }
    }
}
