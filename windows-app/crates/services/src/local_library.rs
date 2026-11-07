//! Local music library service (spec task 7.2/7.3).
//!
//! Ported concept from `local_library.rs` in the macOS app: a set of user-registered local
//! folders (persisted via [`LibraryScanner`]'s `local_folders` table), each independently
//! rescanned into the `tracks_cache`, with the combined track list precomputed off the calling
//! thread into albums/artists/recent/language/stats (R18.4) and published back over a channel.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::sync::Arc;

use chrono::Utc;

use walkman_domain::models::{AlbumItem, ArtistItem, LocalMusicFolder, Track};

use crate::error::{AppError, Result};
use crate::library_scanner::LibraryScanner;

/// Precomputed groupings over the local library's tracks (R18.4), built off the UI thread.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PrecomputedCaches {
    pub albums: Vec<AlbumItem>,
    pub artists: Vec<ArtistItem>,
    /// All tracks, newest `date_added` first.
    pub recent: Vec<Track>,
    /// Tracks under `/LEARNING/` or a language/audiobook/podcast genre.
    pub language_study: Vec<Track>,
    pub stats: LibraryStats,
}

/// Summary counters shown in Device Overview / Local Library headers.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LibraryStats {
    pub track_count: usize,
    pub album_count: usize,
    pub artist_count: usize,
    pub hi_res_count: usize,
    pub language_study_count: usize,
    pub total_bytes: i64,
}

/// Groups `tracks` into albums/artists, sorts the recent list, and computes stats. Pure
/// computation (no I/O), so it is cheap to call from a worker thread (see
/// [`LocalLibrary::rescan_all_async`]) or inline in tests.
pub fn precompute(tracks: Vec<Track>) -> PrecomputedCaches {
    use std::collections::BTreeMap;

    // Group into albums keyed by (lowercased name, lowercased artist), preserving first-seen
    // display casing, matching `AlbumItem::id()`'s own key shape.
    let mut album_buckets: BTreeMap<(String, String), (String, String, Vec<Track>)> = BTreeMap::new();
    for track in &tracks {
        let key = (track.album.to_lowercase(), track.artist.to_lowercase());
        album_buckets
            .entry(key)
            .or_insert_with(|| (track.album.clone(), track.artist.clone(), Vec::new()))
            .2
            .push(track.clone());
    }
    let mut albums: Vec<AlbumItem> = album_buckets
        .into_values()
        .map(|(name, artist, album_tracks)| {
            let year = album_tracks.first().map(|t| t.year.clone()).unwrap_or_default();
            let count = album_tracks.len();
            AlbumItem::new(name, artist, year, count, album_tracks)
        })
        .collect();
    albums.sort_by_key(|a| a.name.to_lowercase());

    // Group into artists keyed by lowercased primary artist name.
    let mut artist_buckets: BTreeMap<String, (String, Vec<Track>)> = BTreeMap::new();
    for track in &tracks {
        let key = track.artist.to_lowercase();
        artist_buckets
            .entry(key)
            .or_insert_with(|| (track.artist.clone(), Vec::new()))
            .1
            .push(track.clone());
    }
    let mut artists: Vec<ArtistItem> = artist_buckets
        .into_values()
        .map(|(name, artist_tracks)| {
            let artist_albums: Vec<AlbumItem> = albums
                .iter()
                .filter(|a| a.artist.to_lowercase() == name.to_lowercase())
                .cloned()
                .collect();
            let album_count = artist_albums.len();
            let track_count = artist_tracks.len();
            ArtistItem::new(name, album_count, track_count, artist_albums, artist_tracks)
        })
        .collect();
    artists.sort_by_key(|a| a.name.to_lowercase());

    let mut recent = tracks.clone();
    recent.sort_by_key(|t| std::cmp::Reverse(t.date_added));

    let language_study: Vec<Track> = tracks.iter().filter(|t| t.is_language_study()).cloned().collect();

    let stats = LibraryStats {
        track_count: tracks.len(),
        album_count: albums.len(),
        artist_count: artists.len(),
        hi_res_count: tracks.iter().filter(|t| t.is_hi_res()).count(),
        language_study_count: language_study.len(),
        total_bytes: tracks.iter().map(|t| t.size_bytes).sum(),
    };

    PrecomputedCaches {
        albums,
        artists,
        recent,
        language_study,
        stats,
    }
}

/// Orchestrates the set of registered local folders on top of a shared [`LibraryScanner`].
pub struct LocalLibrary {
    scanner: Arc<LibraryScanner>,
}

impl LocalLibrary {
    pub fn new(scanner: Arc<LibraryScanner>) -> Self {
        Self { scanner }
    }

    /// Registers `path` as a managed local folder and runs its first scan (R4.1/R4.7).
    pub fn add_folder(&self, path: &Path) -> Result<LocalMusicFolder> {
        if !path.is_dir() {
            return Err(AppError::FileNotFound(path.display().to_string()));
        }
        let mount_root = path.to_string_lossy().into_owned();
        let display_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| mount_root.clone());
        let now = Utc::now();
        let mut folder = LocalMusicFolder {
            id: uuid::Uuid::new_v4().to_string(),
            path: mount_root,
            display_name,
            date_added: now,
            last_scanned: now,
            track_count: 0,
            total_bytes: 0,
            is_missing: false,
        };

