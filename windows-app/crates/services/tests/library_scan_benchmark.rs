//! Benchmark/integration test for spec task 19.2 (R18.4/R19.5): scans a generated 7,000+
//! track fixture library and asserts (a) `LocalLibrary::rescan_all_async`'s scan+precompute
//! runs off the calling thread — the call itself returns near-instantly regardless of how
//! long the background work takes — and (b) the whole thing finishes within a generous
//! performance budget. Reuses the same hand-built, frameless, `STREAMINFO`-only minimal FLAC
//! fixture technique `library_scan_integration.rs` (task 7.4) already established, at a much
//! larger scale.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use walkman_services::library_scanner::LibraryScanner;
use walkman_services::local_library::LocalLibrary;

/// How many tracks the fixture library has (R19.5's "7,000+").
const TRACK_COUNT: usize = 7_200;
/// Tracks per album; every album's tracks share one artist 2-albums-at-a-time (see
/// `album_and_artist_for`).
const TRACKS_PER_ALBUM: usize = 12;

/// Generous wall-clock budget for the *whole* background scan+precompute to finish, polling
/// from the test's perspective. This is intentionally loose (a `Debug`-profile `cargo test`
/// run doing real `lofty` parsing + SQLite upserts for 7,200 files, not a release-optimized
/// production run) — the point of this assertion is "doesn't hang/regress to O(n^2)", not a
/// tight production SLA.
const SCAN_BUDGET: Duration = Duration::from_secs(90);
/// How long the *calling* thread may block inside `rescan_all_async` itself (R18.4: the UI
/// thread must never block on a large-library scan). The call should return after only a
/// cheap folder-list read, not after the scan completes.
const ASYNC_CALL_BUDGET: Duration = Duration::from_millis(200);

fn build_streaminfo(sample_rate: u32, bits_per_sample: u8) -> Vec<u8> {
    let mut bits = BitWriter::default();
    bits.push_bits(4096, 16);
    bits.push_bits(4096, 16);
    bits.push_bits(0, 24);
    bits.push_bits(0, 24);
    bits.push_bits(sample_rate as u64, 20);
    bits.push_bits(0, 3);
    bits.push_bits((bits_per_sample - 1) as u64, 5);
    bits.push_bits(0, 36);
    let mut out = bits.into_bytes();
    out.extend_from_slice(&[0u8; 16]);
    out
}

fn build_vorbis_comment(title: &str, artist: &str, album: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let vendor = b"walkman-studio-bench";
    out.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    out.extend_from_slice(vendor);

    let comments = [format!("TITLE={title}"), format!("ARTIST={artist}"), format!("ALBUM={album}")];
    out.extend_from_slice(&(comments.len() as u32).to_le_bytes());
    for comment in comments {
        out.extend_from_slice(&(comment.len() as u32).to_le_bytes());
        out.extend_from_slice(comment.as_bytes());
    }
    out
}

#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    bit_pos: u8,
}

impl BitWriter {
    fn push_bits(&mut self, value: u64, n_bits: u8) {
        for i in (0..n_bits).rev() {
            let bit = ((value >> i) & 1) as u8;
            if self.bit_pos == 0 {
                self.bytes.push(0);
            }
            let last = self.bytes.last_mut().unwrap();
            *last |= bit << (7 - self.bit_pos);
            self.bit_pos = (self.bit_pos + 1) % 8;
        }
    }

    fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

fn write_minimal_flac(path: &Path, sample_rate: u32, bits_per_sample: u8, title: &str, artist: &str, album: &str) {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"fLaC");

    let streaminfo = build_streaminfo(sample_rate, bits_per_sample);
    bytes.push(0x00);
    bytes.extend_from_slice(&(streaminfo.len() as u32).to_be_bytes()[1..]);
    bytes.extend_from_slice(&streaminfo);

    let vorbis_comment = build_vorbis_comment(title, artist, album);
    bytes.push(0x80 | 0x04);
    bytes.extend_from_slice(&(vorbis_comment.len() as u32).to_be_bytes()[1..]);
    bytes.extend_from_slice(&vorbis_comment);

    std::fs::write(path, bytes).unwrap();
}

/// Deterministically spreads `TRACK_COUNT` tracks across albums (`TRACKS_PER_ALBUM` each) and
/// artists (2 albums each), with every 5th track bumped to Hi-Res — enough structural variety
/// that `precompute`'s grouping isn't just iterating one giant bucket.
fn album_and_artist_for(track_index: usize) -> (String, String) {
    let album_index = track_index / TRACKS_PER_ALBUM;
    let artist_index = album_index / 2;
    (format!("Album {album_index:04}"), format!("Artist {artist_index:04}"))
}

fn write_fixture_library(root: &Path) {
    for i in 0..TRACK_COUNT {
        let (album, artist) = album_and_artist_for(i);
        let dir = root.join(&artist).join(&album);
        std::fs::create_dir_all(&dir).unwrap();
        let (sample_rate, bits) = if i % 5 == 0 { (96_000, 24) } else { (44_100, 16) };
        write_minimal_flac(&dir.join(format!("{:03} Track.flac", i % TRACKS_PER_ALBUM)), sample_rate, bits, &format!("Track {i:05}"), &artist, &album);
    }
}

#[test]
fn scans_a_7000_plus_track_fixture_library_off_thread_within_budget() {
    let root = tempfile::tempdir().unwrap();
    write_fixture_library(root.path());

    let scanner = Arc::new(LibraryScanner::open_in_memory().unwrap());
    let library = Arc::new(LocalLibrary::new(scanner));
    library.add_folder(root.path()).expect("initial add_folder should register the fixture tree");

    // R18.4: the async call itself must return almost immediately — the actual scan runs on
    // a background thread, not inline here — regardless of how long that background work
    // takes for 7,200 files.
    let call_started = Instant::now();
    let rx = library.rescan_all_async();
    let call_elapsed = call_started.elapsed();
    assert!(call_elapsed < ASYNC_CALL_BUDGET, "rescan_all_async blocked the caller for {call_elapsed:?} (budget {ASYNC_CALL_BUDGET:?}) — it should hand off to a worker thread, not scan inline");

    // Poll the channel (as the real UI's per-frame tick does, see `app/src/lifecycle.rs`'s
    // `poll_local_rescan`) until the background scan+precompute finishes or the budget runs
    // out.
    let scan_started = Instant::now();
    let caches = rx.recv_timeout(SCAN_BUDGET).expect("background scan+precompute did not finish within budget").expect("scan should succeed against a well-formed fixture tree");
    let scan_elapsed = scan_started.elapsed();
    assert!(scan_elapsed < SCAN_BUDGET, "background scan+precompute took {scan_elapsed:?}, exceeding the {SCAN_BUDGET:?} budget");

    assert_eq!(caches.stats.track_count, TRACK_COUNT);
    assert_eq!(caches.stats.hi_res_count, TRACK_COUNT.div_ceil(5), "every 5th track was written as Hi-Res");
    assert_eq!(caches.stats.album_count, TRACK_COUNT / TRACKS_PER_ALBUM);
    assert_eq!(caches.stats.artist_count, TRACK_COUNT / TRACKS_PER_ALBUM / 2);
}
