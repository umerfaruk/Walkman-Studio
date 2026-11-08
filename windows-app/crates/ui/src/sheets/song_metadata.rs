//! `SongMetadataEditorSheet`, ported from
//! `Sources/WalkmanStudio/Views/SongMetadataEditorSheet.swift`.
//!
//! Spec task **16.2** (requirement 8.2): tabs `Tags & Specs` / `Album Artwork`, every
//! editable field (Title/Artist/Album/Track #/Disc #/Year/Genre/Lyrics), an artwork drop
//! zone with compliance badges, an export-to-PC action, an online metadata lookup, and an
//! integrated lyrics search. Modal size matches the macOS sheet (≈960×860).
//!
//! ## Honest simplifications
//! - **Drag-and-drop**: egui has no native OS file-drop affordance this crate wires up yet;
//!   "Import Artwork…"/"Export Artwork…" are plain buttons that emit commands for the real
//!   file-dialog platform call (`platform::FileDialogs`, task 6.6) to fulfil (task 18).
//! - **Disc Number**: [`walkman_domain::models::Track`] doesn't carry a disc number (ported
//!   1:1 from the Swift model, which also treats it as writer-only — see
//!   `audio_tagger::TagFields`'s own doc comment), so [`SongEditorFields::from_track`] always
//!   starts it empty; it still round-trips through `Save` into `TagFields::disc_number`.
//! - **Online metadata / lyrics search**: the actual HTTP round trip is `walkman_services`'
//!   job (`online_metadata.rs`/`lyrics.rs`, already built in tasks 8/12); this sheet only
//!   emits the search command and renders whatever candidates/lyrics text it's handed back
//!   in [`SongEditorState::online_candidates`]/[`SongEditorState::lyrics_candidate`].

use eframe::egui;

use walkman_domain::models::Track;
use walkman_services::artwork::ArtworkCompliance;
use walkman_services::online_metadata::MetadataCandidate;

use crate::sheets::text_field;
use crate::theme::{button, color, font, glass};

/// Modal size, matching the macOS sheet's `min width 960, max height 860` (R8.2).
pub const SHEET_SIZE: egui::Vec2 = egui::vec2(960.0, 860.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SongEditorTab {
    #[default]
    TagsAndSpecs,
    AlbumArtwork,
}

/// The editable working copy. Starts from [`SongEditorFields::from_track`] when the sheet
/// opens; `Save` carries the edited copy back out rather than mutating `Track` in place, so
/// `Cancel` can discard it for free.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SongEditorFields {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track_number: String,
    pub disc_number: String,
    pub year: String,
    pub genre: String,
    pub lyrics: String,
}

impl SongEditorFields {
    pub fn from_track(track: &Track) -> Self {
        Self {
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            track_number: track.track_num.clone(),
            disc_number: String::new(),
            year: track.year.clone(),
            genre: track.genre.clone(),
            lyrics: String::new(),
        }
    }

