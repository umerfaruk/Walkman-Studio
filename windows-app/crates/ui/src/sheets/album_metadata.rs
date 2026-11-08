//! Pure types/logic behind `AlbumMetadataEditorSheet`, ported from
//! `Sources/WalkmanStudio/Views/AlbumMetadataEditorSheet.swift`. The rendering itself
//! lives in `iced_ui::sheets::album_metadata` (the egui `render` this module used to
//! carry was removed once the iced migration reached parity — see Milestone 8 of the
//! egui-to-iced migration plan).

use walkman_domain::models::Track;
use walkman_services::artwork::ArtworkCompliance;

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
    /// Compliance facts for the unified artwork staged for this album, if any.
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
}
