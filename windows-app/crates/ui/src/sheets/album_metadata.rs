//! `AlbumMetadataEditorSheet`, ported from
//! `Sources/WalkmanStudio/Views/AlbumMetadataEditorSheet.swift`.
//!
//! Spec task **16.3** (requirement 8.3): batch-edits album title/artist/year/genre across
//! every track in the album, embeds unified Baseline-JPEG artwork to every track plus a
//! companion `folder.jpg`, and supports interactive tracklist reordering with automatic
//! renumbering (`1..N` or `01..N`).
//!
//! ## Honest simplification
//! Artwork import/compliance reuses the exact same pattern as
//! [`crate::sheets::song_metadata`]'s Album Artwork tab (a placeholder tile + the real
//! `artwork::ArtworkCompliance` badges) rather than a parallel implementation — see that
//! module's own doc comment for the drag-and-drop caveat, which applies here too.

use eframe::egui;

use walkman_domain::models::Track;
use walkman_services::artwork::ArtworkCompliance;

use crate::sheets::text_field;
use crate::theme::{button, color, font, glass};

/// Modal size. The macOS sheet doesn't specify an exact size the way the Song editor does
/// (R8.2); this is sized to comfortably fit the batch fields + a scrollable tracklist.
pub const SHEET_SIZE: egui::Vec2 = egui::vec2(720.0, 640.0);

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AlbumEditorFields {
    pub title: String,
    pub artist: String,
    pub year: String,
    pub genre: String,
}

/// One tracklist row: the track's id ([`Track::id`]) plus its editable display fields.
#[derive(Debug, Clone, PartialEq)]
pub struct AlbumEditorTrackRow {
    pub track_id: String,
    pub track_number: String,
    pub title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenumberStyle {
    /// `1, 2, 3, ... N`.
    OneToN,
    /// `01, 02, 03, ... N`, zero-padded to the width of the row count.
    ZeroPaddedToN,
}

/// Renumbers every row's `track_number` in place, in current list order, per `style` (R8.3).
pub fn renumber(rows: &mut [AlbumEditorTrackRow], style: RenumberStyle) {
    let width = rows.len().to_string().len().max(2);
    for (index, row) in rows.iter_mut().enumerate() {
        let n = index + 1;
        row.track_number = match style {
            RenumberStyle::OneToN => n.to_string(),
            RenumberStyle::ZeroPaddedToN => format!("{n:0width$}"),
        };
    }
}

/// Swaps `index` with the row above it. A no-op at the top of the list or out of bounds.
pub fn move_row_up(rows: &mut [AlbumEditorTrackRow], index: usize) {
    if index == 0 || index >= rows.len() {
        return;
    }
    rows.swap(index, index - 1);
}

/// Swaps `index` with the row below it. A no-op at the bottom of the list or out of bounds.
pub fn move_row_down(rows: &mut [AlbumEditorTrackRow], index: usize) {
    if index + 1 >= rows.len() {
        return;
    }
    rows.swap(index, index + 1);
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlbumEditorCommand {
    /// Apply `fields` to every track, and persist each row's (possibly renumbered)
    /// `track_number`/order — `track_ids` is in the tracklist's final display order.
    Save { track_ids: Vec<String>, track_numbers: Vec<String>, fields: AlbumEditorFields },
    ImportArtworkFromFile,
}

#[derive(Debug, Clone, Default)]
pub struct AlbumEditorState {
    pub open: bool,
    pub fields: AlbumEditorFields,
    pub tracks: Vec<AlbumEditorTrackRow>,
    /// Compliance facts for the unified artwork staged for this album, if any. `None` until
    /// task 18 loads real artwork bytes and validates them with `artwork::validate_jpeg`.
    pub artwork_compliance: Option<ArtworkCompliance>,
    pub commands: Vec<AlbumEditorCommand>,
}

impl AlbumEditorState {
    /// Opens the sheet for `tracks` (every track in the album being edited), seeding the
    /// batch fields from the first track (R8.3 edits the album as a unit, so any track's
    /// album-level fields are an equally valid starting point).
    pub fn open_for(&mut self, tracks: &[Track]) {
        self.open = true;
        self.fields = match tracks.first() {
            Some(first) => AlbumEditorFields {
                title: first.album.clone(),
                artist: first.artist.clone(),
                year: first.year.clone(),
                genre: first.genre.clone(),
            },
            None => AlbumEditorFields::default(),
        };
        self.tracks = tracks
            .iter()
            .map(|t| AlbumEditorTrackRow { track_id: t.id().to_owned(), track_number: t.track_num.clone(), title: t.title.clone() })
            .collect();
        self.artwork_compliance = None;
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}

/// Renders the sheet if [`AlbumEditorState::open`]; a no-op otherwise. Call once per frame.
pub fn render(ctx: &egui::Context, state: &mut AlbumEditorState) {
    if !state.open {
        return;
    }
    crate::sheets::modal(ctx, "album_metadata_editor_sheet", SHEET_SIZE, |ui| {
        ui.label(egui::RichText::new("Edit Album Metadata").font(font::title2()).color(color::SILVER_LIGHT));
        ui.add_space(10.0);

        egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(SHEET_SIZE.y - 180.0).show(ui, |ui| {
            text_field(ui, "Album Title", &mut state.fields.title);
            text_field(ui, "Album Artist", &mut state.fields.artist);
            ui.columns(2, |cols| {
                text_field(&mut cols[0], "Year", &mut state.fields.year);
                text_field(&mut cols[1], "Genre", &mut state.fields.genre);
            });

            ui.add_space(10.0);
            render_artwork_row(ui, state);

            ui.add_space(14.0);
            render_tracklist(ui, state);
        });

        ui.add_space(16.0);
        render_footer(ui, state);
    });
}

fn render_artwork_row(ui: &mut egui::Ui, state: &mut AlbumEditorState) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(72.0, 72.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, egui::Rounding::same(8.0), color::INPUT);
        ui.painter().rect_stroke(rect, egui::Rounding::same(8.0), egui::Stroke::new(1.0, glass::white_alpha(0.15)));
        let galley = ui.painter().layout_no_wrap("\u{266A}".to_owned(), egui::FontId::proportional(22.0), color::SILVER_MUTED);
        ui.painter().galley(rect.center() - galley.size() * 0.5, galley, color::SILVER_MUTED);

        ui.add_space(12.0);
        ui.vertical(|ui| {
            if ui.add(button::Button::new("Import Unified Artwork\u{2026}", button::Variant::Prominent)).clicked() {
                state.commands.push(AlbumEditorCommand::ImportArtworkFromFile);
            }
            ui.label(
                egui::RichText::new("Applies to every track plus a companion folder.jpg")
                    .font(font::caption())
                    .color(color::SILVER_MUTED),
            );
            if let Some(compliance) = state.artwork_compliance {
                if compliance.is_walkman_ready() {
                    ui.label(
                        egui::RichText::new(format!("{}\u{00D7}{} \u{2022} Walkman Ready", compliance.width, compliance.height))
                            .font(font::caption_bold())
                            .color(color::HIRES_GOLD),
                    );
                } else {
                    for badge in compliance.badges() {
                        ui.label(egui::RichText::new(badge).font(font::caption()).color(color::SILVER_MUTED));
                    }
                }
            }
        });
    });
}

