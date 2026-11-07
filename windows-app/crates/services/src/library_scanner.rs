//! SQLite-backed library scanner (spec task 7.1).
//!
//! Ported concept from `library_scanner.rs` in the macOS app: a `tracks_cache` table keyed by
//! full path, populated by a recursive filesystem walk that re-reads tags only for files whose
//! modified-time + size changed since the last scan (R18.1/R18.2). Tag *reading* uses `lofty`
//! directly rather than the native `audio_tagger` readers (task 8) — task 8's native paths exist
//! for Sony-compliant byte-level writing and are not a dependency of the scanner (see design.md's
//! task dependency graph: task 7 depends only on tasks 5 and 6.2).

use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use lofty::file::AudioFile;
use lofty::prelude::{Accessor, TaggedFileExt};
use lofty::probe::Probe;
use rusqlite::{params, Connection, OptionalExtension};
use walkdir::WalkDir;

use walkman_domain::models::{LocalMusicFolder, Track};

use crate::error::Result;

/// Audio file extensions the scanner indexes (lowercase, no dot).
const AUDIO_EXTENSIONS: &[&str] = &["flac", "mp3", "m4a", "wav", "aac", "ogg", "wma"];

/// Current schema version (covers both `tracks_cache` and `local_folders`). Bump and add a step
/// to [`migrate`] when the schema changes; existing rows are preserved across `ALTER TABLE`
/// migrations.
const SCHEMA_VERSION: i64 = 3;

/// Counts produced by a single [`LibraryScanner::scan_tree`] call.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanSummary {
    /// Files found under the root with a recognized audio extension.
    pub seen: usize,
    /// Files newly inserted (not previously cached).
    pub added: usize,
    /// Files re-tagged because their size/modified-time changed.
    pub updated: usize,
    /// Previously-cached rows for this `mount_root` whose file no longer exists and were removed.
    pub removed: usize,
    /// Files that matched an audio extension but failed to read (corrupt/locked); skipped, not
    /// fatal to the scan.
    pub failed: usize,
}

/// Owns the `tracks_cache` SQLite connection and the recursive-scan/change-detection logic.
///
/// `rusqlite::Connection` is `Send` but not `Sync`; the `Mutex` lets one `Arc<LibraryScanner>` be
/// shared across the UI thread and worker threads, matching the design's `Arc<Mutex<...>>`
/// service-sharing convention.
pub struct LibraryScanner {
    conn: Mutex<Connection>,
}

