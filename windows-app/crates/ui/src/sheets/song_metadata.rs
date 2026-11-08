//! Pure types/logic behind `SongMetadataEditorSheet`, ported from
//! `Sources/WalkmanStudio/Views/SongMetadataEditorSheet.swift`. The rendering itself
//! lives in `iced_ui::sheets::song_metadata` (the egui `render` this module used to
//! carry was removed once the iced migration reached parity — see Milestone 8 of the
//! egui-to-iced migration plan).
//!
//! ## Honest simplifications
//! - **Disc Number**: [`walkman_domain::models::Track`] doesn't carry a disc number (ported
//!   1:1 from the Swift model, which also treats it as writer-only — see
//!   `audio_tagger::TagFields`'s own doc comment), so [`SongEditorFields::from_track`] always
//!   starts it empty; it still round-trips through `Save` into `TagFields::disc_number`.
//! - **Online metadata / lyrics search**: the actual HTTP round trip is `walkman_services`'
//!   job (`online_metadata.rs`/`lyrics.rs`); this module only carries the search command and
//!   whatever candidates/lyrics text comes back in [`SongEditorState::online_candidates`]/
//!   [`SongEditorState::lyrics_candidate`].

use walkman_domain::models::Track;
use walkman_services::artwork::ArtworkCompliance;
use walkman_services::online_metadata::MetadataCandidate;

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
    /// Compliance facts for the artwork currently embedded/staged, if any.
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

    #[test]
    fn close_clears_open_without_touching_fields() {
        let mut state = SongEditorState::default();
        state.open_for(&sample_track());
        state.fields.title = "Edited".into();
        state.close();

        assert!(!state.open);
        assert_eq!(state.fields.title, "Edited");
    }
}
