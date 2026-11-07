//! SensMe 12-Tone analysis + channel classification, ported from
//! `Sources/WalkmanStudio/SenseMeEngine.swift`.
//!
//! Spec task 5.4. This module holds the portable math (BPM / valence / arousal /
//! mood-score extraction from mono PCM) and the classification into the 14 official
//! Sony channels, plus the SMFM payload layout. The full analysis implementation is
//! completed in task 5.4; this file establishes the channel catalog and public types
//! so the rest of the workspace can compile and reference them.

/// The 14 official Sony SensMe channels (8 mood + 5 time-of-day) plus the global
/// "Shuffle All Songs" pseudo-channel. Order matches the macOS carousel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SenseMeChannel {
    // Mood channels
    Energetic,
    Relax,
    Upbeat,
    Mellow,
    Emotional,
    Lounge,
    Dance,
    Extreme,
    // Time-of-day channels
    Morning,
    Daytime,
    Evening,
    Night,
    Midnight,
    // Global
    ShuffleAllSongs,
}

impl SenseMeChannel {
    /// All channels in carousel display order.
    pub const ALL: [SenseMeChannel; 14] = [
        SenseMeChannel::Morning,
        SenseMeChannel::Daytime,
        SenseMeChannel::Evening,
        SenseMeChannel::Night,
        SenseMeChannel::Midnight,
        SenseMeChannel::Relax,
        SenseMeChannel::Upbeat,
        SenseMeChannel::Energetic,
        SenseMeChannel::Emotional,
        SenseMeChannel::Lounge,
        SenseMeChannel::Dance,
        SenseMeChannel::Extreme,
        SenseMeChannel::Mellow,
        SenseMeChannel::ShuffleAllSongs,
    ];

    /// Display name, matching the macOS channel labels and cover-art filenames.
    pub fn display_name(self) -> &'static str {
        match self {
            SenseMeChannel::Energetic => "Energetic",
            SenseMeChannel::Relax => "Relax",
            SenseMeChannel::Upbeat => "Upbeat",
            SenseMeChannel::Mellow => "Mellow",
            SenseMeChannel::Emotional => "Emotional",
            SenseMeChannel::Lounge => "Lounge",
            SenseMeChannel::Dance => "Dance",
            SenseMeChannel::Extreme => "Extreme",
            SenseMeChannel::Morning => "Morning",
            SenseMeChannel::Daytime => "Daytime",
            SenseMeChannel::Evening => "Evening",
            SenseMeChannel::Night => "Night",
            SenseMeChannel::Midnight => "Midnight",
            SenseMeChannel::ShuffleAllSongs => "Shuffle all songs",
        }
    }

    pub fn is_time_of_day(self) -> bool {
        matches!(
            self,
            SenseMeChannel::Morning
                | SenseMeChannel::Daytime
                | SenseMeChannel::Evening
                | SenseMeChannel::Night
                | SenseMeChannel::Midnight
        )
    }
}

/// Byte size of the FLAC `APPLICATION` block `SMFM` 12-Tone payload. Injection into
/// FLAC (APPLICATION block code 2) / MP3 (ID3v2.3 GEOB / TXXX) lives in the tagger.
pub const SMFM_PAYLOAD_LEN: usize = 3270;

/// 12-Tone analysis result. Ported from Swift `SenseMeInfo`.
#[derive(Debug, Clone, PartialEq)]
pub struct SenseMeInfo {
    pub is_analyzed: bool,
    pub bpm: f64,
    pub valence: f64,
    pub arousal: f64,
    pub mood_scores: [f64; 10],
    pub channels: Vec<String>,
    pub highlight_sec: f64,
}

impl Default for SenseMeInfo {
    fn default() -> Self {
        SenseMeInfo {
            is_analyzed: false,
            bpm: 0.0,
            valence: 0.0,
            arousal: 0.0,
            mood_scores: [0.0; 10],
            channels: Vec::new(),
            highlight_sec: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fourteen_channels_in_order() {
        assert_eq!(SenseMeChannel::ALL.len(), 14);
        assert_eq!(SenseMeChannel::Morning.display_name(), "Morning");
        assert_eq!(SenseMeChannel::ShuffleAllSongs.display_name(), "Shuffle all songs");
    }

    #[test]
    fn time_of_day_flag() {
        assert!(SenseMeChannel::Night.is_time_of_day());
        assert!(!SenseMeChannel::Dance.is_time_of_day());
    }

    #[test]
    fn smfm_len_is_sony_spec() {
        assert_eq!(SMFM_PAYLOAD_LEN, 3270);
    }
}
