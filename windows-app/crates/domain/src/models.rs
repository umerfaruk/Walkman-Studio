//! Core data models, ported from `Sources/WalkmanStudio/Models.swift`.
//!
//! Spec task 5.1. Full field parity with the Swift structs; derived properties are
//! exposed as methods. Timestamps use `chrono::DateTime<Utc>` in place of `Date`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Which storage partition a drive represents. Ported from Swift `DriveType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DriveType {
    Internal,
    Microsd,
    Local,
}

/// Transfer destination root. Ported from Swift `TransferTargetMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransferTargetMode {
    Music,
    Learning,
}

impl TransferTargetMode {
    pub const ALL: [TransferTargetMode; 2] = [TransferTargetMode::Music, TransferTargetMode::Learning];

    pub fn title(self) -> &'static str {
        match self {
            TransferTargetMode::Music => "Music Library",
            TransferTargetMode::Learning => "Language Study & Audiobooks",
        }
    }

    /// Root folder name on the device.
    pub fn root_folder_name(self) -> &'static str {
        match self {
            TransferTargetMode::Music => "MUSIC",
            TransferTargetMode::Learning => "LEARNING",
        }
    }

    /// SF-Symbol name in the macOS app; mapped to a bundled glyph in the UI layer.
    pub fn icon(self) -> &'static str {
        match self {
            TransferTargetMode::Music => "music.note",
            TransferTargetMode::Learning => "character.book.closed.fill",
        }
    }
}

/// A single audio track. Ported from Swift `Track`. `id` is the full path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub filename: String,
    pub relative_path: String,
    pub full_path: String,
    /// "FLAC", "MP3", "M4A", etc.
    pub format: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track_num: String,
    pub year: String,
    pub genre: String,
    pub duration_seconds: f64,
    pub bitrate_kbps: i32,
    pub sample_rate: i32,
    pub bits_per_sample: i32,
    pub has_artwork: bool,
    pub size_bytes: i64,
    pub date_added: DateTime<Utc>,
    pub is_senseme_analyzed: bool,
    pub senseme_bpm: Option<f64>,
    pub senseme_valence: Option<f64>,
    pub senseme_arousal: Option<f64>,
    pub senseme_channels: Vec<String>,
    pub senseme_highlight_sec: Option<f64>,
}

impl Track {
    /// Stable identity (full path), matching Swift `Track.id`.
    pub fn id(&self) -> &str {
        &self.full_path
    }

    /// Hi-Res = exceeds standard 16-bit / 44.1-48kHz CD quality.
    pub fn is_hi_res(&self) -> bool {
        self.bits_per_sample > 16 || self.sample_rate > 48_000
    }

    /// Belongs to Language Study (under /LEARNING/ or a language/audiobook/podcast genre).
    pub fn is_language_study(&self) -> bool {
        if self.full_path.contains("/LEARNING/") || self.full_path.contains("\\LEARNING\\") {
            return true;
        }
        if self.genre.is_empty() {
            return false;
        }
        let g = self.genre.to_lowercase();
        g.contains("language") || g.contains("audiobook") || g.contains("podcast") || g.contains("learning")
    }

    /// Primary artist (featured/guest artists stripped). Delegates to `artist_parse`.
    pub fn primary_artist(&self) -> String {
        crate::artist_parse::parse_primary_and_featured(&self.artist).primary
    }

    /// Featured/guest artists if present. Delegates to `artist_parse`.
    pub fn featured_artist(&self) -> Option<String> {
        crate::artist_parse::parse_primary_and_featured(&self.artist).featured
    }
}

/// A playlist (`.m3u8`). Ported from Swift `Playlist`. `id` is the full path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    pub name: String,
    pub filename: String,
    pub track_count: usize,
    /// Relative POSIX track paths.
    pub tracks: Vec<String>,
    pub full_path: String,
}

impl Playlist {
    pub fn is_language_study(&self) -> bool {
        self.full_path.contains("/LEARNING/") || self.full_path.contains("\\LEARNING\\")
    }
}

/// Status of a queued transfer item. Ported from Swift `TransferStatus`.
#[derive(Debug, Clone, PartialEq)]
pub enum TransferStatus {
    Pending,
    Optimizing,
    Converting,
    Success,
    Skipped(String),
    Failed(String),
}

// NOTE: The remaining models — `WalkmanDrive`, `WalkmanDevice`, `WalkmanDeviceInfo`,
// `AlbumItem`, `ArtistItem`, `TransferItem`, `AudioFileMetadata`, `LocalMusicFolder` —
// are completed in spec task 5.1. The types above form the compiling spine for the
// workspace bootstrap (task 1).

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(path: &str, bits: i32, rate: i32) -> Track {
        Track {
            filename: "x".into(),
            relative_path: "x".into(),
            full_path: path.into(),
            format: "FLAC".into(),
            title: "t".into(),
            artist: "a".into(),
            album: "al".into(),
            track_num: "1".into(),
            year: "2020".into(),
            genre: String::new(),
            duration_seconds: 10.0,
            bitrate_kbps: 900,
            sample_rate: rate,
            bits_per_sample: bits,
            has_artwork: false,
            size_bytes: 1,
            date_added: Utc::now(),
            is_senseme_analyzed: false,
            senseme_bpm: None,
            senseme_valence: None,
            senseme_arousal: None,
            senseme_channels: vec![],
            senseme_highlight_sec: None,
        }
    }

    #[test]
    fn hi_res_detection() {
        assert!(sample("/a.flac", 24, 96_000).is_hi_res());
        assert!(sample("/a.flac", 16, 96_000).is_hi_res());
        assert!(!sample("/a.flac", 16, 44_100).is_hi_res());
    }

    #[test]
    fn language_study_by_path_and_genre() {
        assert!(sample("/Volumes/WALKMAN/LEARNING/x.mp3", 16, 44_100).is_language_study());
        let mut t = sample("/Music/x.mp3", 16, 44_100);
        t.genre = "Audiobook".into();
        assert!(t.is_language_study());
    }

    #[test]
    fn target_mode_roots() {
        assert_eq!(TransferTargetMode::Music.root_folder_name(), "MUSIC");
        assert_eq!(TransferTargetMode::Learning.root_folder_name(), "LEARNING");
    }
}
