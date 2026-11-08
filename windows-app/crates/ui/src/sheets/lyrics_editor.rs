//! Pure types/logic behind the built-in lyrics editor (R15.6) — edit/paste plain lyrics,
//! tap-sync each line to the current playhead, save as a companion `.lrc`. The rendering
//! itself lives in `iced_ui::sheets::lyrics_editor` (the egui `render` this module used
//! to carry was removed once the iced migration reached parity — see Milestone 8 of the
//! egui-to-iced migration plan).
//!
//! Any line the user never taps still gets a sensible timestamp: [`build_lines`] fills every
//! untapped line from the same evenly-spaced `synthesize_lrc_lines` fallback the batch
//! lyrics generator's plain-text path already uses (R15.4's "synthesized timestamps when
//! only plain text is available"), so a save always produces a strictly ordered `.lrc` even
//! with zero taps — tapping lines is an enhancement, not a requirement to save at all.

use walkman_services::walkman_engine::{synthesize_lrc_lines, LrcLine};

#[derive(Debug, Clone, PartialEq)]
pub enum LyricsEditorCommand {
    Save { track_id: String, lines: Vec<LrcLine> },
}

#[derive(Debug, Clone, Default)]
pub struct LyricsEditorState {
    pub open: bool,
    pub track_id: String,
    pub duration_seconds: f64,
    pub raw_text: String,
    /// One entry per non-empty line of `raw_text`, in order; `Some` once that line has been
    /// tap-synced to a playhead position.
    pub synced: Vec<Option<f64>>,
    pub commands: Vec<LyricsEditorCommand>,
}

impl LyricsEditorState {
    /// Opens the editor for `track_id`, seeding `raw_text` from whatever plain lyrics text is
    /// already known (empty if none) and resetting every line to unsynced.
    pub fn open_for(&mut self, track_id: &str, duration_seconds: f64, existing_plain_text: &str) {
        *self = Self {
            open: true,
            track_id: track_id.to_owned(),
            duration_seconds,
            raw_text: existing_plain_text.to_owned(),
            synced: vec![None; non_empty_line_count(existing_plain_text)],
            commands: Vec::new(),
        };
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}

fn non_empty_line_count(text: &str) -> usize {
    text.lines().filter(|l| !l.trim().is_empty()).count()
}

/// Builds the final, strictly-ordered `.lrc` line list to save: every tap-synced line uses
/// its tapped timestamp; every other line falls back to the same evenly-spaced
/// `synthesize_lrc_lines` schedule the batch generator's plain-text path already uses
/// (R15.4), so a line nobody tapped still gets a sensible position rather than `0:00`.
pub fn build_lines(raw_text: &str, synced: &[Option<f64>], duration_seconds: f64) -> Vec<LrcLine> {
    let fallback = synthesize_lrc_lines(raw_text, 0.0, duration_seconds);
    raw_text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, text)| {
            let timestamp = synced.get(i).copied().flatten().or_else(|| fallback.get(i).map(|l| l.timestamp_seconds)).unwrap_or(0.0);
            LrcLine { timestamp_seconds: timestamp, text: text.trim().to_owned() }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_for_seeds_text_and_resets_sync_state() {
        let mut state = LyricsEditorState::default();
        state.open_for("track.flac", 180.0, "Line one\nLine two");

        assert!(state.open);
        assert_eq!(state.raw_text, "Line one\nLine two");
        assert_eq!(state.synced, vec![None, None]);
    }

    #[test]
    fn open_for_with_empty_text_starts_with_no_lines() {
        let mut state = LyricsEditorState::default();
        state.open_for("track.flac", 180.0, "");
        assert!(state.synced.is_empty());
    }

    #[test]
    fn build_lines_uses_tapped_timestamps_where_present() {
        let lines = build_lines("First\nSecond\nThird", &[Some(5.0), None, Some(20.0)], 30.0);
        assert_eq!(lines[0], LrcLine { timestamp_seconds: 5.0, text: "First".into() });
        assert_eq!(lines[2], LrcLine { timestamp_seconds: 20.0, text: "Third".into() });
    }

    #[test]
    fn build_lines_falls_back_to_synthesized_spacing_for_untapped_lines() {
        let lines = build_lines("First\nSecond\nThird", &[None, None, None], 30.0);
        let fallback = synthesize_lrc_lines("First\nSecond\nThird", 0.0, 30.0);
        assert_eq!(lines, fallback);
    }

    #[test]
    fn build_lines_handles_fewer_sync_slots_than_lines() {
        // E.g. the text was edited after some lines were already tapped, shrinking `synced`.
        let lines = build_lines("First\nSecond", &[Some(5.0)], 30.0);
        assert_eq!(lines[0].timestamp_seconds, 5.0);
        assert!(lines[1].timestamp_seconds > 0.0); // fell back to the synthesized schedule.
    }

    #[test]
    fn build_lines_ignores_blank_lines() {
        let lines = build_lines("First\n\nSecond", &[None, None], 10.0);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "First");
        assert_eq!(lines[1].text, "Second");
    }
}
