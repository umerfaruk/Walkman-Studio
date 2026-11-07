//! Transfer & Convert queue (spec task 10.2, R9.1/9.2/9.4/9.6/9.7).
//!
//! `TransferQueue` owns the queued [`TransferItem`]s; preparation (artwork optimization) runs
//! concurrently across the whole queue via `rayon` (R9.7's "single-pass concurrent
//! metadata-preparation stage" — lyrics fetching is task 12.2's job and not wired in here yet,
//! so `prepare_all` only optimizes artwork today). The actual per-item transfer
//! (copy/convert/sanitize/tag) and the dual-target chapter-playlist generation are plain
//! functions rather than queue methods, so they're usable for both the drag-and-drop queue and
//! the Local Library's 1-click direct-transfer flow (R9.7) without going through a queue at all.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::sync::Mutex;

use rayon::prelude::*;
use walkman_domain::models::{Track, TransferItem, TransferStatus, TransferTargetMode};
use walkman_domain::sanitize::sanitize_walkman_text;

use crate::audio_tagger::{self, TagFields};
use crate::duplicate_guard::{resolve_keep_both_path, DuplicateResolution};
use crate::error::{AppError, Result};
use crate::transcode::{self, Preset};
use crate::walkman_engine::target_track_fs_path;

/// File extensions a classic Sony-OS DAP accepts (R9.6).
const SONY_OS_FORMATS: &[&str] = &["MP3", "FLAC", "WAV", "AAC", "M4A", "ALAC"];
/// File extensions an Android Walkman accepts — the broader Android media framework codec set.
const ANDROID_FORMATS: &[&str] = &["MP3", "FLAC", "WAV", "AAC", "M4A", "ALAC", "OGG"];

/// R9.6: format-support rules differ for Sony-OS vs. Android Walkman targets.
pub fn is_format_supported(format: &str, is_android: bool) -> bool {
    let upper = format.to_uppercase();
    let table = if is_android { ANDROID_FORMATS } else { SONY_OS_FORMATS };
    table.contains(&upper.as_str())
}

/// Builds a pending [`TransferItem`] from an already-indexed [`Track`] (R9.7: 1-click direct
/// transfer from the Local Library).
pub fn item_from_track(track: &Track) -> TransferItem {
    TransferItem {
        id: uuid::Uuid::new_v4().to_string(),
        source_path: track.full_path.clone(),
        status: TransferStatus::Pending,
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: track.album.clone(),
        track_number: track.track_num.clone(),
        year: track.year.clone(),
        genre: track.genre.clone(),
        duration_seconds: track.duration_seconds,
        bitrate_kbps: track.bitrate_kbps,
        sample_rate: track.sample_rate,
        bits_per_sample: track.bits_per_sample,
        format: track.format.clone(),
        size_bytes: track.size_bytes,
        artwork_data: None,
        is_artwork_optimized: false,
        lyrics: None,
        is_lyrics_synced: false,
        is_fetching_online: false,
        is_optimizing: false,
        is_expanded: false,
        is_duplicate_on_walkman: false,
        duplicate_match_info: None,
        duplicate_matched_track: None,
    }
}

