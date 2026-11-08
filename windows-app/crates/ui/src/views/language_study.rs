//! Pure types/logic behind `LanguageStudyView`, ported from `LanguageStudyView.swift`.
//! The rendering itself lives in `iced_ui::views::language_study` (the egui `render`
//! this module used to carry was removed once the iced migration reached parity — see
//! Milestone 8 of the egui-to-iced migration plan).

use walkman_domain::models::Track;

/// R5.6's five display modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LanguageDisplayMode {
    #[default]
    Tracks,
    Albums,
    Artists,
    RecentlyAdded,
    Folders,
}

impl LanguageDisplayMode {
    pub const ALL: [LanguageDisplayMode; 5] =
        [Self::Tracks, Self::Albums, Self::Artists, Self::RecentlyAdded, Self::Folders];

    pub fn title(self) -> &'static str {
        match self {
            Self::Tracks => "Tracks",
            Self::Albums => "Albums",
            Self::Artists => "Artists",
            Self::RecentlyAdded => "Recently Added",
            Self::Folders => "Folders",
        }
    }
}

/// R5.6's six sort options ("Speaker/Artist" and "Course/Album" are the language-study
/// terminology for the same `artist`/`album` fields `library.rs`'s table sorts by).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LanguageSortOption {
    #[default]
    TitleAz,
    TitleZa,
    SpeakerArtistAz,
    CourseAlbumAz,
    DurationLongest,
    RecentlyAdded,
}

impl LanguageSortOption {
    pub const ALL: [LanguageSortOption; 6] = [
        Self::TitleAz,
        Self::TitleZa,
        Self::SpeakerArtistAz,
        Self::CourseAlbumAz,
        Self::DurationLongest,
        Self::RecentlyAdded,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::TitleAz => "Title A\u{2013}Z",
            Self::TitleZa => "Title Z\u{2013}A",
            Self::SpeakerArtistAz => "Speaker/Artist A\u{2013}Z",
            Self::CourseAlbumAz => "Course/Album A\u{2013}Z",
            Self::DurationLongest => "Duration (Longest First)",
            Self::RecentlyAdded => "Recently Added (First)",
        }
    }
}

/// R5.6's filter: reuses the real `Track::is_language_study()` (checks `/LEARNING/` path or a
/// language/audiobook/podcast/learning genre).
pub fn language_study_tracks(tracks: &[Track]) -> Vec<Track> {
    tracks.iter().filter(|t| t.is_language_study()).cloned().collect()
}

pub fn sort_tracks(tracks: &mut [Track], option: LanguageSortOption) {
    match option {
        LanguageSortOption::TitleAz => tracks.sort_by_key(|t| t.title.to_lowercase()),
        LanguageSortOption::TitleZa => tracks.sort_by_key(|t| std::cmp::Reverse(t.title.to_lowercase())),
        LanguageSortOption::SpeakerArtistAz => tracks.sort_by_key(|t| t.artist.to_lowercase()),
        LanguageSortOption::CourseAlbumAz => tracks.sort_by_key(|t| t.album.to_lowercase()),
        LanguageSortOption::DurationLongest => tracks.sort_by(|a, b| b.duration_seconds.total_cmp(&a.duration_seconds)),
        LanguageSortOption::RecentlyAdded => tracks.sort_by_key(|t| std::cmp::Reverse(t.date_added)),
    }
}

/// One row in the Folders display mode: tracks grouped by their immediate parent directory.
#[derive(Debug, Clone, PartialEq)]
pub struct FolderGroup {
    pub folder_name: String,
    pub track_count: usize,
}

pub fn group_by_folder(tracks: &[Track]) -> Vec<FolderGroup> {
    use std::collections::BTreeMap;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for t in tracks {
        let folder = std::path::Path::new(&t.full_path)
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Unknown".to_string());
        *counts.entry(folder).or_insert(0) += 1;
    }
    counts.into_iter().map(|(folder_name, track_count)| FolderGroup { folder_name, track_count }).collect()
}

#[derive(Debug, Clone, PartialEq)]
pub enum LanguageStudyCommand {
    Play(String),
    EditMetadata(String),
    TrimCut(String),
    Transfer(Vec<String>),
    Delete(Vec<String>),
    BatchLyrics(Vec<String>),
}

#[derive(Debug, Default)]
pub struct LanguageStudyViewState {
    pub display_mode: LanguageDisplayMode,
    pub sort: LanguageSortOption,
    pub selected: Vec<String>,
    pub selection_anchor: Option<usize>,
    pub commands: Vec<LanguageStudyCommand>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn track(path: &str, title: &str, artist: &str, album: &str, genre: &str, duration: f64) -> Track {
        Track {
            filename: title.to_string(),
            relative_path: path.to_string(),
            full_path: path.to_string(),
            format: "MP3".into(),
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            track_num: "1".into(),
            year: "2024".into(),
            genre: genre.into(),
            duration_seconds: duration,
            bitrate_kbps: 128,
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
    fn language_study_tracks_filters_by_path_or_genre() {
        let tracks = [
            track("/LEARNING/French/lesson1.mp3", "Lesson 1", "Tutor", "French 101", "", 300.0),
            track("/Music/song.mp3", "Song", "Band", "Album", "Rock", 200.0),
            track("/Music/book.mp3", "Chapter 1", "Author", "Book", "Audiobook", 1800.0),
        ];
        let filtered = language_study_tracks(&tracks);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().any(|t| t.title == "Lesson 1"));
        assert!(filtered.iter().any(|t| t.title == "Chapter 1"));
    }

    #[test]
    fn sort_by_duration_longest_first() {
        let mut tracks = [
            track("/LEARNING/a.mp3", "Short", "T", "C", "", 100.0),
            track("/LEARNING/b.mp3", "Long", "T", "C", "", 3000.0),
        ];
        sort_tracks(&mut tracks, LanguageSortOption::DurationLongest);
        assert_eq!(tracks[0].title, "Long");
    }

    #[test]
    fn group_by_folder_counts_by_parent_directory() {
        let tracks = [
            track("/LEARNING/French/lesson1.mp3", "L1", "T", "C", "", 1.0),
            track("/LEARNING/French/lesson2.mp3", "L2", "T", "C", "", 1.0),
            track("/LEARNING/Spanish/lesson1.mp3", "L3", "T", "C", "", 1.0),
        ];
        let folders = group_by_folder(&tracks);
        assert_eq!(folders.len(), 2);
        let french = folders.iter().find(|f| f.folder_name == "French").unwrap();
        assert_eq!(french.track_count, 2);
    }
}