fn render_tracklist(ui: &mut egui::Ui, state: &mut AlbumEditorState) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Tracklist").font(font::caption_bold()).color(color::SILVER_MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(button::Button::new("Renumber 01..N", button::Variant::Glass)).clicked() {
                renumber(&mut state.tracks, RenumberStyle::ZeroPaddedToN);
            }
            ui.add_space(6.0);
            if ui.add(button::Button::new("Renumber 1..N", button::Variant::Glass)).clicked() {
                renumber(&mut state.tracks, RenumberStyle::OneToN);
            }
        });
    });
    ui.add_space(6.0);

    let row_count = state.tracks.len();
    for index in 0..row_count {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(&state.tracks[index].track_number)
                    .font(font::mono_small())
                    .color(color::SILVER_MUTED),
            );
            ui.label(egui::RichText::new(&state.tracks[index].title).font(font::body()).color(color::SILVER_LIGHT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(button::Button::new("\u{2193}", button::Variant::Glass).enabled(index + 1 < row_count)).clicked() {
                    move_row_down(&mut state.tracks, index);
                }
                if ui.add(button::Button::new("\u{2191}", button::Variant::Glass).enabled(index > 0)).clicked() {
                    move_row_up(&mut state.tracks, index);
                }
            });
        });
    }
}

