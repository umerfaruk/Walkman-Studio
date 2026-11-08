//! Pure types/logic behind `LibraryView` ("All Songs"), ported from `LibraryView.swift`.
//! The rendering itself lives in `iced_ui::views::library` (the egui `render` this module
//! used to carry was removed once the iced migration reached parity — see Milestone 8 of
//! the egui-to-iced migration plan).

use walkman_domain::models::Track;

/// The table's sortable columns (R5.1's "multi-column sortable track table").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Column {
    #[default]
    Title,
    Artist,
    Album,
    Duration,
    Format,
}

impl Column {
    pub const ALL: [Column; 5] = [Self::Title, Self::Artist, Self::Album, Self::Duration, Self::Format];

    pub fn title(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::Artist => "Artist",
            Self::Album => "Album",
            Self::Duration => "Duration",
            Self::Format => "Format",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortDirection {
    #[default]
    Ascending,
    Descending,
}

/// Sorts `tracks` in place by `column`/`direction`.
pub fn sort_tracks(tracks: &mut [Track], column: Column, direction: SortDirection) {
    match column {
        Column::Title => tracks.sort_by_key(|t| t.title.to_lowercase()),
        Column::Artist => tracks.sort_by_key(|t| t.artist.to_lowercase()),
        Column::Album => tracks.sort_by_key(|t| t.album.to_lowercase()),
        Column::Duration => tracks.sort_by(|a, b| a.duration_seconds.total_cmp(&b.duration_seconds)),
        Column::Format => tracks.sort_by_key(|t| t.format.to_lowercase()),
    }
    if direction == SortDirection::Descending {
        tracks.reverse();
    }
}

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

/// An action the view wants performed (R5.1's context-menu actions + the batch-lyrics button).
#[derive(Debug, Clone, PartialEq)]
pub enum LibraryCommand {
    Play(String),
    EditMetadata(String),
    TrimCut(String),
    Transfer(Vec<String>),
    Delete(Vec<String>),
    BatchLyrics(Vec<String>),
}

#[derive(Debug, Default)]
pub struct LibraryViewState {
    pub sort_column: Column,
    pub sort_direction: SortDirection,
    pub search_query: String,
    pub selected: Vec<String>,
    pub selection_anchor: Option<usize>,
    pub commands: Vec<LibraryCommand>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn track(title: &str, artist: &str, album: &str, duration: f64, format: &str) -> Track {
        Track {
            filename: format!("{title}.flac"),
            relative_path: format!("{artist}/{album}/{title}.flac"),
            full_path: format!("/music/{artist}/{album}/{title}.flac"),
            format: format.into(),
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            track_num: "1".into(),
            year: "2024".into(),
            genre: String::new(),
            duration_seconds: duration,
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
    fn sort_by_title_ascending_and_descending() {
        let mut tracks = [track("Zeta", "A", "Al", 1.0, "FLAC"), track("Alpha", "A", "Al", 1.0, "FLAC")];
        sort_tracks(&mut tracks, Column::Title, SortDirection::Ascending);
        assert_eq!(tracks[0].title, "Alpha");
        sort_tracks(&mut tracks, Column::Title, SortDirection::Descending);
        assert_eq!(tracks[0].title, "Zeta");
    }

    #[test]
    fn sort_by_duration_and_format() {
        let mut tracks =
            [track("A", "X", "Y", 300.0, "MP3"), track("B", "X", "Y", 100.0, "FLAC")];
        sort_tracks(&mut tracks, Column::Duration, SortDirection::Ascending);
        assert_eq!(tracks[0].title, "B");
        sort_tracks(&mut tracks, Column::Format, SortDirection::Ascending);
        assert_eq!(tracks[0].format, "FLAC");
    }

    #[test]
    fn filter_matches_across_title_artist_album() {
        let tracks = [track("Dawn", "Nova", "Horizon", 1.0, "FLAC"), track("Dusk", "Echo", "Reverb", 1.0, "FLAC")];
        assert_eq!(filter_tracks(&tracks, "nova").len(), 1);
        assert_eq!(filter_tracks(&tracks, "reverb").len(), 1);
        assert_eq!(filter_tracks(&tracks, "").len(), 2);
    }
}