/// Builds a pending [`TransferItem`] by reading `path`'s tags directly (R9.1: drag-and-drop a
/// file that isn't in the Local Library's index at all).
pub fn item_from_path(path: &Path) -> Result<TransferItem> {
    let meta = audio_tagger::read_metadata(path)?;
    let size_bytes = std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);
    let format = path.extension().and_then(|e| e.to_str()).map(|e| e.to_uppercase()).unwrap_or_default();
    let filename_stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();

    Ok(TransferItem {
        id: uuid::Uuid::new_v4().to_string(),
        source_path: path.to_string_lossy().into_owned(),
        status: TransferStatus::Pending,
        title: if meta.title.is_empty() { filename_stem } else { meta.title },
        artist: if meta.artist.is_empty() { "Unknown Artist".to_string() } else { meta.artist },
        album: if meta.album.is_empty() { "Unknown Album".to_string() } else { meta.album },
        track_number: meta.track_number,
        year: meta.year,
        genre: meta.genre,
        duration_seconds: meta.duration_seconds,
        bitrate_kbps: meta.bitrate_kbps,
        sample_rate: meta.sample_rate,
        bits_per_sample: meta.bits_per_sample,
        format,
        size_bytes,
        artwork_data: None,
        is_artwork_optimized: false,
        lyrics: meta.lyrics,
        is_lyrics_synced: false,
        is_fetching_online: false,
        is_optimizing: false,
        is_expanded: false,
        is_duplicate_on_walkman: false,
        duplicate_match_info: None,
        duplicate_matched_track: None,
    })
}

/// A drag-and-drop batch queue of [`TransferItem`]s (R9.1).
#[derive(Default)]
pub struct TransferQueue {
    items: Mutex<Vec<TransferItem>>,
}

impl TransferQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueue(&self, item: TransferItem) {
        self.items.lock().unwrap().push(item);
    }

    pub fn enqueue_tracks(&self, tracks: &[Track]) {
        let mut items = self.items.lock().unwrap();
        items.extend(tracks.iter().map(item_from_track));
    }

    /// Snapshot of every queued item, in queue order.
    pub fn items(&self) -> Vec<TransferItem> {
        self.items.lock().unwrap().clone()
    }

    pub fn len(&self) -> usize {
        self.items.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn remove(&self, id: &str) {
        self.items.lock().unwrap().retain(|i| i.id != id);
    }

    pub fn clear(&self) {
        self.items.lock().unwrap().clear();
    }

    /// Mutates the item with `id` in place, if it's still queued.
    pub fn update<F: FnOnce(&mut TransferItem)>(&self, id: &str, f: F) {
        if let Some(item) = self.items.lock().unwrap().iter_mut().find(|i| i.id == id) {
            f(item);
        }
    }

    /// R9.7's single-pass concurrent preparation stage: reads each pending item's artwork from
    /// its source file (if not already loaded) and optimizes it to a Baseline JPEG, in
    /// parallel across the whole queue. Lyrics fetching (task 12.2) is not wired in yet.
    pub fn prepare_all(&self) {
        let snapshot_ids: Vec<String> = {
            let items = self.items.lock().unwrap();
            items.iter().filter(|i| !i.is_artwork_optimized).map(|i| i.id.clone()).collect()
        };

        let prepared: Vec<(String, Option<Vec<u8>>)> = snapshot_ids
            .into_par_iter()
            .map(|id| {
                let source_path = {
                    let items = self.items.lock().unwrap();
                    items.iter().find(|i| i.id == id).map(|i| i.source_path.clone())
                };
                let artwork = source_path.and_then(|p| extract_and_optimize_artwork(Path::new(&p)));
                (id, artwork)
            })
            .collect();

        let mut items = self.items.lock().unwrap();
        for (id, artwork) in prepared {
            if let Some(item) = items.iter_mut().find(|i| i.id == id) {
                item.artwork_data = artwork;
                item.is_artwork_optimized = true;
            }
        }
    }
}

fn extract_and_optimize_artwork(path: &Path) -> Option<Vec<u8>> {
    use lofty::file::TaggedFileExt;
    use lofty::probe::Probe;
    let tagged_file = Probe::open(path).ok()?.read().ok()?;
    let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag())?;
    let picture = tag.pictures().first()?;
    crate::artwork::encode_baseline_jpeg(picture.data()).ok()
}

/// Sanitized, Walkman-safe copies of a transfer item's text fields, used both for the on-disk
/// path (via `target_track_fs_path`) and the tags written into the transferred file.
struct SanitizedFields {
    title: String,
    artist: String,
    album: String,
    genre: String,
}