fn render_footer(ui: &mut egui::Ui, state: &mut AlbumEditorState) {
    ui.horizontal(|ui| {
        if ui.add(button::Button::new("Save", button::Variant::Prominent)).clicked() {
            state.commands.push(AlbumEditorCommand::Save {
                track_ids: state.tracks.iter().map(|r| r.track_id.clone()).collect(),
                track_numbers: state.tracks.iter().map(|r| r.track_number.clone()).collect(),
                fields: state.fields.clone(),
            });
            state.close();
        }
        ui.add_space(8.0);
        if ui.add(button::Button::new("Cancel", button::Variant::Glass)).clicked() {
            state.close();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_track(id: &str, track_num: &str, title: &str) -> Track {
        Track {
            filename: id.into(),
            relative_path: id.into(),
            full_path: id.into(),
            format: "FLAC".into(),
            title: title.into(),
            artist: "Album Artist".into(),
            album: "The Album".into(),
            track_num: track_num.into(),
            year: "2021".into(),
            genre: "Rock".into(),
            duration_seconds: 200.0,
            bitrate_kbps: 900,
            sample_rate: 44_100,
            bits_per_sample: 16,
            has_artwork: false,
            size_bytes: 1,
            date_added: Utc::now(),
            is_senseme_analyzed: false,
            senseme_bpm: None,
            senseme_valence: None,
            senseme_arousal: None,
            senseme_channels: Vec::new(),
            senseme_highlight_sec: None,
        }
    }

    // ---- Renumbering ----

    #[test]
    fn renumber_one_to_n_is_sequential_unpadded() {
        let mut rows = vec![
            AlbumEditorTrackRow { track_id: "a".into(), track_number: "9".into(), title: "A".into() },
            AlbumEditorTrackRow { track_id: "b".into(), track_number: "1".into(), title: "B".into() },
        ];
        renumber(&mut rows, RenumberStyle::OneToN);
        assert_eq!(rows[0].track_number, "1");
        assert_eq!(rows[1].track_number, "2");
    }

    #[test]
    fn renumber_zero_padded_matches_row_count_width() {
        // 11 rows -> width 2 ("01".."11").
        let mut rows: Vec<AlbumEditorTrackRow> =
            (0..11).map(|i| AlbumEditorTrackRow { track_id: i.to_string(), track_number: String::new(), title: String::new() }).collect();
        renumber(&mut rows, RenumberStyle::ZeroPaddedToN);
        assert_eq!(rows[0].track_number, "01");
        assert_eq!(rows[10].track_number, "11");
    }

    #[test]
    fn renumber_zero_padded_pads_to_at_least_two_digits_for_small_albums() {
        let mut rows: Vec<AlbumEditorTrackRow> =
            (0..3).map(|i| AlbumEditorTrackRow { track_id: i.to_string(), track_number: String::new(), title: String::new() }).collect();
        renumber(&mut rows, RenumberStyle::ZeroPaddedToN);
        assert_eq!(rows[0].track_number, "01");
        assert_eq!(rows[2].track_number, "03");
    }

    // ---- Reordering ----

    #[test]
    fn move_row_up_swaps_with_the_previous_row() {
        let mut rows = vec![
            AlbumEditorTrackRow { track_id: "a".into(), track_number: "1".into(), title: "A".into() },
            AlbumEditorTrackRow { track_id: "b".into(), track_number: "2".into(), title: "B".into() },
        ];
        move_row_up(&mut rows, 1);
        assert_eq!(rows[0].track_id, "b");
        assert_eq!(rows[1].track_id, "a");
    }

    #[test]
    fn move_row_up_at_the_top_is_a_noop() {
        let mut rows = vec![AlbumEditorTrackRow { track_id: "a".into(), track_number: "1".into(), title: "A".into() }];
        move_row_up(&mut rows, 0);
        assert_eq!(rows[0].track_id, "a");
    }

    #[test]
    fn move_row_down_swaps_with_the_next_row_and_is_a_noop_at_the_bottom() {
        let mut rows = vec![
            AlbumEditorTrackRow { track_id: "a".into(), track_number: "1".into(), title: "A".into() },
            AlbumEditorTrackRow { track_id: "b".into(), track_number: "2".into(), title: "B".into() },
        ];
        move_row_down(&mut rows, 0);
        assert_eq!(rows[0].track_id, "b");
        assert_eq!(rows[1].track_id, "a");

        move_row_down(&mut rows, 1); // already at the bottom.
        assert_eq!(rows[1].track_id, "a");
    }

    // ---- open_for seeding ----

    #[test]
    fn open_for_seeds_batch_fields_from_the_first_track() {
        let tracks = vec![sample_track("a", "1", "Track A"), sample_track("b", "2", "Track B")];
        let mut state = AlbumEditorState::default();
        state.open_for(&tracks);

        assert!(state.open);
        assert_eq!(state.fields.title, "The Album");
        assert_eq!(state.fields.artist, "Album Artist");
        assert_eq!(state.fields.year, "2021");
        assert_eq!(state.fields.genre, "Rock");
        assert_eq!(state.tracks.len(), 2);
        assert_eq!(state.tracks[0].track_id, "a");
    }

    #[test]
    fn open_for_with_no_tracks_stays_safely_empty() {
        let mut state = AlbumEditorState::default();
        state.open_for(&[]);
        assert!(state.open);
        assert_eq!(state.fields, AlbumEditorFields::default());
        assert!(state.tracks.is_empty());
    }

    // ---- Headless render smoke test ----

    #[test]
    fn album_editor_renders_in_headless_frame_for_every_state() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let tracks = vec![sample_track("a", "1", "Track A"), sample_track("b", "2", "Track B")];

        let mut closed = AlbumEditorState::default();
        let mut open = AlbumEditorState::default();
        open.open_for(&tracks);
        let mut open_with_compliance = AlbumEditorState::default();
        open_with_compliance.open_for(&tracks);
        open_with_compliance.artwork_compliance = Some(ArtworkCompliance {
            width: 800,
            height: 800,
            is_progressive: false,
            color_model: walkman_services::artwork::ColorModel::YCbCrOrRgb,
        });

        for state in [&mut closed, &mut open, &mut open_with_compliance] {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |_ui| {});
                render(ctx, state);
            });
        }
    }
}
