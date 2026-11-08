//! Pure types/logic behind the SensMe\u{2122} Channels carousel, ported from
//! `Sources/WalkmanStudio/Views/SenseMeChannelsView.swift`. The rendering itself lives
//! in `iced_ui::views::senseme` (the egui `render` this module used to carry was removed
//! once the iced migration reached parity — see Milestone 8 of the egui-to-iced
//! migration plan).

use walkman_domain::models::Track;
use walkman_domain::senseme::SenseMeChannel;

/// Tracks whose `senseme_channels` includes `channel`'s SMFM token
/// ([`SenseMeChannel::channel_token`]), preserving `tracks`' original order. Every analyzed
/// track carries the `ShuffleAllSongs` token ([`walkman_domain::senseme::classify`] always
/// appends it), so this same filter covers the "Shuffle All Songs" channel with no special
/// case.
pub fn tracks_for_channel(tracks: &[Track], channel: SenseMeChannel) -> Vec<&Track> {
    let token = channel.channel_token();
    tracks
        .iter()
        .filter(|t| t.senseme_channels.iter().any(|c| c == token))
        .collect()
}

/// Track count for every channel in [`SenseMeChannel::ALL`] order, for the carousel tiles'
/// count badges.
pub fn channel_counts(tracks: &[Track]) -> [usize; 14] {
    let mut counts = [0usize; 14];
    for (i, channel) in SenseMeChannel::ALL.iter().enumerate() {
        counts[i] = tracks_for_channel(tracks, *channel).len();
    }
    counts
}

/// Ids (`Track::id()`) of every track that has not yet been 12-Tone analyzed
/// (`!is_senseme_analyzed`), for the analysis-trigger command.
pub fn unanalyzed_track_ids(tracks: &[Track]) -> Vec<String> {
    tracks
        .iter()
        .filter(|t| !t.is_senseme_analyzed)
        .map(|t| t.id().to_owned())
        .collect()
}

/// Selects the next channel, wrapping from the last tile back to the first.
pub fn select_next(selected: usize) -> usize {
    (selected + 1) % SenseMeChannel::ALL.len()
}

/// Selects the previous channel, wrapping from the first tile to the last.
pub fn select_previous(selected: usize) -> usize {
    if selected == 0 {
        SenseMeChannel::ALL.len() - 1
    } else {
        selected - 1
    }
}

/// A small deterministic splitmix64-based permutation of `ids`, used for "Shuffle
/// Channel"/"Shuffle All Songs" rather than pulling in a `rand` dependency for one call site.
/// Deterministic for a given `seed` (useful for tests); callers pick a fresh `seed` (e.g. the
/// current time) per shuffle so repeated clicks reorder.
pub fn shuffled_ids(ids: &[String], seed: u64) -> Vec<String> {
    let mut state = seed.wrapping_add(0x9E3779B97F4A7C15);
    let mut next_u64 = move || {
        state = state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    };

    let mut shuffled = ids.to_vec();
    // Fisher-Yates, back to front.
    for i in (1..shuffled.len()).rev() {
        let j = (next_u64() % (i as u64 + 1)) as usize;
        shuffled.swap(i, j);
    }
    shuffled
}

/// Commands the carousel emits for the real player/SensMe analysis service to drain.
/// Track ids are `Track::id()` (the full path).
#[derive(Debug, Clone, PartialEq)]
pub enum SenseMeCommand {
    /// Queue and play this channel's tracks, in order (ids).
    PlayChannel(Vec<String>),
    /// Queue and play this channel's tracks in a freshly shuffled order (ids, already
    /// shuffled).
    ShuffleChannel(Vec<String>),
    /// Trigger on-board 12-Tone analysis for every track that isn't analyzed yet (ids).
    AnalyzeUnanalyzed(Vec<String>),
}

#[derive(Debug, Clone, Default)]
pub struct SenseMeViewState {
    /// Index into [`SenseMeChannel::ALL`] of the carousel's centered/selected tile.
    pub selected: usize,
    /// Seed consumed (and advanced) by the next shuffle action, so repeated clicks reorder.
    pub shuffle_seed: u64,
    pub commands: Vec<SenseMeCommand>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use walkman_domain::models::Track;