    /// Copies a looked-up [`MetadataCandidate`]'s fields in, leaving `lyrics`/disc number
    /// untouched (online metadata candidates don't carry either).
    pub fn apply_candidate(&mut self, candidate: &MetadataCandidate) {
        self.title = candidate.title.clone();
        self.artist = candidate.artist.clone();
        self.album = candidate.album.clone();
        self.year = candidate.year.clone();
        self.genre = candidate.genre.clone();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SongEditorCommand {
    Save { track_id: String, fields: SongEditorFields },
    ExportArtworkToPc(String),
    ImportArtworkFromFile(String),
    SearchOnlineMetadata { track_id: String, artist: String, title: String },
    SearchLyrics { track_id: String, artist: String, title: String, album: String, duration_seconds: f64 },
}

#[derive(Debug, Clone, Default)]
pub struct SongEditorState {
    pub open: bool,
    pub track_id: String,
    pub track_duration_seconds: f64,
    pub fields: SongEditorFields,
    pub tab: SongEditorTab,
    /// Compliance facts for the artwork currently embedded/staged, if any. `None` until
    /// task 18 loads real artwork bytes and validates them with `artwork::validate_jpeg`.
    pub artwork_compliance: Option<ArtworkCompliance>,
    pub online_candidates: Vec<MetadataCandidate>,
    pub lyrics_candidate: Option<String>,
    pub commands: Vec<SongEditorCommand>,
}

impl SongEditorState {
    /// Opens the sheet for `track`, resetting every working-copy/result field (so a second
    /// open never shows a stale search result from a previous track).
    pub fn open_for(&mut self, track: &Track) {
        self.open = true;
        self.track_id = track.id().to_owned();
        self.track_duration_seconds = track.duration_seconds;
        self.fields = SongEditorFields::from_track(track);
        self.tab = SongEditorTab::TagsAndSpecs;
        self.artwork_compliance = None;
        self.online_candidates.clear();
        self.lyrics_candidate = None;
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}

/// Renders the sheet if [`SongEditorState::open`]; a no-op otherwise. Call once per frame.
pub fn render(ctx: &egui::Context, state: &mut SongEditorState) {
    if !state.open {
        return;
    }
    let track_id = state.track_id.clone();
    crate::sheets::modal(ctx, "song_metadata_editor_sheet", SHEET_SIZE, |ui| {
        ui.label(egui::RichText::new("Edit Song Metadata").font(font::title2()).color(color::SILVER_LIGHT));
        ui.add_space(10.0);
        render_tabs(ui, state);
        ui.add_space(10.0);

        egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(SHEET_SIZE.y - 180.0).show(ui, |ui| {
            match state.tab {
                SongEditorTab::TagsAndSpecs => render_tags_and_specs(ui, &track_id, state),
                SongEditorTab::AlbumArtwork => render_artwork_tab(ui, &track_id, state),
            }
        });

        ui.add_space(16.0);
        render_footer(ui, &track_id, state);
    });
}

fn render_tabs(ui: &mut egui::Ui, state: &mut SongEditorState) {
    ui.horizontal(|ui| {
        for (tab, label) in [(SongEditorTab::TagsAndSpecs, "Tags & Specs"), (SongEditorTab::AlbumArtwork, "Album Artwork")] {
            let variant = if state.tab == tab { button::Variant::Capsule } else { button::Variant::Glass };
            if ui.add(button::Button::new(label, variant)).clicked() {
                state.tab = tab;
            }
        }
    });
}

fn render_tags_and_specs(ui: &mut egui::Ui, track_id: &str, state: &mut SongEditorState) {
    text_field(ui, "Title", &mut state.fields.title);
    text_field(ui, "Artist", &mut state.fields.artist);
    text_field(ui, "Album", &mut state.fields.album);
    ui.columns(2, |cols| {
        text_field(&mut cols[0], "Track Number", &mut state.fields.track_number);
        text_field(&mut cols[1], "Disc Number", &mut state.fields.disc_number);
    });
    ui.columns(2, |cols| {
        text_field(&mut cols[0], "Year", &mut state.fields.year);
        text_field(&mut cols[1], "Genre", &mut state.fields.genre);
    });

    ui.label(egui::RichText::new("Lyrics").font(font::caption()).color(color::SILVER_MUTED));
    ui.add(egui::TextEdit::multiline(&mut state.fields.lyrics).desired_rows(6).font(font::body()).desired_width(f32::INFINITY));
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        if ui.add(button::Button::new("Search Online Metadata\u{2026}", button::Variant::Glass)).clicked() {
            state.commands.push(SongEditorCommand::SearchOnlineMetadata {
                track_id: track_id.to_owned(),
                artist: state.fields.artist.clone(),
                title: state.fields.title.clone(),
            });
        }
        ui.add_space(8.0);
        if ui.add(button::Button::new("Find Lyrics\u{2026}", button::Variant::Glass)).clicked() {
            state.commands.push(SongEditorCommand::SearchLyrics {
                track_id: track_id.to_owned(),
                artist: state.fields.artist.clone(),
                title: state.fields.title.clone(),
                album: state.fields.album.clone(),
                duration_seconds: state.track_duration_seconds,
            });
        }
    });

    if !state.online_candidates.is_empty() {
        ui.add_space(10.0);
        ui.label(egui::RichText::new("Online Matches").font(font::caption_bold()).color(color::SILVER_MUTED));
        for candidate in state.online_candidates.clone() {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{} \u{2014} {} ({})", candidate.title, candidate.artist, candidate.album))
                        .font(font::callout())
                        .color(color::SILVER_LIGHT),
                );
                if ui.add(button::Button::new("Apply", button::Variant::Glass)).clicked() {
                    state.fields.apply_candidate(&candidate);
                }
            });
        }
    }

    if let Some(lyrics) = state.lyrics_candidate.clone() {
        ui.add_space(10.0);
        ui.label(egui::RichText::new("Lyrics Match").font(font::caption_bold()).color(color::SILVER_MUTED));
        ui.label(egui::RichText::new(&lyrics).font(font::mono_small()).color(color::SILVER_LIGHT));
        if ui.add(button::Button::new("Apply to Lyrics Field", button::Variant::Glass)).clicked() {
            state.fields.lyrics = lyrics;
        }
    }
}

