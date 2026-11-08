//! Pure types/logic behind `RecentlyAddedView`, ported from `RecentlyAddedView.swift`.
//! The rendering itself lives in `iced_ui::views::recently_added` (the egui `render` this
//! module used to carry was removed once the iced migration reached parity — see
//! Milestone 8 of the egui-to-iced migration plan).

use chrono::{DateTime, Utc};

use walkman_domain::models::Track;

/// R5.2's four time scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeScope {
    Past24Hours,
    Past7Days,
    Past30Days,
    #[default]
    AllTime,
}

impl TimeScope {
    pub const ALL: [TimeScope; 4] = [Self::Past24Hours, Self::Past7Days, Self::Past30Days, Self::AllTime];

    pub fn title(self) -> &'static str {
        match self {
            Self::Past24Hours => "Past 24 Hours",
            Self::Past7Days => "Past 7 Days",
            Self::Past30Days => "Past 30 Days",
            Self::AllTime => "All Time",
        }
    }

    fn max_age(self) -> Option<chrono::Duration> {
        match self {
            Self::Past24Hours => Some(chrono::Duration::hours(24)),
            Self::Past7Days => Some(chrono::Duration::days(7)),
            Self::Past30Days => Some(chrono::Duration::days(30)),
            Self::AllTime => None,
        }
    }
}

/// Filters `tracks` to `scope`'s window (relative to `now`) and ranks them newest-first.
pub fn ranked_tracks(tracks: &[Track], scope: TimeScope, now: DateTime<Utc>) -> Vec<Track> {
    let mut filtered: Vec<Track> = match scope.max_age() {
        Some(max_age) => tracks.iter().filter(|t| now.signed_duration_since(t.date_added) <= max_age).cloned().collect(),
        None => tracks.to_vec(),
    };
    filtered.sort_by_key(|t| std::cmp::Reverse(t.date_added));
    filtered
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecentlyAddedCommand {
    Play(String),
    EditMetadata(String),
    TrimCut(String),
    Transfer(Vec<String>),
    Delete(Vec<String>),
    BatchLyrics(Vec<String>),
}

#[derive(Debug, Default)]
pub struct RecentlyAddedViewState {
    pub scope: TimeScope,
    pub selected: Vec<String>,
    pub selection_anchor: Option<usize>,
    pub commands: Vec<RecentlyAddedCommand>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track_added(hours_ago: i64) -> Track {
        Track {
            filename: "x".into(),
            relative_path: "x".into(),
            full_path: format!("/music/track-{hours_ago}.flac"),
            format: "FLAC".into(),
            title: format!("Track {hours_ago}"),
            artist: "Artist".into(),
            album: "Album".into(),
            track_num: "1".into(),
            year: "2024".into(),
            genre: String::new(),
            duration_seconds: 180.0,
            bitrate_kbps: 900,
            sample_rate: 44_100,
            bits_per_sample: 16,
            has_artwork: false,
            size_bytes: 1,
            date_added: Utc::now() - chrono::Duration::hours(hours_ago),
            is_senseme_analyzed: false,
            senseme_bpm: None,
            senseme_valence: None,
            senseme_arousal: None,
            senseme_channels: Vec::new(),
            senseme_highlight_sec: None,
        }
    }

    #[test]
    fn past_24_hours_excludes_older_tracks() {
        let now = Utc::now();
        let tracks = [track_added(1), track_added(48)];
        let ranked = ranked_tracks(&tracks, TimeScope::Past24Hours, now);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].title, "Track 1");
    }

    #[test]
    fn past_7_and_30_days_widen_the_window() {
        let now = Utc::now();
        let tracks = [track_added(24 * 5), track_added(24 * 20), track_added(24 * 40)];
        assert_eq!(ranked_tracks(&tracks, TimeScope::Past7Days, now).len(), 1);
        assert_eq!(ranked_tracks(&tracks, TimeScope::Past30Days, now).len(), 2);
    }

    #[test]
    fn all_time_includes_everything_and_ranks_newest_first() {
        let now = Utc::now();
        let tracks = [track_added(100), track_added(1), track_added(50)];
        let ranked = ranked_tracks(&tracks, TimeScope::AllTime, now);
        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].title, "Track 1");
        assert_eq!(ranked[2].title, "Track 100");
    }
}