    fn sample_track(id: &str, channels: &[&str], analyzed: bool) -> Track {
        Track {
            filename: id.into(),
            relative_path: id.into(),
            full_path: id.into(),
            format: "FLAC".into(),
            title: id.into(),
            artist: "Artist".into(),
            album: "Album".into(),
            track_num: "1".into(),
            year: "2024".into(),
            genre: String::new(),
            duration_seconds: 200.0,
            bitrate_kbps: 900,
            sample_rate: 44_100,
            bits_per_sample: 16,
            has_artwork: false,
            size_bytes: 1,
            date_added: Utc::now(),
            is_senseme_analyzed: analyzed,
            senseme_bpm: None,
            senseme_valence: None,
            senseme_arousal: None,
            senseme_channels: channels.iter().map(|s| s.to_string()).collect(),
            senseme_highlight_sec: Some(42.0),
        }
    }

    #[test]
    fn tracks_for_channel_filters_by_token() {
        let tracks = vec![
            sample_track("a", &["Energetic", "Daytime", "Shuffle All Songs"], true),
            sample_track("b", &["Relax", "Night", "Shuffle All Songs"], true),
        ];
        let energetic = tracks_for_channel(&tracks, SenseMeChannel::Energetic);
        assert_eq!(energetic.len(), 1);
        assert_eq!(energetic[0].id(), "a");

        // Every analyzed track carries "Shuffle All Songs" — no special case needed.
        let shuffle_all = tracks_for_channel(&tracks, SenseMeChannel::ShuffleAllSongs);
        assert_eq!(shuffle_all.len(), 2);
    }

    #[test]
    fn channel_counts_matches_all_channels_in_order() {
        let tracks = vec![sample_track("a", &["Morning"], true)];
        let counts = channel_counts(&tracks);
        assert_eq!(counts.len(), 14);
        let morning_index = SenseMeChannel::ALL.iter().position(|c| *c == SenseMeChannel::Morning).unwrap();
        assert_eq!(counts[morning_index], 1);
        // Every other channel is empty for this single-channel track.
        for (i, &c) in counts.iter().enumerate() {
            if i != morning_index {
                assert_eq!(c, 0);
            }
        }
    }

    #[test]
    fn unanalyzed_track_ids_finds_only_unanalyzed() {
        let tracks = vec![
            sample_track("analyzed", &["Energetic"], true),
            sample_track("pending", &[], false),
        ];
        assert_eq!(unanalyzed_track_ids(&tracks), vec!["pending".to_string()]);
    }

    #[test]
    fn select_next_wraps_past_the_last_channel() {
        assert_eq!(select_next(13), 0);
        assert_eq!(select_next(0), 1);
    }

    #[test]
    fn select_previous_wraps_before_the_first_channel() {
        assert_eq!(select_previous(0), 13);
        assert_eq!(select_previous(5), 4);
    }

    #[test]
    fn shuffled_ids_is_a_permutation_of_the_input() {
        let ids: Vec<String> = (0..20).map(|i| i.to_string()).collect();
        let shuffled = shuffled_ids(&ids, 7);
        let mut sorted_shuffled = shuffled.clone();
        sorted_shuffled.sort();
        let mut sorted_input = ids.clone();
        sorted_input.sort();
        assert_eq!(sorted_shuffled, sorted_input);
    }

    #[test]
    fn shuffled_ids_is_deterministic_for_the_same_seed() {
        let ids: Vec<String> = (0..10).map(|i| i.to_string()).collect();
        assert_eq!(shuffled_ids(&ids, 42), shuffled_ids(&ids, 42));
    }

    #[test]
    fn shuffled_ids_differs_across_seeds() {
        let ids: Vec<String> = (0..20).map(|i| i.to_string()).collect();
        assert_ne!(shuffled_ids(&ids, 1), shuffled_ids(&ids, 2));
    }

    #[test]
    fn shuffled_ids_handles_empty_and_singleton_input() {
        let empty: Vec<String> = Vec::new();
        assert_eq!(shuffled_ids(&empty, 1), empty);
        let one = vec!["only".to_string()];
        assert_eq!(shuffled_ids(&one, 99), one);
    }
}