        let summary = self.scanner.scan_tree(path, &folder.path)?;
        let _ = summary; // folder.track_count/total_bytes are recomputed from the cache below.
        let cached = self.scanner.tracks_for_mount(&folder.path)?;
        folder.track_count = cached.len();
        folder.total_bytes = cached.iter().map(|t| t.size_bytes).sum();
        self.scanner.upsert_folder(&folder)?;
        Ok(folder)
    }

    /// Unregisters a folder and drops its cached tracks (R4.6 "Remove Folder from Local Library?").
    pub fn remove_folder(&self, id: &str) -> Result<()> {
        if let Some(folder) = self.list_folders()?.into_iter().find(|f| f.id == id) {
            self.scanner.remove_mount(&folder.path)?;
        }
        self.scanner.remove_folder(id)
    }

    /// Re-points a folder at a new filesystem path, re-scanning under the new root and dropping
    /// the cache for the old one (R4.4 "Relocate Folder…").
    pub fn relocate_folder(&self, id: &str, new_path: &Path) -> Result<LocalMusicFolder> {
        let mut folder = self
            .list_folders()?
            .into_iter()
            .find(|f| f.id == id)
            .ok_or_else(|| AppError::FileNotFound(id.to_string()))?;
        if !new_path.is_dir() {
            return Err(AppError::FileNotFound(new_path.display().to_string()));
        }

        let old_root = folder.path.clone();
        folder.path = new_path.to_string_lossy().into_owned();
        folder.display_name = new_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| folder.path.clone());
        folder.is_missing = false;

        self.scanner.scan_tree(new_path, &folder.path)?;
        if old_root != folder.path {
            self.scanner.remove_mount(&old_root)?;
        }
        let cached = self.scanner.tracks_for_mount(&folder.path)?;
        folder.track_count = cached.len();
        folder.total_bytes = cached.iter().map(|t| t.size_bytes).sum();
        folder.last_scanned = Utc::now();
        self.scanner.upsert_folder(&folder)?;
        Ok(folder)
    }

    /// Re-scans one folder in place (background indexing continuation / manual "Rescan").
    /// Marks the folder `is_missing` rather than erroring when its path no longer exists on
    /// disk, matching the macOS app's "Remove Missing Folders and Tracks?" flow (R4.6), which
    /// acts on that flag rather than failing the rescan outright.
    pub fn rescan_folder(&self, id: &str) -> Result<LocalMusicFolder> {
        let mut folder = self
            .list_folders()?
            .into_iter()
            .find(|f| f.id == id)
            .ok_or_else(|| AppError::FileNotFound(id.to_string()))?;

        let path = PathBuf::from(&folder.path);
        if !path.is_dir() {
            folder.is_missing = true;
            self.scanner.upsert_folder(&folder)?;
            return Ok(folder);
        }

        self.scanner.scan_tree(&path, &folder.path)?;
        let cached = self.scanner.tracks_for_mount(&folder.path)?;
        folder.track_count = cached.len();
        folder.total_bytes = cached.iter().map(|t| t.size_bytes).sum();
        folder.is_missing = false;
        folder.last_scanned = Utc::now();
        self.scanner.upsert_folder(&folder)?;
        Ok(folder)
    }

    pub fn list_folders(&self) -> Result<Vec<LocalMusicFolder>> {
        self.scanner.list_folders()
    }

    /// All tracks across every registered folder (R4.2's `Songs` display mode and friends).
    pub fn tracks(&self) -> Result<Vec<Track>> {
        let folders = self.list_folders()?;
        let mut tracks = Vec::new();
        for folder in folders {
            tracks.extend(self.scanner.tracks_for_mount(&folder.path)?);
        }
        Ok(tracks)
    }

    /// Updates one track's editable metadata fields in the cache (e.g. after the metadata
    /// editor, task 16, commits a change) without a full rescan.
    pub fn update_track(&self, track: &Track, mount_root: &str) -> Result<()> {
        self.scanner.upsert_track(track, mount_root, 0)
    }

    /// Re-scans every registered (non-missing-on-disk) folder, then builds
    /// [`PrecomputedCaches`] from the combined track list — synchronously, on the calling
    /// thread. See [`Self::rescan_all_async`] for the off-thread variant R18.4 asks for.
    pub fn rescan_all(&self) -> Result<PrecomputedCaches> {
        for folder in self.list_folders()? {
            self.rescan_folder(&folder.id)?;
        }
        Ok(precompute(self.tracks()?))
    }

    /// Runs [`Self::rescan_all`] on a background thread and delivers the result over the
    /// returned channel, so the UI thread never blocks on a rescan of a large collection
    /// (R18.4). The app/UI layer polls the receiver each frame (see design.md's threading
    /// model) once this is wired into `AppState` in task 18.
    pub fn rescan_all_async(self: &Arc<Self>) -> Receiver<Result<PrecomputedCaches>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let this = Arc::clone(self);
        std::thread::spawn(move || {
            let _ = tx.send(this.rescan_all());
        });
        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // These tests exercise folder CRUD and `precompute` directly against `Track` values
    // (manually built or manually upserted into the cache) rather than real encoded audio
    // bytes; `library_scanner`'s own tests cover the tag-reading path against the filesystem.

    fn sample_track(title: &str, artist: &str, album: &str, bits: i32, rate: i32) -> Track {
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
            sample_rate: rate,
            bits_per_sample: bits,
            has_artwork: false,
            size_bytes: 1_000,
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
    fn precompute_groups_albums_and_artists_and_counts_hi_res() {
        let t1 = sample_track("Song A", "Artist One", "Album X", 16, 44_100);
        let mut t2 = sample_track("Song B", "Artist One", "Album X", 24, 96_000);
        t2.date_added = Utc::now() + chrono::Duration::seconds(10);
        let t3 = sample_track("Song C", "Artist Two", "Album Y", 16, 44_100);

        let caches = precompute(vec![t1.clone(), t2.clone(), t3.clone()]);

        assert_eq!(caches.stats.track_count, 3);
        assert_eq!(caches.stats.album_count, 2);
        assert_eq!(caches.stats.artist_count, 2);
        assert_eq!(caches.stats.hi_res_count, 1);
        assert_eq!(caches.albums.len(), 2);
        let album_x = caches.albums.iter().find(|a| a.name == "Album X").unwrap();
        assert_eq!(album_x.track_count, 2);
        assert!(album_x.is_hi_res);
        // Most-recently-added first.
        assert_eq!(caches.recent.first().unwrap().title, "Song B");
    }

    #[test]
    fn precompute_filters_language_study_tracks() {
        let mut lesson = sample_track("Lesson 1", "Tutor", "Course", 16, 44_100);
        lesson.full_path = "/music/LEARNING/Course/Lesson 1.flac".into();
        let song = sample_track("Song", "Band", "Album", 16, 44_100);

        let caches = precompute(vec![lesson, song]);
        assert_eq!(caches.language_study.len(), 1);
        assert_eq!(caches.stats.language_study_count, 1);
    }

    fn make_library() -> (tempfile::TempDir, LocalLibrary) {
        let dir = tempfile::tempdir().unwrap();
        let scanner = Arc::new(LibraryScanner::open_in_memory().unwrap());
        (dir, LocalLibrary::new(scanner))
    }

    #[test]
    fn add_list_and_remove_folder_round_trips_persistence() {
        let (dir, lib) = make_library();
        fs::write(dir.path().join("placeholder.mp3"), b"not audio").unwrap();

        let folder = lib.add_folder(dir.path()).unwrap();
        assert_eq!(folder.path, dir.path().to_string_lossy());
        assert!(!folder.is_missing);

        let listed = lib.list_folders().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, folder.id);

        lib.remove_folder(&folder.id).unwrap();
        assert!(lib.list_folders().unwrap().is_empty());
    }

    #[test]
    fn add_folder_rejects_a_missing_path() {
        let (dir, lib) = make_library();
        let missing = dir.path().join("does-not-exist");
        assert!(lib.add_folder(&missing).is_err());
    }

    #[test]
    fn rescan_folder_marks_is_missing_when_path_disappears() {
        let (dir, lib) = make_library();
        let folder = lib.add_folder(dir.path()).unwrap();

        drop(dir); // removes the temp directory from disk
        let rescanned = lib.rescan_folder(&folder.id).unwrap();
        assert!(rescanned.is_missing);
    }

    #[test]
    fn relocate_folder_moves_cache_to_the_new_root() {
        let (old_dir, lib) = make_library();
        let new_dir = tempfile::tempdir().unwrap();
        fs::write(new_dir.path().join("a.mp3"), b"not audio").unwrap();

        let folder = lib.add_folder(old_dir.path()).unwrap();
        let relocated = lib.relocate_folder(&folder.id, new_dir.path()).unwrap();

        assert_eq!(relocated.path, new_dir.path().to_string_lossy());
        assert!(lib.scanner.tracks_for_mount(&old_dir.path().to_string_lossy()).unwrap().is_empty());
    }

    #[test]
    fn rescan_all_async_delivers_precomputed_caches_without_blocking() {
        let (dir, lib) = make_library();
        fs::write(dir.path().join("a.mp3"), b"not audio").unwrap();
        lib.add_folder(dir.path()).unwrap();

        let lib = Arc::new(lib);
        let rx = lib.rescan_all_async();
        let caches = rx.recv().unwrap().unwrap();
        // The placeholder file isn't valid audio, so it never entered the cache; the call
        // completing at all (not hanging, not panicking) is what this test is checking.
        assert_eq!(caches.stats.track_count, 0);
    }
}
