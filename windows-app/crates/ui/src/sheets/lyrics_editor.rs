//! The built-in lyrics editor (spec task 18, R15.6): edit/paste plain lyrics, tap-sync each
//! line to the current playhead while a track plays, and save as a companion `.lrc` — over
//! the real `walkman_services::walkman_engine::{LrcLine, synthesize_lrc_lines, write_lrc}`
//! built in task 9.3.
//!
//! Any line the user never taps still gets a sensible timestamp: [`build_lines`] fills every
//! untapped line from the same evenly-spaced `synthesize_lrc_lines` fallback the batch
//! lyrics generator's plain-text path already uses (R15.4's "synthesized timestamps when
//! only plain text is available"), so a save always produces a strictly ordered `.lrc` even
//! with zero taps — tapping lines is an enhancement, not a requirement to save at all.

use eframe::egui;

use walkman_services::walkman_engine::{synthesize_lrc_lines, LrcLine};

use crate::theme::{button, color, font};

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

/// Renders the sheet if [`LyricsEditorState::open`]; a no-op otherwise. `playhead_seconds` is
/// the current player position (e.g. `AppState::now_playing`'s), used by each line's "Tap"
/// button.
pub fn render(ctx: &egui::Context, playhead_seconds: f64, state: &mut LyricsEditorState) {
    if !state.open {
        return;
    }
    let track_id = state.track_id.clone();
    crate::sheets::modal(ctx, "lyrics_editor_sheet", egui::vec2(640.0, 600.0), |ui| {
        ui.label(egui::RichText::new("Edit Lyrics").font(font::title2()).color(color::SILVER_LIGHT));
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Paste or type lyrics below, one line per line, then tap each line while the track plays to sync it.")
                .font(font::caption())
                .color(color::SILVER_MUTED),
        );
        ui.add_space(8.0);

        let text_changed = ui.add(egui::TextEdit::multiline(&mut state.raw_text).desired_rows(6).desired_width(f32::INFINITY)).changed();
        if text_changed {
            state.synced = vec![None; non_empty_line_count(&state.raw_text)];
        }
        ui.add_space(10.0);

        ui.label(egui::RichText::new("Sync").font(font::caption_bold()).color(color::SILVER_MUTED));
        egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
            let lines: Vec<String> = state.raw_text.lines().filter(|l| !l.trim().is_empty()).map(str::to_owned).collect();
            for (index, text) in lines.iter().enumerate() {
                ui.horizontal(|ui| {
                    let timestamp_label = state.synced.get(index).copied().flatten().map(format_timestamp).unwrap_or_else(|| "\u{2014}".to_owned());
                    ui.label(egui::RichText::new(timestamp_label).font(font::mono_small()).color(color::HIRES_GOLD));
                    ui.label(egui::RichText::new(text).font(font::body()).color(color::SILVER_LIGHT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(button::Button::new("Tap", button::Variant::Glass)).clicked() {
                            if let Some(slot) = state.synced.get_mut(index) {
                                *slot = Some(playhead_seconds);
                            }
                        }
                    });
                });
            }
        });

        ui.add_space(14.0);
        ui.horizontal(|ui| {
            if ui.add(button::Button::new("Save", button::Variant::Prominent).enabled(!state.raw_text.trim().is_empty())).clicked() {
                let lines = build_lines(&state.raw_text, &state.synced, state.duration_seconds);
                state.commands.push(LyricsEditorCommand::Save { track_id: track_id.clone(), lines });
                state.close();
            }
            ui.add_space(8.0);
            if ui.add(button::Button::new("Cancel", button::Variant::Glass)).clicked() {
                state.close();
            }
        });
    });
}

fn format_timestamp(seconds: f64) -> String {
    let total = seconds.max(0.0) as u64;
    format!("{}:{:02}", total / 60, total % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- open_for / lifecycle ----

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

    // ---- build_lines ----

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

    // ---- Headless render smoke test ----

    #[test]
    fn lyrics_editor_renders_in_headless_frame_for_every_state() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);

        let mut closed = LyricsEditorState::default();
        let mut empty = LyricsEditorState::default();
        empty.open_for("track.flac", 180.0, "");
        let mut with_lines = LyricsEditorState::default();
        with_lines.open_for("track.flac", 180.0, "First line\nSecond line\nThird line");
        with_lines.synced[0] = Some(12.5);

        for state in [&mut closed, &mut empty, &mut with_lines] {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |_ui| {});
                render(ctx, 42.0, state);
            });
        }
    }
}
