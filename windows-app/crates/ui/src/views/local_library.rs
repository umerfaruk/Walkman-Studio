//! Pure types/logic behind `LocalLibraryView`, ported from `LocalLibraryView.swift`. The
//! rendering itself lives in `iced_ui::views::local_library` (the egui `render` this
//! module used to carry was removed once the iced migration reached parity — see
//! Milestone 8 of the egui-to-iced migration plan).

use std::collections::BTreeMap;

use walkman_domain::models::Track;

/// R4.2's five display modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LocalDisplayMode {
    #[default]
    Songs,
    Albums,
    Artists,
    RecentlyAdded,
    Folders,
}

impl LocalDisplayMode {
    pub const ALL: [LocalDisplayMode; 5] =
        [Self::Songs, Self::Albums, Self::Artists, Self::RecentlyAdded, Self::Folders];

    pub fn title(self) -> &'static str {
        match self {
            Self::Songs => "Songs",
            Self::Albums => "Albums",
            Self::Artists => "Artists",
            Self::RecentlyAdded => "Recently Added",
            Self::Folders => "Folders",
        }
    }
}

/// R4.3's eight sort options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LocalSortOption {
    #[default]
    TitleAz,
    TitleZa,
    ArtistAz,
    AlbumAz,
    DateAddedNewest,
    DateAddedOldest,
    DurationLongest,
    AudioQuality,
}

impl LocalSortOption {
    pub const ALL: [LocalSortOption; 8] = [
        Self::TitleAz,
        Self::TitleZa,
        Self::ArtistAz,
        Self::AlbumAz,
        Self::DateAddedNewest,
        Self::DateAddedOldest,
        Self::DurationLongest,
        Self::AudioQuality,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::TitleAz => "Title A\u{2013}Z",
            Self::TitleZa => "Title Z\u{2013}A",
            Self::ArtistAz => "Artist A\u{2013}Z",
            Self::AlbumAz => "Album A\u{2013}Z",
            Self::DateAddedNewest => "Date Added (Newest)",
            Self::DateAddedOldest => "Date Added (Oldest)",
            Self::DurationLongest => "Duration (Longest)",
            Self::AudioQuality => "Audio Quality",
        }
    }
}

/// Sorts `tracks` in place per `option` (R4.3).
pub fn sort_tracks(tracks: &mut [Track], option: LocalSortOption) {
    match option {
        LocalSortOption::TitleAz => tracks.sort_by_key(|t| t.title.to_lowercase()),
        LocalSortOption::TitleZa => tracks.sort_by_key(|t| std::cmp::Reverse(t.title.to_lowercase())),
        LocalSortOption::ArtistAz => tracks.sort_by_key(|t| t.artist.to_lowercase()),
        LocalSortOption::AlbumAz => tracks.sort_by_key(|t| t.album.to_lowercase()),
        LocalSortOption::DateAddedNewest => tracks.sort_by_key(|t| std::cmp::Reverse(t.date_added)),
        LocalSortOption::DateAddedOldest => tracks.sort_by_key(|t| t.date_added),
        LocalSortOption::DurationLongest => {
            tracks.sort_by(|a, b| b.duration_seconds.total_cmp(&a.duration_seconds));
        }
        LocalSortOption::AudioQuality => tracks.sort_by_key(|t| std::cmp::Reverse(t.bitrate_kbps)),
    }
}

/// Case/whitespace-insensitive substring match against title/artist/album (R4.4's search bar).
pub fn filter_tracks<'a>(tracks: &'a [Track], query: &str) -> Vec<&'a Track> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return tracks.iter().collect();
    }
    tracks
        .iter()
        .filter(|t| {
            t.title.to_lowercase().contains(&query)
                || t.artist.to_lowercase().contains(&query)
                || t.album.to_lowercase().contains(&query)
        })
        .collect()
}

/// One row in the Albums display mode: an `(album, artist)` grouping of the filtered tracks.
#[derive(Debug, Clone, PartialEq)]
pub struct AlbumGroup {
    pub album: String,
    pub artist: String,
    pub track_count: usize,
}