fn sanitize_fields(item: &TransferItem) -> SanitizedFields {
    SanitizedFields {
        title: sanitize_walkman_text(&item.title, true),
        artist: sanitize_walkman_text(&item.artist, true),
        album: sanitize_walkman_text(&item.album, true),
        genre: sanitize_walkman_text(&item.genre, false),
    }
}

/// What happened to one item in [`transfer_item`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferOutcome {
    Written(PathBuf),
    Skipped,
}

/// Transfers one item: resolves its target path under `target_root` (R9.4's
/// `/MUSIC/Artist/Album/NN - Title.ext` layout, built from sanitized fields), copies or
/// transcodes the audio per `preset`, writes sanitized tags (+ optimized artwork, if present)
/// onto the result, and applies `duplicate_resolution` if the destination already exists
/// (R9.5). `duplicate_resolution` is only consulted when the destination collides; pass `None`
/// to default to `KeepBoth` (never silently overwrite).
pub fn transfer_item(
    item: &TransferItem,
    target_root: &Path,
    preset: Preset,
    loudnorm: bool,
    duplicate_resolution: Option<DuplicateResolution>,
) -> Result<TransferOutcome> {
    let fields = sanitize_fields(item);
    let source_ext = Path::new(&item.source_path).extension().and_then(|e| e.to_str()).unwrap_or("");
    let ext = {
        let preset_ext = preset.output_extension();
        if preset_ext.is_empty() { source_ext } else { preset_ext }
    };

    let mut dest = target_track_fs_path(target_root, &fields.artist, &fields.album, &item.track_number, &fields.title, ext);

    if dest.exists() {
        match duplicate_resolution.unwrap_or(DuplicateResolution::KeepBoth) {
            DuplicateResolution::SkipExisting => return Ok(TransferOutcome::Skipped),
            DuplicateResolution::Overwrite => {}
            DuplicateResolution::KeepBoth => dest = resolve_keep_both_path(&dest),
        }
    }

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if preset.requires_ffmpeg() {
        transcode::transcode(preset, Path::new(&item.source_path), &dest, loudnorm)?;
    } else {
        std::fs::copy(&item.source_path, &dest)?;
    }

    audio_tagger::write_tags(
        &dest,
        &TagFields {
            title: &fields.title,
            artist: &fields.artist,
            album: &fields.album,
            track_number: &item.track_number,
            disc_number: "",
            year: &item.year,
            genre: &fields.genre,
            lyrics: item.lyrics.as_deref(),
            artwork_jpeg: item.artwork_data.as_deref(),
        },
    )
    .map_err(|e| AppError::Tag(format!("transferred {} but failed to write tags: {e}", dest.display())))?;

    Ok(TransferOutcome::Written(dest))
}

/// Progress emitted by [`transfer_batch_async`] (R9.6's `TransferSyncSheet`).
#[derive(Debug, Clone)]
pub struct TransferProgress {
    pub completed: usize,
    pub total: usize,
    pub current_title: String,
    pub last_result: Option<(String, std::result::Result<TransferOutcome, String>)>,
}

/// Transfers every item in `items` to `target_root` sequentially on a background thread,
/// publishing a [`TransferProgress`] after each one so the UI thread never blocks (same
/// off-thread + channel pattern as `local_library::rescan_all_async`, task 7.3).
pub fn transfer_batch_async(
    items: Vec<TransferItem>,
    target_root: PathBuf,
    preset: Preset,
    loudnorm: bool,
    duplicate_resolution: Option<DuplicateResolution>,
) -> Receiver<TransferProgress> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let total = items.len();
        for (i, item) in items.into_iter().enumerate() {
            let result = transfer_item(&item, &target_root, preset, loudnorm, duplicate_resolution)
                .map_err(|e| e.to_string());
            let progress = TransferProgress {
                completed: i + 1,
                total,
                current_title: item.title.clone(),
                last_result: Some((item.id.clone(), result)),
            };
            if tx.send(progress).is_err() {
                return; // receiver dropped — caller stopped listening.
            }
        }
    });
    rx
}

