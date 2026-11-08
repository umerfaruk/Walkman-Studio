//! Pure types/logic behind `AlbumsView`, ported from `AlbumsView.swift`. The rendering
//! itself lives in `iced_ui::views::albums` (the egui `render` this module used to carry
//! was removed once the iced migration reached parity — see Milestone 8 of the
//! egui-to-iced migration plan).

use walkman_domain::models::{AlbumItem, Track};

/// R5.3's seven sort options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlbumSortOption {
    #[default]
    RecentlyAdded,
    NameAz,
    NameZa,
    ArtistAz,
    ReleaseYearNewest,
    ReleaseYearOldest,
    TrackCountMost,
}

impl AlbumSortOption {
    pub const ALL: [AlbumSortOption; 7] = [
        Self::RecentlyAdded,
        Self::NameAz,
        Self::NameZa,
        Self::ArtistAz,
        Self::ReleaseYearNewest,
        Self::ReleaseYearOldest,
        Self::TrackCountMost,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::RecentlyAdded => "Recently Added",
            Self::NameAz => "Album Name A\u{2013}Z",
            Self::NameZa => "Album Name Z\u{2013}A",
            Self::ArtistAz => "Artist A\u{2013}Z",
            Self::ReleaseYearNewest => "Release Year (Newest)",
            Self::ReleaseYearOldest => "Release Year (Oldest)",
            Self::TrackCountMost => "Track Count (Most)",
        }
    }
}

/// Groups `tracks` into [`AlbumItem`]s by `(name, artist)`, matching the Swift grouping
/// (`AlbumItem.id` is `"{name}|{artist}"` lowercased).
pub fn group_into_albums(tracks: &[Track]) -> Vec<AlbumItem> {
    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<(String, String), Vec<Track>> = BTreeMap::new();
    for t in tracks {
        buckets.entry((t.album.to_lowercase(), t.artist.to_lowercase())).or_default().push(t.clone());
    }
    buckets
        .into_values()
        .map(|group| {
            let first = &group[0];
            AlbumItem::new(first.album.clone(), first.artist.clone(), first.year.clone(), group.len(), group)
        })
        .collect()
}

pub fn sort_albums(albums: &mut [AlbumItem], option: AlbumSortOption) {
    match option {
        AlbumSortOption::RecentlyAdded => albums.sort_by_key(|a| std::cmp::Reverse(a.latest_date_added)),
        AlbumSortOption::NameAz => albums.sort_by_key(|a| a.name.to_lowercase()),
        AlbumSortOption::NameZa => albums.sort_by_key(|a| std::cmp::Reverse(a.name.to_lowercase())),
        AlbumSortOption::ArtistAz => albums.sort_by_key(|a| a.artist.to_lowercase()),
        AlbumSortOption::ReleaseYearNewest => albums.sort_by_key(|a| std::cmp::Reverse(a.year.clone())),
        AlbumSortOption::ReleaseYearOldest => albums.sort_by_key(|a| a.year.clone()),
        AlbumSortOption::TrackCountMost => albums.sort_by_key(|a| std::cmp::Reverse(a.track_count)),
    }
}

/// Which format chip is active; `All` shows every album regardless of format.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum FormatFilter {
    #[default]
    All,
    Format(String),
}

/// Applies the format chip + Hi-Res-only toggle + search query to `albums`.
pub fn filter_albums<'a>(albums: &'a [AlbumItem], format: &FormatFilter, hi_res_only: bool, query: &str) -> Vec<&'a AlbumItem> {
    let query = query.trim().to_lowercase();
    albums
        .iter()
        .filter(|a| match format {
            FormatFilter::All => true,
            FormatFilter::Format(f) => a.primary_format().eq_ignore_ascii_case(f),
        })
        .filter(|a| !hi_res_only || a.is_hi_res)
        .filter(|a| query.is_empty() || a.name.to_lowercase().contains(&query) || a.artist.to_lowercase().contains(&query))
        .collect()
}