pub fn group_by_album(tracks: &[&Track]) -> Vec<AlbumGroup> {
    let mut counts: BTreeMap<(String, String), (String, String, usize)> = BTreeMap::new();
    for t in tracks {
        let key = (t.album.to_lowercase(), t.artist.to_lowercase());
        let entry = counts.entry(key).or_insert_with(|| (t.album.clone(), t.artist.clone(), 0));
        entry.2 += 1;
    }
    let mut groups: Vec<AlbumGroup> = counts
        .into_values()
        .map(|(album, artist, track_count)| AlbumGroup { album, artist, track_count })
        .collect();
    groups.sort_by_key(|g| g.album.to_lowercase());
    groups
}

/// One row in the Artists display mode.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtistGroup {
    pub artist: String,
    pub track_count: usize,
}

pub fn group_by_artist(tracks: &[&Track]) -> Vec<ArtistGroup> {
    let mut counts: BTreeMap<String, (String, usize)> = BTreeMap::new();
    for t in tracks {
        let entry = counts.entry(t.artist.to_lowercase()).or_insert_with(|| (t.artist.clone(), 0));
        entry.1 += 1;
    }
    let mut groups: Vec<ArtistGroup> =
        counts.into_values().map(|(artist, track_count)| ArtistGroup { artist, track_count }).collect();
    groups.sort_by_key(|g| g.artist.to_lowercase());
    groups
}

/// Which modal sheet (if any) is open (R4.1/4.5/4.6).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum LocalLibrarySheet {
    #[default]
    None,
    ManageFolders,
    DirectTransfer { track_ids: Vec<String> },
    FormatWarning { track_ids: Vec<String> },
    RemoveFolderConfirm { folder_id: String, folder_name: String },
    RemoveMissingConfirm,
    AudioNotFound { path: String },
}

/// An action the view wants performed, pushed to [`LocalLibraryViewState::commands`] for the
/// owner of the state to drain.
#[derive(Debug, Clone, PartialEq)]
pub enum LocalLibraryCommand {
    AddFolder,
    RemoveFolder(String),
    RelocateFolder(String),
    RescanFolder(String),
    RemoveMissingFoldersAndTracks,
    /// Transfer & Convert (All) — the format-warning alert's escalation action (R4.5).
    TransferAndConvertAll(Vec<String>),
    /// A plain direct transfer with no format warning needed.
    DirectTransfer(Vec<String>),
    OpenMetadataEditor(String),
    OpenTrimCut(String),
    Delete(Vec<String>),
}

/// This view's own UI state.
#[derive(Debug, Default)]
pub struct LocalLibraryViewState {
    pub display_mode: LocalDisplayMode,
    pub sort: LocalSortOption,
    pub search_query: String,
    pub selected: std::collections::HashSet<String>,
    pub sheet: LocalLibrarySheet,
    /// Actions emitted this/last frame; drained by the state's owner.
    pub commands: Vec<LocalLibraryCommand>,
}

