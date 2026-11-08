//! Pure types/logic behind `TransferQueueItemEditorSheet`, ported from
//! `Sources/WalkmanStudio/Views/TransferQueueItemEditorSheet.swift`. The rendering
//! itself lives in `iced_ui::sheets::transfer_item` (the egui `render` this module used
//! to carry was removed once the iced migration reached parity — see Milestone 8 of the
//! egui-to-iced migration plan).
//!
//! Opens directly from the real `walkman_domain::models::TransferItem`; its own editable
//! metadata fields are seeded into the same [`crate::sheets::song_metadata::SongEditorFields`]
//! working copy the Song editor uses, since a transfer item's tag set is identical — not a
//! parallel field set.

use walkman_domain::models::{TransferItem, TransferStatus};
use walkman_services::online_metadata::MetadataCandidate;

use crate::sheets::song_metadata::SongEditorFields;

impl SongEditorFields {
    /// Seeds a working copy from a queued [`TransferItem`]'s editable fields (disc number
    /// stays empty — `TransferItem` doesn't carry one either, the same writer-only gap
    /// documented on [`SongEditorFields::from_track`]).
    pub fn from_transfer_item(item: &TransferItem) -> Self {
        Self {
            title: item.title.clone(),
            artist: item.artist.clone(),
            album: item.album.clone(),
            track_number: item.track_number.clone(),
            disc_number: String::new(),
            year: item.year.clone(),
            genre: item.genre.clone(),
            lyrics: item.lyrics.clone().unwrap_or_default(),
        }
    }
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
    pub fn open_for(&mut self, item: &TransferItem) {
        self.open = true;
        self.item_id = item.id.clone();
        self.status = Some(item.status.clone());
        self.duration_seconds = item.duration_seconds;
        self.fields = SongEditorFields::from_transfer_item(item);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn sample_item(id: &str, expanded: bool) -> TransferItem {
        TransferItem {
            id: id.into(),
            source_path: format!("D:/Downloads/{id}.flac"),
            status: TransferStatus::Pending,
            title: format!("Title {id}"),
            artist: "Artist".into(),
            album: "Album".into(),
            track_number: "1".into(),
            year: "2024".into(),
            genre: "Pop".into(),
            duration_seconds: 180.0,
            bitrate_kbps: 900,
            sample_rate: 44_100,
            bits_per_sample: 16,
            format: "FLAC".into(),
            size_bytes: 1,
            artwork_data: None,
            is_artwork_optimized: false,
            lyrics: None,
            is_lyrics_synced: false,
            is_fetching_online: false,
            is_optimizing: false,
            is_expanded: expanded,
            is_duplicate_on_walkman: false,
            duplicate_match_info: None,
            duplicate_matched_track: None,
        }
    }

    #[test]
    fn status_label_covers_every_variant() {
        let mut seen = HashSet::new();
        for status in [
            TransferStatus::Pending,
            TransferStatus::Optimizing,
            TransferStatus::Converting,
            TransferStatus::Success,
            TransferStatus::Skipped("already on device".into()),
            TransferStatus::Failed("decode error".into()),
        ] {
            assert!(seen.insert(status_label(&status)));
        }
    }

    #[test]
    fn open_for_seeds_fields_and_status_from_the_item() {
        let mut state = TransferItemEditorState::default();
        state.open_for(&sample_item("item-1", false));

        assert!(state.open);
        assert_eq!(state.item_id, "item-1");
        assert_eq!(state.status, Some(TransferStatus::Pending));
        assert_eq!(state.fields.title, "Title item-1");
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

        state.open_for(&sample_item("item-1", false));

        assert!(state.online_candidates.is_empty());
        assert!(state.lyrics_candidate.is_none());
    }

    #[test]
    fn close_clears_open_without_touching_fields() {
        let mut state = TransferItemEditorState::default();
        state.open_for(&sample_item("item-1", false));
        state.fields.title = "Edited".into();
        state.close();

        assert!(!state.open);
        assert_eq!(state.fields.title, "Edited");
    }
}
