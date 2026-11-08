//! Pure types/logic behind `HiResAudioView`, ported from `HiResAudioView.swift`. The
//! rendering itself lives in `iced_ui::views::hi_res` (the egui `render` this module
//! used to carry was removed once the iced migration reached parity — see Milestone 8
//! of the egui-to-iced migration plan).

use walkman_domain::models::Track;

use crate::views::albums::AlbumSortOption;

/// R5.5's three view modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HiResViewMode {
    #[default]
    Tracks,
    Albums,
    Artists,
}

impl HiResViewMode {
    pub const ALL: [HiResViewMode; 3] = [Self::Tracks, Self::Albums, Self::Artists];

    pub fn title(self) -> &'static str {
        match self {
            Self::Tracks => "Tracks",
            Self::Albums => "Albums",
            Self::Artists => "Artists",
        }
    }
}

/// R5.5's six track sort options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrackSortOption {
    #[default]
    Default,
    Title,
    Artist,
    Album,
    HighestQuality,
    RecentlyAdded,
}

impl TrackSortOption {
    pub const ALL: [TrackSortOption; 6] =
        [Self::Default, Self::Title, Self::Artist, Self::Album, Self::HighestQuality, Self::RecentlyAdded];

    pub fn title(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Title => "Title",
            Self::Artist => "Artist",
            Self::Album => "Album",
            Self::HighestQuality => "Highest Quality",
            Self::RecentlyAdded => "Recently Added",
        }
    }
}

/// R5.5's "filter to tracks >16-bit or >48kHz".
pub fn hi_res_tracks(tracks: &[Track]) -> Vec<Track> {
    tracks.iter().filter(|t| t.is_hi_res()).cloned().collect()
}

pub fn sort_tracks(tracks: &mut [Track], option: TrackSortOption) {
    match option {
        TrackSortOption::Default => {}
        TrackSortOption::Title => tracks.sort_by_key(|t| t.title.to_lowercase()),
        TrackSortOption::Artist => tracks.sort_by_key(|t| t.artist.to_lowercase()),
        TrackSortOption::Album => tracks.sort_by_key(|t| t.album.to_lowercase()),
        TrackSortOption::HighestQuality => {
            tracks.sort_by_key(|t| std::cmp::Reverse(t.bits_per_sample as i64 * t.sample_rate as i64));
        }
        TrackSortOption::RecentlyAdded => tracks.sort_by_key(|t| std::cmp::Reverse(t.date_added)),
    }
}

/// The format/quality filter chip (R5.5): `All`, a specific format, or `HiRes` isn't needed
/// here (every track in this view is already Hi-Res) — `Format` is the only real narrowing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum QualityFilter {
    #[default]
    All,
    Format(String),
}

pub fn filter_tracks<'a>(tracks: &'a [Track], filter: &QualityFilter, query: &str) -> Vec<&'a Track> {
    let query = query.trim().to_lowercase();
    tracks
        .iter()
        .filter(|t| match filter {
            QualityFilter::All => true,
            QualityFilter::Format(f) => t.format.eq_ignore_ascii_case(f),
        })
        .filter(|t| {
            query.is_empty()
                || t.title.to_lowercase().contains(&query)
                || t.artist.to_lowercase().contains(&query)
                || t.album.to_lowercase().contains(&query)
        })
        .collect()
}

pub fn distinct_formats(tracks: &[Track]) -> Vec<String> {
    let mut formats: Vec<String> = tracks.iter().map(|t| t.format.clone()).collect();
    formats.sort();
    formats.dedup();
    formats
}

#[derive(Debug, Clone, PartialEq)]
pub enum HiResCommand {
    Play(String),
    EditMetadata(String),
    TrimCut(String),
    Transfer(Vec<String>),
    Delete(Vec<String>),
    BatchLyrics(Vec<String>),
}

#[derive(Debug, Default)]
pub struct HiResViewState {
    pub mode: HiResViewMode,
    pub track_sort: TrackSortOption,
    pub album_sort: AlbumSortOption,
    pub quality_filter: QualityFilter,
    pub search_query: String,
    pub selected: Vec<String>,
    pub selection_anchor: Option<usize>,
    pub commands: Vec<HiResCommand>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn track(title: &str, artist: &str, album: &str, bits: i32, rate: i32, format: &str) -> Track {
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
    fn hi_res_tracks_filters_to_over_16_bit_or_over_48khz() {
        let tracks = [
            track("A", "X", "Y", 16, 44_100, "MP3"),
            track("B", "X", "Y", 24, 48_000, "FLAC"),
            track("C", "X", "Y", 16, 96_000, "FLAC"),
        ];
        let hi_res = hi_res_tracks(&tracks);
        assert_eq!(hi_res.len(), 2);
        assert!(hi_res.iter().all(|t| t.title != "A"));
    }

    #[test]
    fn sort_by_highest_quality_prefers_bits_times_rate() {
        let mut tracks = [
            track("Low", "X", "Y", 24, 48_000, "FLAC"),
            track("High", "X", "Y", 24, 192_000, "FLAC"),
        ];
        sort_tracks(&mut tracks, TrackSortOption::HighestQuality);
        assert_eq!(tracks[0].title, "High");
    }

    #[test]
    fn filter_by_format_and_search() {
        let tracks = [track("Dawn", "Nova", "Horizon", 24, 96_000, "FLAC"), track("Dusk", "Echo", "Reverb", 24, 96_000, "ALAC")];
        assert_eq!(filter_tracks(&tracks, &QualityFilter::Format("FLAC".into()), "").len(), 1);
        assert_eq!(filter_tracks(&tracks, &QualityFilter::All, "echo").len(), 1);
    }
}
