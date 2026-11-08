//! Pure types/logic behind `TrimAndCutSheet`, ported from
//! `Sources/WalkmanStudio/Views/TrimAndCutSheet.swift`. The rendering itself lives in
//! `iced_ui::sheets::trim_cut` (the egui `render`/waveform painting this module used to
//! carry was removed once the iced migration reached parity — see Milestone 8 of the
//! egui-to-iced migration plan).
//!
//! "Gapless preview playback" (R14.1) means actually feeding the trimmed/cut range to the
//! real `Player`; this module only carries `Preview`/`StopPreview` commands — driving an
//! actual `Player` instance from them and writing the playhead position back into
//! [`TrimCutSheetState::playhead_ms`] each tick is the app layer's job.

use walkman_services::trim_cut::{ChannelMode, TrimCutMode, TrimCutOptions};

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
    /// Peak samples for the waveform, normalized 0.0..=1.0. Empty until the app layer runs
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn ms_to_fraction_and_back_round_trip() {
        assert_eq!(ms_to_fraction(5_000, 10_000), 0.5);
        assert_eq!(fraction_to_ms(0.5, 10_000), 5_000);
        assert_eq!(ms_to_fraction(0, 0), 0.0); // degenerate duration never divides by zero
    }
}
