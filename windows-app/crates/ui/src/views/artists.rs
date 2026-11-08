//! Pure types/logic behind `ArtistsView`, ported from `ArtistsView.swift`. The rendering
//! itself lives in `iced_ui::views::artists` (the egui `render` this module used to carry
//! was removed once the iced migration reached parity — see Milestone 8 of the
//! egui-to-iced migration plan).

use walkman_domain::models::{ArtistItem, Track};

/// Groups `tracks` into [`ArtistItem`]s by artist name (case-insensitive), each carrying its
/// own album grouping (reusing `albums::group_into_albums` per-artist).
pub fn group_into_artists(tracks: &[Track]) -> Vec<ArtistItem> {
    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<String, Vec<Track>> = BTreeMap::new();
    for t in tracks {
        buckets.entry(t.artist.to_lowercase()).or_default().push(t.clone());
    }
    let mut artists: Vec<ArtistItem> = buckets
        .into_values()
        .map(|artist_tracks| {
            let name = artist_tracks[0].artist.clone();
            let albums = crate::views::albums::group_into_albums(&artist_tracks);
            let album_count = albums.len();
            let track_count = artist_tracks.len();
            ArtistItem::new(name, album_count, track_count, albums, artist_tracks)
        })
        .collect();
    artists.sort_by_key(|a| a.name.to_lowercase());
    artists
}

pub fn filter_artists<'a>(artists: &'a [ArtistItem], query: &str) -> Vec<&'a ArtistItem> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return artists.iter().collect();
    }
    artists.iter().filter(|a| a.name.to_lowercase().contains(&query)).collect()
}

/// Which level of the artist → albums → tracks drill-down is active.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ArtistDrillDown {
    #[default]
    Grid,
    Albums { artist: String },
    Tracks { artist: String, album_id: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArtistsCommand {
    Play(String),
    EditMetadata(String),
    TrimCut(String),
    Transfer(Vec<String>),
    Delete(Vec<String>),
}

#[derive(Debug, Default)]
pub struct ArtistsViewState {
    pub search_query: String,
    pub drill_down: ArtistDrillDown,
    pub commands: Vec<ArtistsCommand>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn track(title: &str, artist: &str, album: &str) -> Track {
        Track {
            filename: format!("{title}.flac"),
            relative_path: format!("{artist}/{album}/{title}.flac"),
            full_path: format!("/music/{artist}/{album}/{title}.flac"),
            format: "FLAC".into(),
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            track_num: "1".into(),
            year: "2024".into(),
            genre: String::new(),
            duration_seconds: 180.0,
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
    fn group_into_artists_groups_by_name_and_sorts_alphabetically() {
        let tracks = [
            track("A", "Zeta", "Al1"),
            track("B", "Alpha", "Al2"),
            track("C", "Alpha", "Al2"),
        ];
        let artists = group_into_artists(&tracks);
        assert_eq!(artists.len(), 2);
        assert_eq!(artists[0].name, "Alpha");
        assert_eq!(artists[0].track_count, 2);
        assert_eq!(artists[0].album_count, 1);
        assert_eq!(artists[1].name, "Zeta");
    }

    #[test]
    fn filter_artists_matches_case_insensitively() {
        let tracks = [track("A", "Nova", "Al1"), track("B", "Echo", "Al2")];
        let artists = group_into_artists(&tracks);
        assert_eq!(filter_artists(&artists, "nova").len(), 1);
        assert_eq!(filter_artists(&artists, "").len(), 2);
    }
}