/// The distinct formats present across `albums`, for the format filter chips.
pub fn distinct_formats(albums: &[AlbumItem]) -> Vec<String> {
    let mut formats: Vec<String> = albums.iter().map(|a| a.primary_format()).collect();
    formats.sort();
    formats.dedup();
    formats
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlbumsCommand {
    Play(String),
    EditAlbumMetadata(String),
    EditTrackMetadata(String),
    TrimCut(String),
    Transfer(Vec<String>),
    Delete(Vec<String>),
    BatchLyrics(Vec<String>),
}

#[derive(Debug, Default)]
pub struct AlbumsViewState {
    pub sort: AlbumSortOption,
    pub format_filter: FormatFilter,
    pub hi_res_only: bool,
    pub search_expanded: bool,
    pub search_query: String,
    /// The album id (`AlbumItem::id()`) currently drilled into, if any.
    pub drilled_into: Option<String>,
    pub selected: Vec<String>,
    pub selection_anchor: Option<usize>,
    pub commands: Vec<AlbumsCommand>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn track(title: &str, artist: &str, album: &str, year: &str, format: &str, bits: i32, rate: i32) -> Track {
        Track {
            filename: format!("{title}.flac"),
            relative_path: format!("{artist}/{album}/{title}.flac"),
            full_path: format!("/music/{artist}/{album}/{title}.flac"),
            format: format.into(),
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            track_num: "1".into(),
            year: year.into(),
            genre: String::new(),
            duration_seconds: 180.0,
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
            senseme_channels: Vec::new(),
            senseme_highlight_sec: None,
        }
    }

    #[test]
    fn group_into_albums_groups_by_name_and_artist() {
        let tracks = [
            track("A", "Nova", "Horizon", "2024", "FLAC", 16, 44_100),
            track("B", "Nova", "Horizon", "2024", "FLAC", 16, 44_100),
            track("C", "Echo", "Reverb", "2021", "MP3", 16, 44_100),
        ];
        let albums = group_into_albums(&tracks);
        assert_eq!(albums.len(), 2);
        let horizon = albums.iter().find(|a| a.name == "Horizon").unwrap();
        assert_eq!(horizon.track_count, 2);
    }

    #[test]
    fn sort_by_name_az_za_and_track_count() {
        let tracks = [
            track("A", "X", "Zeta", "2024", "FLAC", 16, 44_100),
            track("B", "X", "Alpha", "2024", "FLAC", 16, 44_100),
            track("C", "X", "Alpha", "2024", "FLAC", 16, 44_100),
        ];
        let mut albums = group_into_albums(&tracks);
        sort_albums(&mut albums, AlbumSortOption::NameAz);
        assert_eq!(albums[0].name, "Alpha");
        sort_albums(&mut albums, AlbumSortOption::NameZa);
        assert_eq!(albums[0].name, "Zeta");
        sort_albums(&mut albums, AlbumSortOption::TrackCountMost);
        assert_eq!(albums[0].name, "Alpha");
        assert_eq!(albums[0].track_count, 2);
    }

    #[test]
    fn sort_by_release_year_newest_and_oldest() {
        let tracks = [
            track("A", "X", "Old", "2000", "FLAC", 16, 44_100),
            track("B", "X", "New", "2024", "FLAC", 16, 44_100),
        ];
        let mut albums = group_into_albums(&tracks);
        sort_albums(&mut albums, AlbumSortOption::ReleaseYearNewest);
        assert_eq!(albums[0].name, "New");
        sort_albums(&mut albums, AlbumSortOption::ReleaseYearOldest);
        assert_eq!(albums[0].name, "Old");
    }

    #[test]
    fn sort_by_recently_added_uses_latest_date_added() {
        let mut tracks = [
            track("A", "X", "Old", "2020", "FLAC", 16, 44_100),
            track("B", "X", "New", "2020", "FLAC", 16, 44_100),
        ];
        tracks[0].date_added = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        tracks[1].date_added = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let mut albums = group_into_albums(&tracks);
        sort_albums(&mut albums, AlbumSortOption::RecentlyAdded);
        assert_eq!(albums[0].name, "New");
    }

    #[test]
    fn filter_by_format_hi_res_and_search() {
        let tracks = [
            track("A", "Nova", "Horizon", "2024", "FLAC", 24, 96_000),
            track("B", "Echo", "Reverb", "2021", "MP3", 16, 44_100),
        ];
        let albums = group_into_albums(&tracks);

        let all = filter_albums(&albums, &FormatFilter::All, false, "");
        assert_eq!(all.len(), 2);

        let flac_only = filter_albums(&albums, &FormatFilter::Format("FLAC".into()), false, "");
        assert_eq!(flac_only.len(), 1);
        assert_eq!(flac_only[0].name, "Horizon");

        let hi_res_only = filter_albums(&albums, &FormatFilter::All, true, "");
        assert_eq!(hi_res_only.len(), 1);
        assert_eq!(hi_res_only[0].name, "Horizon");

        let searched = filter_albums(&albums, &FormatFilter::All, false, "reverb");
        assert_eq!(searched.len(), 1);
        assert_eq!(searched[0].name, "Reverb");
    }

    #[test]
    fn distinct_formats_is_sorted_and_deduplicated() {
        let tracks = [
            track("A", "X", "Al1", "2024", "mp3", 16, 44_100),
            track("B", "X", "Al2", "2024", "FLAC", 16, 44_100),
            track("C", "X", "Al3", "2024", "FLAC", 16, 44_100),
        ];
        let albums = group_into_albums(&tracks);
        assert_eq!(distinct_formats(&albums), vec!["FLAC".to_string(), "mp3".to_string()]);
    }
}
