//! `TransferQueueItemEditorSheet`, ported from
//! `Sources/WalkmanStudio/Views/TransferQueueItemEditorSheet.swift`.
//!
//! Spec task **16.4** (requirement 9.1): per-item metadata tabs plus the same integrated
//! online metadata/lyrics fetch as the Song Metadata Editor (task 16.2), opened from a row
//! in the Transfer & Convert queue (task 17.1).
//!
//! ## Honest simplification / sequencing note
//! `TransferQueueView` (task 17.1, which this sheet is opened from) hasn't been built yet —
//! there is no `TransferItem` domain type to read from (only
//! [`walkman_domain::models::TransferStatus`] exists so far). This sheet is written against
//! a local [`TransferItemSummary`] (an id, a status, and the exact same
//! [`crate::sheets::song_metadata::SongEditorFields`] working copy the Song editor already
//! uses — a transfer item's editable fields are the same tag set) rather than a type that
//! doesn't exist yet; task 17.1 is expected to construct a `TransferItemSummary` from
//! whatever `TransferItem` it defines when it opens this sheet.

use eframe::egui;

use walkman_domain::models::TransferStatus;
use walkman_services::online_metadata::MetadataCandidate;

use crate::sheets::song_metadata::SongEditorFields;
use crate::sheets::text_field;
use crate::theme::{button, color, font};

pub const SHEET_SIZE: egui::Vec2 = egui::vec2(720.0, 600.0);

/// What a `TransferQueueView` row hands this sheet when opening it for one item.
#[derive(Debug, Clone, PartialEq)]
pub struct TransferItemSummary {
    pub id: String,
    pub status: TransferStatus,
    pub duration_seconds: f64,
    pub fields: SongEditorFields,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransferItemEditorCommand {
    Save { item_id: String, fields: SongEditorFields },
    SearchOnlineMetadata { item_id: String, artist: String, title: String },
    SearchLyrics { item_id: String, artist: String, title: String, album: String, duration_seconds: f64 },
}

#[derive(Debug, Clone, Default)]
pub struct TransferItemEditorState {
    pub open: bool,
    pub item_id: String,
    pub status: Option<TransferStatus>,
    pub duration_seconds: f64,
    pub fields: SongEditorFields,
    pub online_candidates: Vec<MetadataCandidate>,
    pub lyrics_candidate: Option<String>,
    pub commands: Vec<TransferItemEditorCommand>,
}

impl TransferItemEditorState {
    pub fn open_for(&mut self, item: &TransferItemSummary) {
        self.open = true;
        self.item_id = item.id.clone();
        self.status = Some(item.status.clone());
        self.duration_seconds = item.duration_seconds;
        self.fields = item.fields.clone();
        self.online_candidates.clear();
        self.lyrics_candidate = None;
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}

/// A short status label for the sheet header, matching the queue row's own vocabulary
/// (pending/optimizing/converting/success/skipped/failed).
pub fn status_label(status: &TransferStatus) -> String {
    match status {
        TransferStatus::Pending => "Pending".to_owned(),
        TransferStatus::Optimizing => "Optimizing".to_owned(),
        TransferStatus::Converting => "Converting".to_owned(),
        TransferStatus::Success => "Success".to_owned(),
        TransferStatus::Skipped(reason) => format!("Skipped ({reason})"),
        TransferStatus::Failed(reason) => format!("Failed ({reason})"),
    }
}

/// Renders the sheet if [`TransferItemEditorState::open`]; a no-op otherwise. Call once per
/// frame.
pub fn render(ctx: &egui::Context, state: &mut TransferItemEditorState) {
    if !state.open {
        return;
    }
    let item_id = state.item_id.clone();
    crate::sheets::modal(ctx, "transfer_item_editor_sheet", SHEET_SIZE, |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Edit Transfer Item").font(font::title2()).color(color::SILVER_LIGHT));
            if let Some(status) = &state.status {
                ui.add_space(10.0);
                ui.label(egui::RichText::new(status_label(status)).font(font::caption_bold()).color(color::SILVER_MUTED));
            }
        });
        ui.add_space(10.0);

        egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(SHEET_SIZE.y - 160.0).show(ui, |ui| {
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
            ui.add(egui::TextEdit::multiline(&mut state.fields.lyrics).desired_rows(5).font(font::body()).desired_width(f32::INFINITY));
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.add(button::Button::new("Search Online Metadata\u{2026}", button::Variant::Glass)).clicked() {
                    state.commands.push(TransferItemEditorCommand::SearchOnlineMetadata {
                        item_id: item_id.clone(),
                        artist: state.fields.artist.clone(),
                        title: state.fields.title.clone(),
                    });
                }
                ui.add_space(8.0);
                if ui.add(button::Button::new("Find Lyrics\u{2026}", button::Variant::Glass)).clicked() {
                    state.commands.push(TransferItemEditorCommand::SearchLyrics {
                        item_id: item_id.clone(),
                        artist: state.fields.artist.clone(),
                        title: state.fields.title.clone(),
                        album: state.fields.album.clone(),
                        duration_seconds: state.duration_seconds,
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
        });

        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui.add(button::Button::new("Save", button::Variant::Prominent)).clicked() {
                state.commands.push(TransferItemEditorCommand::Save { item_id: item_id.clone(), fields: state.fields.clone() });
                state.close();
            }
            ui.add_space(8.0);
            if ui.add(button::Button::new("Cancel", button::Variant::Glass)).clicked() {
                state.close();
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_item() -> TransferItemSummary {
        TransferItemSummary {
            id: "item-1".into(),
            status: TransferStatus::Pending,
            duration_seconds: 180.0,
            fields: SongEditorFields {
                title: "Title".into(),
                artist: "Artist".into(),
                album: "Album".into(),
                track_number: "1".into(),
                disc_number: String::new(),
                year: "2024".into(),
                genre: "Pop".into(),
                lyrics: String::new(),
            },
        }
    }

    // ---- Status label ----

    #[test]
    fn status_label_covers_every_variant() {
        assert_eq!(status_label(&TransferStatus::Pending), "Pending");
        assert_eq!(status_label(&TransferStatus::Optimizing), "Optimizing");
        assert_eq!(status_label(&TransferStatus::Converting), "Converting");
        assert_eq!(status_label(&TransferStatus::Success), "Success");
        assert_eq!(status_label(&TransferStatus::Skipped("already on device".into())), "Skipped (already on device)");
        assert_eq!(status_label(&TransferStatus::Failed("decode error".into())), "Failed (decode error)");
    }

    // ---- open_for / lifecycle ----

    #[test]
    fn open_for_seeds_fields_and_status_from_the_item() {
        let mut state = TransferItemEditorState::default();
        state.open_for(&sample_item());

        assert!(state.open);
        assert_eq!(state.item_id, "item-1");
        assert_eq!(state.status, Some(TransferStatus::Pending));
        assert_eq!(state.fields.title, "Title");
    }

    #[test]
    fn open_for_resets_stale_results_from_a_previous_item() {
        let mut state = TransferItemEditorState::default();
        state.online_candidates.push(MetadataCandidate {
            title: "Stale".into(),
            artist: "Stale".into(),
            album: "Stale".into(),
            year: "1999".into(),
            genre: "Stale".into(),
            cover_art_url: None,
            source: "itunes",
        });
        state.lyrics_candidate = Some("stale".into());

        state.open_for(&sample_item());

        assert!(state.online_candidates.is_empty());
        assert!(state.lyrics_candidate.is_none());
    }

    #[test]
    fn close_clears_open_without_touching_fields() {
        let mut state = TransferItemEditorState::default();
        state.open_for(&sample_item());
        state.fields.title = "Edited".into();
        state.close();

        assert!(!state.open);
        assert_eq!(state.fields.title, "Edited");
    }

    // ---- Headless render smoke test ----

    #[test]
    fn transfer_item_editor_renders_in_headless_frame_for_every_state() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);

        let mut closed = TransferItemEditorState::default();
        let mut open = TransferItemEditorState::default();
        open.open_for(&sample_item());
        let mut failed = TransferItemEditorState::default();
        failed.open_for(&TransferItemSummary { status: TransferStatus::Failed("boom".into()), ..sample_item() });
        failed.online_candidates.push(MetadataCandidate {
            title: "Match".into(),
            artist: "Match Artist".into(),
            album: "Match Album".into(),
            year: "2020".into(),
            genre: "Pop".into(),
            cover_art_url: None,
            source: "deezer",
        });
        failed.lyrics_candidate = Some("La la la".into());

        for state in [&mut closed, &mut open, &mut failed] {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |_ui| {});
                render(ctx, state);
            });
        }
    }
}