/// Opens the direct-transfer flow for `track_ids`, routing through the format-warning alert
/// first if the paths suggest a format a Walkman wouldn't accept as-is (R4.5). This view has no
/// device/format context of its own (that's `transfer_queue::is_format_supported`, task 10.2's
/// job, invoked once real device state exists) — it approximates with a simple extension check
/// so the sheet flow itself is exercisable before that wiring lands.
pub fn open_direct_transfer(state: &mut LocalLibraryViewState, track_ids: Vec<String>) {
    let any_uncommon = track_ids.iter().any(|p| {
        let ext = std::path::Path::new(p).extension().and_then(|e| e.to_str()).unwrap_or("").to_uppercase();
        !matches!(ext.as_str(), "MP3" | "FLAC" | "WAV" | "AAC" | "M4A" | "ALAC")
    });
    state.sheet = if any_uncommon {
        LocalLibrarySheet::FormatWarning { track_ids }
    } else {
        LocalLibrarySheet::DirectTransfer { track_ids }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn track(title: &str, artist: &str, album: &str, bits: i32, rate: i32, bitrate: i32) -> Track {
        Track {
            filename: format!("{title}.flac"),
            relative_path: format!("{artist}/{album}/{title}.flac"),
            full_path: format!("/local/{artist}/{album}/{title}.flac"),
            format: "FLAC".into(),
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            track_num: "1".into(),
            year: "2024".into(),
            genre: String::new(),
            duration_seconds: 180.0,
            bitrate_kbps: bitrate,
            sample_rate: rate,
            bits_per_sample: bits,
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
    fn filter_tracks_matches_title_artist_or_album_case_insensitively() {
        let tracks = vec![
            track("Dawn", "Nova", "Horizon", 16, 44_100, 900),
            track("Dusk", "Echo", "Reverb", 16, 44_100, 900),
        ];
        assert_eq!(filter_tracks(&tracks, "nova").len(), 1);
        assert_eq!(filter_tracks(&tracks, "REVERB").len(), 1);
        assert_eq!(filter_tracks(&tracks, "").len(), 2);
        assert_eq!(filter_tracks(&tracks, "nonexistent").len(), 0);
    }

    #[test]
    fn sort_tracks_title_az_and_za() {
        let mut tracks = vec![
            track("Zeta", "A", "Al", 16, 44_100, 900),
            track("Alpha", "A", "Al", 16, 44_100, 900),
        ];
        sort_tracks(&mut tracks, LocalSortOption::TitleAz);
        assert_eq!(tracks[0].title, "Alpha");
        sort_tracks(&mut tracks, LocalSortOption::TitleZa);
        assert_eq!(tracks[0].title, "Zeta");
    }

    #[test]
    fn sort_tracks_by_date_added_newest_and_oldest() {
        let mut tracks = vec![
            track("A", "X", "Y", 16, 44_100, 900),
            track("B", "X", "Y", 16, 44_100, 900),
        ];
        tracks[0].date_added = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        tracks[1].date_added = Utc.with_ymd_and_hms(2023, 1, 1, 0, 0, 0).unwrap();

        sort_tracks(&mut tracks, LocalSortOption::DateAddedNewest);
        assert_eq!(tracks[0].title, "B");
        sort_tracks(&mut tracks, LocalSortOption::DateAddedOldest);
        assert_eq!(tracks[0].title, "A");
    }

    #[test]
    fn sort_tracks_by_audio_quality_prefers_higher_bitrate() {
        let mut tracks = vec![
            track("Low", "X", "Y", 16, 44_100, 128),
            track("High", "X", "Y", 24, 96_000, 900),
        ];
        sort_tracks(&mut tracks, LocalSortOption::AudioQuality);
        assert_eq!(tracks[0].title, "High");
    }

    #[test]
    fn group_by_album_counts_tracks_per_album_artist_pair() {
        let tracks = [
            track("A", "Nova", "Horizon", 16, 44_100, 900),
            track("B", "Nova", "Horizon", 16, 44_100, 900),
            track("C", "Echo", "Reverb", 16, 44_100, 900),
        ];
        let refs: Vec<&Track> = tracks.iter().collect();
        let groups = group_by_album(&refs);
        assert_eq!(groups.len(), 2);
        let horizon = groups.iter().find(|g| g.album == "Horizon").unwrap();
        assert_eq!(horizon.track_count, 2);
        assert_eq!(horizon.artist, "Nova");
    }

    #[test]
    fn group_by_artist_counts_tracks_per_artist() {
        let tracks = [
            track("A", "Nova", "Horizon", 16, 44_100, 900),
            track("B", "Nova", "Echoes", 16, 44_100, 900),
        ];
        let refs: Vec<&Track> = tracks.iter().collect();
        let groups = group_by_artist(&refs);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].track_count, 2);
    }

    #[test]
    fn open_direct_transfer_routes_through_format_warning_for_uncommon_extensions() {
        let mut state = LocalLibraryViewState::default();
        open_direct_transfer(&mut state, vec!["/music/song.ogg".to_string()]);
        assert!(matches!(state.sheet, LocalLibrarySheet::FormatWarning { .. }));

        let mut state2 = LocalLibraryViewState::default();
        open_direct_transfer(&mut state2, vec!["/music/song.flac".to_string()]);
        assert!(matches!(state2.sheet, LocalLibrarySheet::DirectTransfer { .. }));
    }
}
