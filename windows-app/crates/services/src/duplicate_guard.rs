//! Pre-transfer duplicate guard (spec task 10.3, R9.5).
//!
//! A lightweight, *exact-ish* name check against the target drive's `tracks_cache` (via
//! `library_scanner`, task 7.1) — not the fuzzier ±4s/quality-diff clustering
//! `duplicate_finder.rs` (task 11.1) does for the standalone Duplicates view. This one just
//! needs to answer "is a track with this title/artist already on the target?" fast enough to
//! run inline before every transfer, using the cache instead of touching the drive's
//! filesystem.

use std::path::{Path, PathBuf};

use walkman_domain::models::Track;

use crate::error::Result;
use crate::library_scanner::LibraryScanner;

/// What the user chose in the `DuplicateImportGuardSheet` (R9.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateResolution {
    SkipExisting,
    Overwrite,
    KeepBoth,
}

fn normalize(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Returns the first cached track under `mount_root` whose title and artist both match
/// (case/whitespace-insensitively), if any.
pub fn check_for_duplicate(scanner: &LibraryScanner, mount_root: &str, title: &str, artist: &str) -> Result<Option<Track>> {
    let normalized_title = normalize(title);
    let normalized_artist = normalize(artist);
    let cached = scanner.tracks_for_mount(mount_root)?;
    Ok(cached
        .into_iter()
        .find(|t| normalize(&t.title) == normalized_title && normalize(&t.artist) == normalized_artist))
}

/// Checks every item in `items` (`(title, artist)`) against the cache in one pass, returning
/// the indices that have a duplicate — so the UI can show one guard sheet covering a whole batch
/// transfer rather than prompting per item.
pub fn check_batch_for_duplicates(
    scanner: &LibraryScanner,
    mount_root: &str,
    items: &[(String, String)],
) -> Result<Vec<usize>> {
    let cached = scanner.tracks_for_mount(mount_root)?;
    let cached_keys: std::collections::HashSet<(String, String)> =
        cached.iter().map(|t| (normalize(&t.title), normalize(&t.artist))).collect();

    Ok(items
        .iter()
        .enumerate()
        .filter(|(_, (title, artist))| cached_keys.contains(&(normalize(title), normalize(artist))))
        .map(|(i, _)| i)
        .collect())
}

/// Finds a non-colliding path for `DuplicateResolution::KeepBoth`: `Title.ext` → `Title (2).ext`
/// → `Title (3).ext`, etc., checked against the real filesystem.
pub fn resolve_keep_both_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().map(|e| e.to_string_lossy().into_owned());
    let parent = path.parent().unwrap_or(Path::new(""));

    let mut n = 2;
    loop {
        let candidate_name = match &ext {
            Some(ext) => format!("{stem} ({n}).{ext}"),
            None => format!("{stem} ({n})"),
        };
        let candidate = parent.join(candidate_name);
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn track(title: &str, artist: &str) -> Track {
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
            duration_seconds: 200.0,
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
    fn detects_a_duplicate_ignoring_case_and_whitespace() {
        let scanner = LibraryScanner::open_in_memory().unwrap();
        scanner.upsert_track(&track("Dawn", "Nova"), "/music", 0).unwrap();

        let found = check_for_duplicate(&scanner, "/music", "  dawn  ", "NOVA").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().title, "Dawn");
    }

    #[test]
    fn reports_no_duplicate_for_a_track_not_in_the_cache() {
        let scanner = LibraryScanner::open_in_memory().unwrap();
        scanner.upsert_track(&track("Dawn", "Nova"), "/music", 0).unwrap();

        assert!(check_for_duplicate(&scanner, "/music", "Dusk", "Nova").unwrap().is_none());
    }

    #[test]
    fn duplicate_check_is_scoped_to_the_given_mount() {
        let scanner = LibraryScanner::open_in_memory().unwrap();
        scanner.upsert_track(&track("Dawn", "Nova"), "/other-drive", 0).unwrap();

        assert!(check_for_duplicate(&scanner, "/music", "Dawn", "Nova").unwrap().is_none());
    }

    #[test]
    fn batch_check_returns_indices_of_colliding_items_only() {
        let scanner = LibraryScanner::open_in_memory().unwrap();
        scanner.upsert_track(&track("Dawn", "Nova"), "/music", 0).unwrap();

        let items = vec![
            ("Dawn".to_string(), "Nova".to_string()),   // duplicate
            ("Dusk".to_string(), "Nova".to_string()),   // not a duplicate
            ("dawn".to_string(), "nova".to_string()),   // duplicate (case-insensitive)
        ];
        let indices = check_batch_for_duplicates(&scanner, "/music", &items).unwrap();
        assert_eq!(indices, vec![0, 2]);
    }

    #[test]
    fn keep_both_path_is_unchanged_when_nothing_collides() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Song.flac");
        assert_eq!(resolve_keep_both_path(&path), path);
    }

    #[test]
    fn keep_both_path_increments_until_free() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Song.flac");
        std::fs::write(&path, b"x").unwrap();
        std::fs::write(dir.path().join("Song (2).flac"), b"x").unwrap();

        let resolved = resolve_keep_both_path(&path);
        assert_eq!(resolved, dir.path().join("Song (3).flac"));
    }

    #[test]
    fn keep_both_path_handles_extensionless_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("README");
        std::fs::write(&path, b"x").unwrap();

        let resolved = resolve_keep_both_path(&path);
        assert_eq!(resolved, dir.path().join("README (2)"));
    }
}
