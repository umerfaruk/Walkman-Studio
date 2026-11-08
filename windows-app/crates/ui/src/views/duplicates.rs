//! Pure types/logic behind `DuplicatesView`, ported from
//! `Sources/WalkmanStudio/Views/DuplicatesView.swift`. The rendering itself lives in
//! `iced_ui::views::duplicates` (the egui `render` this module used to carry was
//! removed once the iced migration reached parity — see Milestone 8 of the
//! egui-to-iced migration plan).

use walkman_services::duplicate_finder::{DuplicateGroup, DuplicateScope, DuplicateSortOption};

/// A stable identity for a group across re-sorts/re-filters within a frame (groups
/// themselves carry no id) — every member's full path, joined, is unique to that exact set
/// of tracks regardless of display order.
pub fn group_key(group: &DuplicateGroup) -> String {
    let mut paths: Vec<&str> = group.tracks.iter().map(|t| t.full_path.as_str()).collect();
    paths.sort_unstable();
    paths.join("\u{1F}")
}

#[derive(Debug, Clone, PartialEq)]
pub enum DuplicatesCommand {
    /// Delete this one track (+ companions), after the confirm sheet.
    DeleteTrack(String),
    /// Keep a group's `best_quality_track`, delete the rest (the full paths listed).
    ResolveGroupKeepBest(Vec<String>),
    /// R10.4's one-click `Auto-Resolve Quality` across every visible group.
    AutoResolveQuality,
    Audition(String),
    RevealInExplorer(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum DuplicatesSheet {
    #[default]
    None,
    DeleteConfirm { track_id: String },
    ResolveGroupConfirm { group_key: String, track_ids: Vec<String> },
    AutoResolveConfirm,
}

#[derive(Debug, Clone)]
pub struct DuplicatesViewState {
    pub scope: DuplicateScope,
    pub sort: DuplicateSortOption,
    pub expanded: Vec<String>,
    pub sheet: DuplicatesSheet,
    pub commands: Vec<DuplicatesCommand>,
}

impl Default for DuplicatesViewState {
    fn default() -> Self {
        Self {
            scope: DuplicateScope::ExactSameAlbum,
            sort: DuplicateSortOption::SpaceRecoverable,
            expanded: Vec::new(),
            sheet: DuplicatesSheet::default(),
            commands: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use walkman_domain::models::Track;
    use walkman_services::duplicate_finder;

    #[allow(clippy::too_many_arguments)]
    fn track(path: &str, title: &str, artist: &str, album: &str, duration: f64, bits: i32, rate: i32, size: i64) -> Track {
        Track {
            filename: path.into(),
            relative_path: path.into(),
            full_path: path.into(),
            format: "FLAC".into(),
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            track_num: "1".into(),
            year: "2024".into(),
            genre: String::new(),
            duration_seconds: duration,
            bitrate_kbps: 900,
            sample_rate: rate,
            bits_per_sample: bits,
            has_artwork: false,
            size_bytes: size,
            date_added: Utc::now(),
            is_senseme_analyzed: false,
            senseme_bpm: None,
            senseme_valence: None,
            senseme_arousal: None,
            senseme_channels: Vec::new(),
            senseme_highlight_sec: None,
        }
    }

    fn sample_group() -> DuplicateGroup {
        let a = track("a.flac", "Song", "Artist", "Album", 200.0, 16, 44_100, 10_000);
        let b = track("b.flac", "Song", "Artist", "Album", 200.0, 24, 96_000, 30_000);
        DuplicateGroup { tracks: vec![a, b], has_quality_diff: true }
    }

    #[test]
    fn group_key_is_stable_regardless_of_member_order() {
        let mut group = sample_group();
        let key1 = group_key(&group);
        group.tracks.reverse();
        let key2 = group_key(&group);
        assert_eq!(key1, key2);
    }

    #[test]
    fn group_key_differs_for_different_groups() {
        let g1 = sample_group();
        let g2 = DuplicateGroup { tracks: vec![track("c.flac", "Other", "X", "Y", 1.0, 16, 44_100, 1)], has_quality_diff: false };
        assert_ne!(group_key(&g1), group_key(&g2));
    }

    #[test]
    fn default_state_starts_on_exact_album_scope_sorted_by_recoverable_space() {
        let state = DuplicatesViewState::default();
        assert_eq!(state.scope, DuplicateScope::ExactSameAlbum);
        assert_eq!(state.sort, DuplicateSortOption::SpaceRecoverable);
        assert_eq!(state.sheet, DuplicatesSheet::None);
    }

    #[test]
    fn sample_group_is_found_by_the_real_duplicate_finder() {
        let tracks = vec![
            track("a.flac", "Song", "Artist", "Album", 200.0, 16, 44_100, 10_000),
            track("b.flac", "Song", "Artist", "Album", 200.5, 24, 96_000, 30_000),
        ];
        let groups = duplicate_finder::find_duplicates(&tracks, DuplicateScope::ExactSameAlbum);
        assert_eq!(groups.len(), 1);
    }
}