impl LibraryScanner {
    /// Opens (creating if needed) the cache database at `db_path` and runs any pending schema
    /// migrations.
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        Self::configure_and_migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// In-memory database, for tests.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::configure_and_migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn configure_and_migrate(conn: &Connection) -> Result<()> {
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .or_else(|_| conn.execute_batch("PRAGMA synchronous=NORMAL;"))?;
        migrate(conn)?;
        Ok(())
    }

    /// Recursively scans `root`, tagging every cached row with `mount_root` (the drive mount
    /// point or local-folder path this tree belongs to — R18.2's "per-mount" grouping key).
    /// Re-reads tags only for files whose size or modified-time changed since the last scan, and
    /// removes cached rows under `mount_root` whose file is now missing.
    pub fn scan_tree(&self, root: &Path, mount_root: &str) -> Result<ScanSummary> {
        let mut summary = ScanSummary::default();
        let mut seen_paths = std::collections::HashSet::new();

        for entry in WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            if !has_audio_extension(path) {
                continue;
            }
            summary.seen += 1;
            let full_path = path.to_string_lossy().into_owned();
            seen_paths.insert(full_path.clone());

            let metadata = match std::fs::metadata(path) {
                Ok(m) => m,
                Err(_) => {
                    summary.failed += 1;
                    continue;
                }
            };
            let size_bytes = metadata.len() as i64;
            let mtime_secs = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            let cached = self.cached_stat(&full_path)?;
            match cached {
                Some((cached_size, cached_mtime)) if cached_size == size_bytes && cached_mtime == mtime_secs => {
                    // Unchanged: keep the cached row as-is.
                    continue;
                }
                existing => {
                    let is_new = existing.is_none();
                    match read_track(path, root, size_bytes) {
                        Ok(track) => {
                            self.upsert_track(&track, mount_root, mtime_secs)?;
                            if is_new {
                                summary.added += 1;
                            } else {
                                summary.updated += 1;
                            }
                        }
                        Err(_) => summary.failed += 1,
                    }
                }
            }
        }

        summary.removed = self.remove_missing_under(mount_root, &seen_paths)?;
        Ok(summary)
    }

    fn cached_stat(&self, full_path: &str) -> Result<Option<(i64, i64)>> {
        let conn = self.conn.lock().unwrap();
        let row = conn
            .query_row(
                "SELECT size_bytes, mtime_secs FROM tracks_cache WHERE full_path = ?1",
                params![full_path],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?;
        Ok(row)
    }

    /// Inserts or replaces the cached row for `track.full_path`.
    pub fn upsert_track(&self, track: &Track, mount_root: &str, mtime_secs: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let senseme_channels = serde_json::to_string(&track.senseme_channels).unwrap_or_else(|_| "[]".into());
        conn.execute(
            "INSERT INTO tracks_cache (
                full_path, mount_root, filename, relative_path, format, title, artist, album,
                track_num, year, genre, duration_seconds, bitrate_kbps, sample_rate,
                bits_per_sample, has_artwork, size_bytes, date_added, mtime_secs,
                is_senseme_analyzed, senseme_bpm, senseme_valence, senseme_arousal,
                senseme_channels, senseme_highlight_sec
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
                ?19, ?20, ?21, ?22, ?23, ?24, ?25
            )
            ON CONFLICT(full_path) DO UPDATE SET
                mount_root = excluded.mount_root,
                filename = excluded.filename,
                relative_path = excluded.relative_path,
                format = excluded.format,
                title = excluded.title,
                artist = excluded.artist,
                album = excluded.album,
                track_num = excluded.track_num,
                year = excluded.year,
                genre = excluded.genre,
                duration_seconds = excluded.duration_seconds,
                bitrate_kbps = excluded.bitrate_kbps,
                sample_rate = excluded.sample_rate,
                bits_per_sample = excluded.bits_per_sample,
                has_artwork = excluded.has_artwork,
                size_bytes = excluded.size_bytes,
                mtime_secs = excluded.mtime_secs,
                is_senseme_analyzed = excluded.is_senseme_analyzed,
                senseme_bpm = excluded.senseme_bpm,
                senseme_valence = excluded.senseme_valence,
                senseme_arousal = excluded.senseme_arousal,
                senseme_channels = excluded.senseme_channels,
                senseme_highlight_sec = excluded.senseme_highlight_sec",
            params![
                track.full_path,
                mount_root,
                track.filename,
                track.relative_path,
                track.format,
                track.title,
                track.artist,
                track.album,
                track.track_num,
                track.year,
                track.genre,
                track.duration_seconds,
                track.bitrate_kbps,
                track.sample_rate,
                track.bits_per_sample,
                track.has_artwork as i64,
                track.size_bytes,
                track.date_added.to_rfc3339(),
                mtime_secs,
                track.is_senseme_analyzed as i64,
                track.senseme_bpm,
                track.senseme_valence,
                track.senseme_arousal,
                senseme_channels,
                track.senseme_highlight_sec,
            ],
        )?;
        Ok(())
    }

    /// Removes cached rows under `mount_root` whose `full_path` is not in `still_present`.
    /// Returns the number of rows removed.
    fn remove_missing_under(
        &self,
        mount_root: &str,
        still_present: &std::collections::HashSet<String>,
    ) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let mut stale = Vec::new();
        {
            let mut stmt = conn.prepare("SELECT full_path FROM tracks_cache WHERE mount_root = ?1")?;
            let mut rows = stmt.query(params![mount_root])?;
            while let Some(row) = rows.next()? {
                let path: String = row.get(0)?;
                if !still_present.contains(&path) {
                    stale.push(path);
                }
            }
        }
        for path in &stale {
            conn.execute("DELETE FROM tracks_cache WHERE full_path = ?1", params![path])?;
        }
        Ok(stale.len())
    }

    /// Deletes every cached row under `mount_root` (e.g. a local folder removed by the user).
    pub fn remove_mount(&self, mount_root: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM tracks_cache WHERE mount_root = ?1", params![mount_root])?;
        Ok(())
    }

    /// Loads every cached track under `mount_root` (R18.2: per-mount cache load, used by the
    /// duplicate guard and for quick reloads without rescanning).
    pub fn tracks_for_mount(&self, mount_root: &str) -> Result<Vec<Track>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM tracks_cache WHERE mount_root = ?1")?;
        let mut rows = stmt.query(params![mount_root])?;
        let mut tracks = Vec::new();
        while let Some(row) = rows.next()? {
            tracks.push(track_from_row(row)?);
        }
        Ok(tracks)
    }

    /// Loads every cached track across all mounts.
    pub fn all_tracks(&self) -> Result<Vec<Track>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM tracks_cache")?;
        let mut rows = stmt.query([])?;
        let mut tracks = Vec::new();
        while let Some(row) = rows.next()? {
            tracks.push(track_from_row(row)?);
        }
        Ok(tracks)
    }

    /// Lists every registered local folder (R4.1/R4.7 persistence), ordered by `date_added`.
    pub fn list_folders(&self) -> Result<Vec<LocalMusicFolder>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM local_folders ORDER BY date_added ASC")?;
        let mut rows = stmt.query([])?;
        let mut folders = Vec::new();
        while let Some(row) = rows.next()? {
            folders.push(folder_from_row(row)?);
        }
        Ok(folders)
    }

    /// Inserts or replaces the persisted row for `folder.id`.
    pub fn upsert_folder(&self, folder: &LocalMusicFolder) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO local_folders (
                id, path, display_name, date_added, last_scanned, track_count, total_bytes,
                is_missing
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(id) DO UPDATE SET
                path = excluded.path,
                display_name = excluded.display_name,
                last_scanned = excluded.last_scanned,
                track_count = excluded.track_count,
                total_bytes = excluded.total_bytes,
                is_missing = excluded.is_missing",
            params![
                folder.id,
                folder.path,
                folder.display_name,
                folder.date_added.to_rfc3339(),
                folder.last_scanned.to_rfc3339(),
                folder.track_count as i64,
                folder.total_bytes,
                folder.is_missing as i64,
            ],
        )?;
        Ok(())
    }

    /// Removes the persisted folder row for `id` (its cached tracks are removed separately via
    /// [`LibraryScanner::remove_mount`], keyed by the folder's path).
    pub fn remove_folder(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM local_folders WHERE id = ?1", params![id])?;
        Ok(())
    }
}

