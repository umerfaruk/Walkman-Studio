//! SensMe™ channels carousel, ported from `Sources/WalkmanStudio/Views/SenseMeChannelsView.swift`.
//!
//! Spec task **15.3** (requirements 6.4, 6.5): a carousel over the 14 official SensMe
//! channels ([`SenseMeChannel::ALL`], already classified/ported in task 5.4), a per-channel
//! track count with Play/Shuffle actions, a global "Shuffle All Songs" action, and an
//! analysis-trigger command for any track that hasn't been 12-Tone analyzed yet.
//!
//! ## Honest simplifications
//! - **3D cover-flow**: egui's 2D immediate-mode painter has no perspective/3D transform, so
//!   the macOS "3D cover-flow carousel" is approximated as a flat horizontal strip of glass
//!   tiles centered on the selected channel, with the selected tile emphasized (larger, gold
//!   border) instead of a true depth/perspective illusion.
//! - **Atmospheric backgrounds**: no per-channel background art asset exists; each tile's
//!   backdrop is the existing glass surface rather than real channel artwork.
//! - **Live lyrics during SensMe playback**: reuses the fullscreen karaoke lyrics already
//!   built in task 15.2 ([`crate::now_playing::render_fullscreen_overlay`]) rather than a
//!   second lyrics renderer — "Play"/"Shuffle" set [`crate::shell::AppState::is_fullscreen_player`]
//!   so the existing (already player-bound, task 15.1) fullscreen view takes over; this view
//!   only resolves which tracks to queue.
//! - **Gapless highlight-seek playback**: this view resolves *which* track ids belong to a
//!   channel, in play order, and emits them as a single command; actually queuing them and
//!   seeking each to its `Track::senseme_highlight_sec` on advance is the player/task 18's job
//!   (the player already supports seeking and gapless auto-advance from task 13).

use eframe::egui;

use walkman_domain::models::Track;
use walkman_domain::senseme::SenseMeChannel;

use crate::theme::{button, color, font, glass};

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

/// Commands the carousel emits for task 18 to drain against the real player/SensMe analysis
/// service. Track ids are `Track::id()` (the full path).
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

/// How many tiles show on either side of the selected tile in the flat carousel strip.
const CAROUSEL_SIDE_TILES: usize = 2;
const TILE_SIZE: f32 = 120.0;
const SELECTED_TILE_SIZE: f32 = 150.0;

pub fn render(ui: &mut egui::Ui, tracks: &[Track], state: &mut SenseMeViewState) {
    render_toolbar(ui, tracks, state);
    ui.add_space(16.0);
    render_carousel(ui, tracks, state);
    ui.add_space(20.0);
    render_selected_channel_detail(ui, tracks, state);
}

fn render_toolbar(ui: &mut egui::Ui, tracks: &[Track], state: &mut SenseMeViewState) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("SensMe\u{2122} Channels")
                .font(font::title2())
                .color(color::SILVER_LIGHT),
        );
        ui.add_space(16.0);

        let shuffle_all_ids: Vec<String> = tracks_for_channel(tracks, SenseMeChannel::ShuffleAllSongs)
            .iter()
            .map(|t| t.id().to_owned())
            .collect();
        if ui
            .add(button::Button::new("Shuffle All Songs", button::Variant::Gold).enabled(!shuffle_all_ids.is_empty()))
            .clicked()
        {
            state.shuffle_seed = state.shuffle_seed.wrapping_add(1);
            state
                .commands
                .push(SenseMeCommand::ShuffleChannel(shuffled_ids(&shuffle_all_ids, state.shuffle_seed)));
        }

        ui.add_space(8.0);
        let unanalyzed = unanalyzed_track_ids(tracks);
        if ui
            .add(button::Button::new(format!("Analyze Library ({})", unanalyzed.len()), button::Variant::Glass).enabled(!unanalyzed.is_empty()))
            .clicked()
        {
            state.commands.push(SenseMeCommand::AnalyzeUnanalyzed(unanalyzed));
        }
    });
}

