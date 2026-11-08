//! Milestone 9 (polish/verification) of the egui-to-iced migration plan: a basic
//! performance check on large libraries, mirroring `library_scan_benchmark.rs`'s own
//! "doesn't regress to O(n^2)" scale (7,200 tracks) and generous, Debug-profile-aware
//! budget. Builds each list-heavy view's `Element` tree (no GPU/windowing — just the
//! widget-tree construction + this view's own filter/sort/group pass) over that many
//! tracks and asserts it finishes well within a human-perceptible frame budget, so a
//! future `O(n^2)` regression in a filter/sort/group function gets caught here rather
//! than only as "the app feels slow" once a library is actually that large.

use std::time::{Duration, Instant};

use chrono::Utc;
use walkman_domain::models::{LocalMusicFolder, Track};

use walkman_ui::iced_ui::views::{albums, artists, hi_res, library, local_library, recently_added};
use walkman_ui::views::albums::AlbumsViewState;
use walkman_ui::views::artists::ArtistsViewState;
use walkman_ui::views::hi_res::HiResViewState;
use walkman_ui::views::library::LibraryViewState;
use walkman_ui::views::local_library::LocalLibraryViewState;
use walkman_ui::views::recently_added::RecentlyAddedViewState;

/// Matches `library_scan_benchmark.rs::TRACK_COUNT` so both benchmarks describe the same
/// "large library" scale.
const TRACK_COUNT: usize = 7_200;
const TRACKS_PER_ALBUM: usize = 12;

/// Generous wall-clock budget per view, for the same reason
/// `library_scan_benchmark.rs::SCAN_BUDGET` is generous: this is a `Debug`-profile
/// `cargo test` run, not a release-optimized production frame — the point is "doesn't
/// hang/regress to O(n^2)", not a tight production SLA. A real 60fps frame budget is
/// ~16ms; 2 seconds is well over two orders of magnitude of headroom.
const VIEW_BUDGET: Duration = Duration::from_secs(2);

fn fixture_tracks() -> Vec<Track> {
    (0..TRACK_COUNT)
        .map(|i| {
            let album_index = i / TRACKS_PER_ALBUM;
            Track {
                filename: format!("track-{i}.flac"),
                relative_path: format!("Artist {}/Album {album_index}/track-{i}.flac", album_index % 400),
                full_path: format!("/music/Artist {}/Album {album_index}/track-{i}.flac", album_index % 400),
                format: "FLAC".into(),
                title: format!("Track {i}"),
                artist: format!("Artist {}", album_index % 400),
                album: format!("Album {album_index}"),
                track_num: (i % TRACKS_PER_ALBUM + 1).to_string(),
                year: "2024".into(),
                genre: "Rock".into(),
                duration_seconds: 180.0,
                bitrate_kbps: 900,
                sample_rate: if i % 5 == 0 { 96_000 } else { 44_100 },
                bits_per_sample: if i % 5 == 0 { 24 } else { 16 },
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
        })
        .collect()
}

/// Asserts `build` finishes within [`VIEW_BUDGET`], panicking with a clear message
/// (including the actual elapsed time) if it doesn't — same shape as
/// `library_scan_benchmark.rs`'s own budget assertions.
fn assert_within_budget(label: &str, build: impl FnOnce()) {
    let start = Instant::now();
    build();
    let elapsed = start.elapsed();
    assert!(elapsed <= VIEW_BUDGET, "{label} took {elapsed:?}, over the {VIEW_BUDGET:?} budget (possible O(n^2) regression)");
}

#[test]
fn local_library_songs_view_scales_to_a_large_library() {
    let tracks = fixture_tracks();
    let folders: Vec<LocalMusicFolder> = Vec::new();
    let state = LocalLibraryViewState::default();
    assert_within_budget("local_library::view (Songs)", || {
        let _ = local_library::view(&tracks, &folders, &state);
    });
}

#[test]
fn library_all_songs_view_scales_to_a_large_library() {
    let tracks = fixture_tracks();
    let state = LibraryViewState::default();
    assert_within_budget("library::view", || {
        let _ = library::view(&tracks, &state);
    });
}

#[test]
fn recently_added_view_scales_to_a_large_library() {
    let tracks = fixture_tracks();
    let state = RecentlyAddedViewState::default();
    assert_within_budget("recently_added::view", || {
        let _ = recently_added::view(&tracks, &state);
    });
}

#[test]
fn albums_grid_view_scales_to_a_large_library() {
    let tracks = fixture_tracks();
    let state = AlbumsViewState::default();
    assert_within_budget("albums::view", || {
        let _ = albums::view(&tracks, &state);
    });
}

#[test]
fn artists_grid_view_scales_to_a_large_library() {
    let tracks = fixture_tracks();
    let state = ArtistsViewState::default();
    assert_within_budget("artists::view", || {
        let _ = artists::view(&tracks, &state);
    });
}

#[test]
fn hi_res_view_scales_to_a_large_library() {
    let tracks = fixture_tracks();
    let state = HiResViewState::default();
    assert_within_budget("hi_res::view", || {
        let _ = hi_res::view(&tracks, &state);
    });
}