fn folder_from_row(row: &rusqlite::Row) -> Result<LocalMusicFolder> {
    let date_added: String = row.get("date_added")?;
    let last_scanned: String = row.get("last_scanned")?;
    Ok(LocalMusicFolder {
        id: row.get("id")?,
        path: row.get("path")?,
        display_name: row.get("display_name")?,
        date_added: parse_rfc3339_or_now(&date_added),
        last_scanned: parse_rfc3339_or_now(&last_scanned),
        track_count: row.get::<_, i64>("track_count")? as usize,
        total_bytes: row.get("total_bytes")?,
        is_missing: row.get::<_, i64>("is_missing")? != 0,
    })
}

fn parse_rfc3339_or_now(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn has_audio_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Reads tags via `lofty` and builds a [`Track`] for `path`.
fn read_track(path: &Path, scan_root: &Path, size_bytes: i64) -> Result<Track> {
    let tagged_file = Probe::open(path)
        .map_err(|e| crate::error::AppError::Tag(e.to_string()))?
        .read()
        .map_err(|e| crate::error::AppError::Tag(e.to_string()))?;

    let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());
    let properties = tagged_file.properties();

    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let relative_path = path
        .strip_prefix(scan_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let format = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_uppercase())
        .unwrap_or_default();

    let title = tag
        .and_then(|t| t.title())
        .map(|s| s.into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| file_stem(&filename));
    let artist = tag
        .and_then(|t| t.artist())
        .map(|s| s.into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown Artist".into());
    let album = tag
        .and_then(|t| t.album())
        .map(|s| s.into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown Album".into());
    let genre = tag.and_then(|t| t.genre()).map(|s| s.into_owned()).unwrap_or_default();
    let year = tag.and_then(|t| t.year()).map(|y| y.to_string()).unwrap_or_default();
    let track_num = tag.and_then(|t| t.track()).map(|n| n.to_string()).unwrap_or_default();
    let has_artwork = tag.map(|t| !t.pictures().is_empty()).unwrap_or(false);

    let duration_seconds = properties.duration().as_secs_f64();
    let bitrate_kbps = properties.audio_bitrate().unwrap_or(0) as i32;
    let sample_rate = properties.sample_rate().unwrap_or(44_100) as i32;
    let bits_per_sample = properties.bit_depth().unwrap_or(16) as i32;

    Ok(Track {
        filename,
        relative_path,
        full_path: path.to_string_lossy().into_owned(),
        format,
        title,
        artist,
        album,
        track_num,
        year,
        genre,
        duration_seconds,
        bitrate_kbps,
        sample_rate,
        bits_per_sample,
        has_artwork,
        size_bytes,
        date_added: file_date_added(path),
        is_senseme_analyzed: false,
        senseme_bpm: None,
        senseme_valence: None,
        senseme_arousal: None,
        senseme_channels: Vec::new(),
        senseme_highlight_sec: None,
    })
}

fn file_stem(filename: &str) -> String {
    match filename.rfind('.') {
        Some(idx) if idx > 0 => filename[..idx].to_string(),
        _ => filename.to_string(),
    }
}

fn file_date_added(path: &Path) -> DateTime<Utc> {
    std::fs::metadata(path)
        .and_then(|m| m.created().or_else(|_| m.modified()))
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(|_| Utc::now())
}

fn track_from_row(row: &rusqlite::Row) -> Result<Track> {
    let senseme_channels: String = row.get("senseme_channels")?;
    let senseme_channels: Vec<String> = serde_json::from_str(&senseme_channels).unwrap_or_default();
    let date_added: String = row.get("date_added")?;
    let date_added = parse_rfc3339_or_now(&date_added);

    Ok(Track {
        filename: row.get("filename")?,
        relative_path: row.get("relative_path")?,
        full_path: row.get("full_path")?,
        format: row.get("format")?,
        title: row.get("title")?,
        artist: row.get("artist")?,
        album: row.get("album")?,
        track_num: row.get("track_num")?,
        year: row.get("year")?,
        genre: row.get("genre")?,
        duration_seconds: row.get("duration_seconds")?,
        bitrate_kbps: row.get("bitrate_kbps")?,
        sample_rate: row.get("sample_rate")?,
        bits_per_sample: row.get("bits_per_sample")?,
        has_artwork: row.get::<_, i64>("has_artwork")? != 0,
        size_bytes: row.get("size_bytes")?,
        date_added,
        is_senseme_analyzed: row.get::<_, i64>("is_senseme_analyzed")? != 0,
        senseme_bpm: row.get("senseme_bpm")?,
        senseme_valence: row.get("senseme_valence")?,
        senseme_arousal: row.get("senseme_arousal")?,
        senseme_channels,
        senseme_highlight_sec: row.get("senseme_highlight_sec")?,
    })
}

/// Applies ordered schema migrations up to [`SCHEMA_VERSION`], preserving existing rows.
fn migrate(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
        [],
    )?;
    let current: i64 = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    if current < 1 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tracks_cache (
                full_path TEXT PRIMARY KEY,
                mount_root TEXT NOT NULL,
                filename TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                format TEXT NOT NULL,
                title TEXT NOT NULL,
                artist TEXT NOT NULL,
                album TEXT NOT NULL,
                track_num TEXT NOT NULL,
                year TEXT NOT NULL,
                genre TEXT NOT NULL,
                duration_seconds REAL NOT NULL,
                bitrate_kbps INTEGER NOT NULL,
                sample_rate INTEGER NOT NULL,
                bits_per_sample INTEGER NOT NULL,
                has_artwork INTEGER NOT NULL,
                size_bytes INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_tracks_cache_mount_root ON tracks_cache(mount_root);",
        )?;
    }
    if current < 2 {
        // SenseMe + date columns, added after the base schema shipped (mirrors the macOS app's
        // own schema history per design.md).
        for stmt in [
            "ALTER TABLE tracks_cache ADD COLUMN date_added TEXT NOT NULL DEFAULT '1970-01-01T00:00:00+00:00'",
            "ALTER TABLE tracks_cache ADD COLUMN mtime_secs INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE tracks_cache ADD COLUMN is_senseme_analyzed INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE tracks_cache ADD COLUMN senseme_bpm REAL",
            "ALTER TABLE tracks_cache ADD COLUMN senseme_valence REAL",
            "ALTER TABLE tracks_cache ADD COLUMN senseme_arousal REAL",
            "ALTER TABLE tracks_cache ADD COLUMN senseme_channels TEXT NOT NULL DEFAULT '[]'",
            "ALTER TABLE tracks_cache ADD COLUMN senseme_highlight_sec REAL",
        ] {
            // Columns may already exist if this DB was created fresh at SCHEMA_VERSION 2;
            // ignore "duplicate column" errors so migration is idempotent.
            if let Err(e) = conn.execute(stmt, []) {
                if !e.to_string().contains("duplicate column") {
                    return Err(e.into());
                }
            }
        }
    }
    if current < 3 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS local_folders (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                display_name TEXT NOT NULL,
                date_added TEXT NOT NULL,
                last_scanned TEXT NOT NULL,
                track_count INTEGER NOT NULL,
                total_bytes INTEGER NOT NULL,
                is_missing INTEGER NOT NULL
            );",
        )?;
    }

    conn.execute(
        "INSERT INTO schema_meta (key, value) VALUES ('schema_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![SCHEMA_VERSION.to_string()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_silence_flac(path: &Path) {
        // A tiny valid FLAC file (empty/minimal) is nontrivial to hand-construct; tests instead
        // write a plain file with a recognized extension and accept that `lofty` will fail to
        // parse it as audio, which exercises the `failed` counter and the mtime+size change
        // detection path without needing real encoded audio in the test fixtures.
        fs::write(path, b"not a real audio file").unwrap();
    }

    #[test]
    fn migrate_is_idempotent_and_reaches_current_version() {
        let scanner = LibraryScanner::open_in_memory().unwrap();
        let conn = scanner.conn.lock().unwrap();
        let version: String = conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION.to_string());
        drop(conn);
        // Re-running migrate on the same connection must not error (idempotent ALTERs).
        let conn = scanner.conn.lock().unwrap();
        migrate(&conn).unwrap();
    }

    #[test]
    fn migrating_from_v1_preserves_existing_rows_and_adds_columns() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE tracks_cache (
                full_path TEXT PRIMARY KEY, mount_root TEXT NOT NULL, filename TEXT NOT NULL,
                relative_path TEXT NOT NULL, format TEXT NOT NULL, title TEXT NOT NULL,
                artist TEXT NOT NULL, album TEXT NOT NULL, track_num TEXT NOT NULL,
                year TEXT NOT NULL, genre TEXT NOT NULL, duration_seconds REAL NOT NULL,
                bitrate_kbps INTEGER NOT NULL, sample_rate INTEGER NOT NULL,
                bits_per_sample INTEGER NOT NULL, has_artwork INTEGER NOT NULL,
                size_bytes INTEGER NOT NULL
             );
             INSERT INTO schema_meta (key, value) VALUES ('schema_version', '1');
             INSERT INTO tracks_cache (
                full_path, mount_root, filename, relative_path, format, title, artist, album,
                track_num, year, genre, duration_seconds, bitrate_kbps, sample_rate,
                bits_per_sample, has_artwork, size_bytes
             ) VALUES (
                '/a.flac', '/', 'a.flac', 'a.flac', 'FLAC', 'Old Title', 'Old Artist', 'Old Album',
                '1', '2020', 'Rock', 180.0, 900, 44100, 16, 0, 12345
             );",
        )
        .unwrap();

        migrate(&conn).unwrap();

        let title: String = conn
            .query_row("SELECT title FROM tracks_cache WHERE full_path = '/a.flac'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "Old Title");
        let mtime: i64 = conn
            .query_row("SELECT mtime_secs FROM tracks_cache WHERE full_path = '/a.flac'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mtime, 0);
    }

    #[test]
    fn scan_tree_counts_added_updated_removed_and_failed() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.flac");
        let b = dir.path().join("b.mp3");
        let not_audio = dir.path().join("cover.jpg");
        write_silence_flac(&a);
        write_silence_flac(&b);
        fs::write(&not_audio, b"ignored").unwrap();

        let scanner = LibraryScanner::open_in_memory().unwrap();
        let mount = dir.path().to_string_lossy().into_owned();

        let first = scanner.scan_tree(dir.path(), &mount).unwrap();
        assert_eq!(first.seen, 2); // only .flac/.mp3, not .jpg
        assert_eq!(first.added, 0);
        assert_eq!(first.failed, 2); // placeholder files aren't valid audio
        assert_eq!(first.removed, 0);

        // Second scan with nothing changed: still fails to parse (not valid audio), but the
        // scanner never cached a row for them, so they're retried, not "updated".
        let second = scanner.scan_tree(dir.path(), &mount).unwrap();
        assert_eq!(second.seen, 2);
        assert_eq!(second.failed, 2);

        fs::remove_file(&a).unwrap();
        let third = scanner.scan_tree(dir.path(), &mount).unwrap();
        assert_eq!(third.seen, 1);
    }

    #[test]
    fn tracks_for_mount_round_trips_a_manually_upserted_track() {
        let scanner = LibraryScanner::open_in_memory().unwrap();
        let track = Track {
            filename: "song.flac".into(),
            relative_path: "Artist/Album/song.flac".into(),
            full_path: "/music/Artist/Album/song.flac".into(),
            format: "FLAC".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            track_num: "1".into(),
            year: "2024".into(),
            genre: "Rock".into(),
            duration_seconds: 210.5,
            bitrate_kbps: 900,
            sample_rate: 96_000,
            bits_per_sample: 24,
            has_artwork: true,
            size_bytes: 54_321,
            date_added: Utc::now(),
            is_senseme_analyzed: true,
            senseme_bpm: Some(120.0),
            senseme_valence: Some(0.5),
            senseme_arousal: Some(0.2),
            senseme_channels: vec!["Energetic".into(), "Morning".into()],
            senseme_highlight_sec: Some(42.0),
        };

        scanner.upsert_track(&track, "/music", 1_700_000_000).unwrap();
        let loaded = scanner.tracks_for_mount("/music").unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].title, "Song");
        assert_eq!(loaded[0].bits_per_sample, 24);
        assert!(loaded[0].is_senseme_analyzed);
        assert_eq!(loaded[0].senseme_channels, vec!["Energetic".to_string(), "Morning".to_string()]);

        scanner.remove_mount("/music").unwrap();
        assert!(scanner.tracks_for_mount("/music").unwrap().is_empty());
    }
}