fn render_carousel(ui: &mut egui::Ui, tracks: &[Track], state: &mut SenseMeViewState) {
    let counts = channel_counts(tracks);
    let len = SenseMeChannel::ALL.len();

    ui.horizontal_centered(|ui| {
        if ui.add(button::Button::new("\u{2039}", button::Variant::Glass)).clicked() {
            state.selected = select_previous(state.selected);
        }
        ui.add_space(12.0);

        ui.horizontal(|ui| {
            for offset in -(CAROUSEL_SIDE_TILES as isize)..=(CAROUSEL_SIDE_TILES as isize) {
                let index = ((state.selected as isize + offset).rem_euclid(len as isize)) as usize;
                render_tile(ui, index, counts[index], state);
                ui.add_space(10.0);
            }
        });

        ui.add_space(12.0);
        if ui.add(button::Button::new("\u{203A}", button::Variant::Glass)).clicked() {
            state.selected = select_next(state.selected);
        }
    });
}

fn render_tile(ui: &mut egui::Ui, index: usize, count: usize, state: &mut SenseMeViewState) {
    let channel = SenseMeChannel::ALL[index];
    let is_selected = index == state.selected;
    let size = if is_selected { SELECTED_TILE_SIZE } else { TILE_SIZE };
    let glass_state = if is_selected { glass::State::Selected } else { glass::State::Normal };

    let response = glass::liquid_glass(glass_state).show(ui, |ui| {
        ui.set_width(size);
        ui.set_height(size);
        ui.vertical_centered(|ui| {
            ui.add_space(size * 0.3);
            ui.label(
                egui::RichText::new(channel.display_name())
                    .font(if is_selected { font::callout_bold() } else { font::callout() })
                    .color(if is_selected { color::HIRES_GOLD } else { color::SILVER_LIGHT }),
            );
            ui.label(
                egui::RichText::new(format!("{count} tracks"))
                    .font(font::caption())
                    .color(color::SILVER_MUTED),
            );
        });
    });

    if response.response.interact(egui::Sense::click()).clicked() {
        state.selected = index;
    }
}

fn render_selected_channel_detail(ui: &mut egui::Ui, tracks: &[Track], state: &mut SenseMeViewState) {
    let channel = SenseMeChannel::ALL[state.selected];
    let channel_ids: Vec<String> = tracks_for_channel(tracks, channel).iter().map(|t| t.id().to_owned()).collect();
    let has_tracks = !channel_ids.is_empty();

    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new(channel.display_name())
                .font(font::title1())
                .color(color::SILVER_LIGHT),
        );
        ui.label(
            egui::RichText::new(format!("{} track{}", channel_ids.len(), if channel_ids.len() == 1 { "" } else { "s" }))
                .font(font::callout())
                .color(color::SILVER_MUTED),
        );
        ui.add_space(12.0);

        ui.horizontal(|ui| {
            if ui.add(button::Button::new("Play", button::Variant::Prominent).enabled(has_tracks)).clicked() {
                state.commands.push(SenseMeCommand::PlayChannel(channel_ids.clone()));
            }
            ui.add_space(8.0);
            if ui.add(button::Button::new("Shuffle", button::Variant::Glass).enabled(has_tracks)).clicked() {
                state.shuffle_seed = state.shuffle_seed.wrapping_add(1);
                state
                    .commands
                    .push(SenseMeCommand::ShuffleChannel(shuffled_ids(&channel_ids, state.shuffle_seed)));
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

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

    // ---- Channel filtering + counts ----

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

    // ---- Carousel selection wrap-around ----

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

    // ---- Shuffle ----

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

    // ---- Headless render smoke test ----

    /// Drives the carousel through a headless egui frame with no tracks, with tracks spread
    /// across several channels, and at a boundary selection index — egui lays out and paints
    /// during `Context::run`, so a bad rect/font/widget would panic here.
    #[test]
    fn senseme_view_renders_in_headless_frame() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);

        let tracks = vec![
            sample_track("a", &["Energetic", "Daytime", "Shuffle All Songs"], true),
            sample_track("b", &["Relax", "Night", "Shuffle All Songs"], true),
            sample_track("c", &[], false),
        ];

        for (tracks, selected) in [(Vec::new(), 0usize), (tracks.clone(), 0), (tracks, 13)] {
            let mut state = SenseMeViewState { selected, ..SenseMeViewState::default() };
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render(ui, &tracks, &mut state);
                });
            });
        }
    }
}