fn render_artwork_tab(ui: &mut egui::Ui, track_id: &str, state: &mut SongEditorState) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(200.0, 200.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, egui::Rounding::same(10.0), color::INPUT);
        ui.painter().rect_stroke(rect, egui::Rounding::same(10.0), egui::Stroke::new(1.0, glass::white_alpha(0.15)));
        let galley = ui.painter().layout_no_wrap("\u{266A}".to_owned(), egui::FontId::proportional(40.0), color::SILVER_MUTED);
        ui.painter().galley(rect.center() - galley.size() * 0.5, galley, color::SILVER_MUTED);

        ui.add_space(16.0);
        ui.vertical(|ui| {
            if ui.add(button::Button::new("Import Artwork\u{2026}", button::Variant::Prominent)).clicked() {
                state.commands.push(SongEditorCommand::ImportArtworkFromFile(track_id.to_owned()));
            }
            ui.add_space(6.0);
            if ui.add(button::Button::new("Export to PC\u{2026}", button::Variant::Glass)).clicked() {
                state.commands.push(SongEditorCommand::ExportArtworkToPc(track_id.to_owned()));
            }
            ui.add_space(12.0);
            render_compliance_badges(ui, state.artwork_compliance);
        });
    });
}

fn render_compliance_badges(ui: &mut egui::Ui, compliance: Option<ArtworkCompliance>) {
    match compliance {
        None => {
            ui.label(egui::RichText::new("No artwork loaded").font(font::caption()).color(color::SILVER_MUTED));
        }
        Some(compliance) => {
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
    }
}

fn render_footer(ui: &mut egui::Ui, track_id: &str, state: &mut SongEditorState) {
    ui.horizontal(|ui| {
        if ui.add(button::Button::new("Save", button::Variant::Prominent)).clicked() {
            state.commands.push(SongEditorCommand::Save { track_id: track_id.to_owned(), fields: state.fields.clone() });
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

    fn sample_track() -> Track {
        Track {
            filename: "song.flac".into(),
            relative_path: "Artist/Album/song.flac".into(),
            full_path: "D:/Music/Artist/Album/song.flac".into(),
            format: "FLAC".into(),
            title: "Song Title".into(),
            artist: "The Artist".into(),
            album: "The Album".into(),
            track_num: "3".into(),
            year: "2021".into(),
            genre: "Rock".into(),
            duration_seconds: 245.0,
            bitrate_kbps: 900,
            sample_rate: 96_000,
            bits_per_sample: 24,
            has_artwork: true,
            size_bytes: 25_000_000,
            date_added: Utc::now(),
            is_senseme_analyzed: false,
            senseme_bpm: None,
            senseme_valence: None,
            senseme_arousal: None,
            senseme_channels: Vec::new(),
            senseme_highlight_sec: None,
        }
    }

    // ---- Working-copy lifecycle ----

    #[test]
    fn open_for_seeds_fields_from_the_track_with_an_empty_disc_number() {
        let track = sample_track();
        let mut state = SongEditorState::default();
        state.open_for(&track);

        assert!(state.open);
        assert_eq!(state.track_id, track.full_path);
        assert_eq!(state.fields.title, "Song Title");
        assert_eq!(state.fields.artist, "The Artist");
        assert_eq!(state.fields.album, "The Album");
        assert_eq!(state.fields.track_number, "3");
        assert_eq!(state.fields.disc_number, ""); // Track has no disc-number field to seed from.
        assert_eq!(state.fields.year, "2021");
        assert_eq!(state.fields.genre, "Rock");
        assert_eq!(state.fields.lyrics, "");
    }

    #[test]
    fn open_for_resets_stale_results_from_a_previous_track() {
        let mut state = SongEditorState::default();
        state.online_candidates.push(MetadataCandidate {
            title: "Stale".into(),
            artist: "Stale".into(),
            album: "Stale".into(),
            year: "1999".into(),
            genre: "Stale".into(),
            cover_art_url: None,
            source: "itunes",
        });
        state.lyrics_candidate = Some("stale lyrics".into());
        state.tab = SongEditorTab::AlbumArtwork;

        state.open_for(&sample_track());

        assert!(state.online_candidates.is_empty());
        assert!(state.lyrics_candidate.is_none());
        assert_eq!(state.tab, SongEditorTab::TagsAndSpecs);
    }

    #[test]
    fn apply_candidate_copies_title_artist_album_year_genre_only() {
        let mut fields = SongEditorFields { lyrics: "kept".into(), disc_number: "2".into(), ..SongEditorFields::default() };
        let candidate = MetadataCandidate {
            title: "New Title".into(),
            artist: "New Artist".into(),
            album: "New Album".into(),
            year: "2020".into(),
            genre: "Pop".into(),
            cover_art_url: Some("https://example.com/art.jpg".into()),
            source: "deezer",
        };
        fields.apply_candidate(&candidate);

        assert_eq!(fields.title, "New Title");
        assert_eq!(fields.artist, "New Artist");
        assert_eq!(fields.album, "New Album");
        assert_eq!(fields.year, "2020");
        assert_eq!(fields.genre, "Pop");
        // Untouched by a candidate (it carries neither).
        assert_eq!(fields.lyrics, "kept");
        assert_eq!(fields.disc_number, "2");
    }

    // ---- Save/Cancel commands ----

    #[test]
    fn close_clears_open_without_touching_fields() {
        let mut state = SongEditorState::default();
        state.open_for(&sample_track());
        state.fields.title = "Edited".into();
        state.close();

        assert!(!state.open);
        assert_eq!(state.fields.title, "Edited");
    }

    // ---- Headless render smoke test ----

    /// Drives the sheet through a headless egui frame while closed, open on each tab, and
    /// with online/lyrics results populated — egui lays out and paints during `Context::run`,
    /// so a bad rect/font/widget would panic here.
    #[test]
    fn song_editor_renders_in_headless_frame_for_every_state() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let track = sample_track();

        let mut closed = SongEditorState::default();
        let mut tags_tab = SongEditorState::default();
        tags_tab.open_for(&track);
        let mut artwork_tab = SongEditorState::default();
        artwork_tab.open_for(&track);
        artwork_tab.tab = SongEditorTab::AlbumArtwork;
        artwork_tab.artwork_compliance = Some(ArtworkCompliance {
            width: 1200,
            height: 1200,
            is_progressive: true,
            color_model: walkman_services::artwork::ColorModel::YCbCrOrRgb,
        });
        let mut with_results = SongEditorState::default();
        with_results.open_for(&track);
        with_results.online_candidates.push(MetadataCandidate {
            title: "Match".into(),
            artist: "Match Artist".into(),
            album: "Match Album".into(),
            year: "2020".into(),
            genre: "Pop".into(),
            cover_art_url: None,
            source: "itunes",
        });
        with_results.lyrics_candidate = Some("La la la".into());

        for state in [&mut closed, &mut tags_tab, &mut artwork_tab, &mut with_results] {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |_ui| {});
                render(ctx, state);
            });
        }
    }
}