/// Writes a chapter playlist for the `LEARNING` target (R9.2's "automatic chapter playlist
/// generation for the learning target"), listing `chapters` (relative path from `learning_dir`,
/// display text, duration seconds) in order.
pub fn write_chapter_playlist(learning_dir: &Path, course_name: &str, chapters: &[(String, String, f64)]) -> Result<PathBuf> {
    use walkman_domain::sanitize::{sanitize_filename, DEFAULT_MAX_FILENAME_LEN};
    let playlist_path = learning_dir.join(format!("{}.m3u8", sanitize_filename(course_name, DEFAULT_MAX_FILENAME_LEN)));
    crate::walkman_engine::write_playlist(&playlist_path, chapters)?;
    Ok(playlist_path)
}

/// Resolves the target root directory for `mode` under a drive's music directory's parent (the
/// drive root), e.g. `E:\MUSIC` or `E:\LEARNING`.
pub fn target_root_for(drive_music_dir: &Path, mode: TransferTargetMode) -> PathBuf {
    let drive_root = drive_music_dir.parent().unwrap_or(drive_music_dir);
    drive_root.join(mode.root_folder_name())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_track(title: &str, artist: &str) -> Track {
        Track {
            filename: format!("{title}.flac"),
            relative_path: format!("{artist}/{title}.flac"),
            full_path: format!("/music/{artist}/{title}.flac"),
            format: "FLAC".into(),
            title: title.into(),
            artist: artist.into(),
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
    fn format_support_differs_for_sony_os_vs_android() {
        assert!(is_format_supported("mp3", false));
        assert!(is_format_supported("mp3", true));
        assert!(!is_format_supported("ogg", false)); // Sony-OS DAPs don't support OGG
        assert!(is_format_supported("ogg", true));
    }

    #[test]
    fn queue_enqueue_remove_and_clear() {
        let queue = TransferQueue::new();
        queue.enqueue_tracks(&[sample_track("Dawn", "Nova"), sample_track("Dusk", "Nova")]);
        assert_eq!(queue.len(), 2);

        let id = queue.items()[0].id.clone();
        queue.update(&id, |item| item.is_expanded = true);
        assert!(queue.items()[0].is_expanded);

        queue.remove(&id);
        assert_eq!(queue.len(), 1);

        queue.clear();
        assert!(queue.is_empty());
    }

    #[test]
    fn transfer_item_lays_out_the_sony_path_and_writes_sanitized_tags() {
        let source_dir = tempfile::tempdir().unwrap();
        let target_dir = tempfile::tempdir().unwrap();
        let source_path = source_dir.path().join("input.flac");

        // Minimal valid FLAC (same fixture shape used elsewhere in this crate's tests).
        let mut flac = Vec::new();
        flac.extend_from_slice(b"fLaC");
        flac.push(0x80);
        flac.extend_from_slice(&34u32.to_be_bytes()[1..]);
        flac.extend_from_slice(&[0u8; 34]);
        std::fs::write(&source_path, &flac).unwrap();

        let mut item = item_from_track(&sample_track("My Song", "AC/DC"));
        item.source_path = source_path.to_string_lossy().into_owned();
        item.album = "Back in Black".to_string();
        item.track_number = "3".to_string();

        let outcome = transfer_item(&item, target_dir.path(), Preset::CopyRaw, false, None).unwrap();
        let TransferOutcome::Written(path) = outcome else { panic!("expected Written") };
        assert!(path.starts_with(target_dir.path()));
        assert!(path.to_string_lossy().contains("ACDC")); // '/' sanitized out of the artist name
        assert!(path.file_name().unwrap().to_string_lossy().starts_with("03 - My Song"));

        let bytes = std::fs::read(&path).unwrap();
        let tags = crate::flac_tags::read_tags(&bytes).unwrap();
        assert_eq!(tags.title, "My Song");
        // The written *tag* keeps the real artist name — only the on-disk *path* needs
        // filesystem-safe sanitizing (`sanitize_walkman_text` doesn't strip `/`, unlike
        // `sanitize_filename`, which `target_track_fs_path` uses for the path above).
        assert_eq!(tags.artist, "AC/DC");
    }

    #[test]
    fn transfer_item_respects_skip_existing_on_collision() {
        let source_dir = tempfile::tempdir().unwrap();
        let target_dir = tempfile::tempdir().unwrap();
        let source_path = source_dir.path().join("input.flac");
        let mut flac = Vec::new();
        flac.extend_from_slice(b"fLaC");
        flac.push(0x80);
        flac.extend_from_slice(&34u32.to_be_bytes()[1..]);
        flac.extend_from_slice(&[0u8; 34]);
        std::fs::write(&source_path, &flac).unwrap();

        let mut item = item_from_track(&sample_track("Song", "Artist"));
        item.source_path = source_path.to_string_lossy().into_owned();

        let first = transfer_item(&item, target_dir.path(), Preset::CopyRaw, false, None).unwrap();
        let TransferOutcome::Written(path) = &first else { panic!() };
        let before = std::fs::read(path).unwrap();

        let second = transfer_item(&item, target_dir.path(), Preset::CopyRaw, false, Some(DuplicateResolution::SkipExisting)).unwrap();
        assert_eq!(second, TransferOutcome::Skipped);
        assert_eq!(std::fs::read(path).unwrap(), before);
    }

    #[test]
    fn write_chapter_playlist_sanitizes_the_course_name_and_lists_chapters_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_chapter_playlist(
            dir.path(),
            "Lesson: 1?",
            &[
                ("Lesson 1/01.mp3".to_string(), "Chapter 1".to_string(), 300.0),
                ("Lesson 1/02.mp3".to_string(), "Chapter 2".to_string(), 280.0),
            ],
        )
        .unwrap();
        assert!(!path.file_name().unwrap().to_string_lossy().contains(':'));

        let playlist = crate::walkman_engine::read_playlist(&path).unwrap();
        assert_eq!(playlist.tracks, vec!["Lesson 1/01.mp3".to_string(), "Lesson 1/02.mp3".to_string()]);
    }

    #[test]
    fn target_root_for_resolves_music_and_learning_roots() {
        let music_dir = Path::new(r"E:\MUSIC");
        assert_eq!(target_root_for(music_dir, TransferTargetMode::Music), PathBuf::from(r"E:\MUSIC"));
        assert_eq!(target_root_for(music_dir, TransferTargetMode::Learning), PathBuf::from(r"E:\LEARNING"));
    }

    #[test]
    fn transfer_batch_async_reports_progress_for_every_item_without_blocking() {
        let source_dir = tempfile::tempdir().unwrap();
        let target_dir = tempfile::tempdir().unwrap();
        let mut flac = Vec::new();
        flac.extend_from_slice(b"fLaC");
        flac.push(0x80);
        flac.extend_from_slice(&34u32.to_be_bytes()[1..]);
        flac.extend_from_slice(&[0u8; 34]);

        let mut items = Vec::new();
        for i in 0..3 {
            let path = source_dir.path().join(format!("song{i}.flac"));
            std::fs::write(&path, &flac).unwrap();
            let mut item = item_from_track(&sample_track(&format!("Song {i}"), "Artist"));
            item.source_path = path.to_string_lossy().into_owned();
            items.push(item);
        }

        let rx = transfer_batch_async(items, target_dir.path().to_path_buf(), Preset::CopyRaw, false, None);
        let mut completed = Vec::new();
        for progress in rx.iter() {
            completed.push(progress.completed);
        }
        assert_eq!(completed, vec![1, 2, 3]);
    }
}
