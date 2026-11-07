//! Clean & Eject (spec task 9.4): removes device junk, then safely ejects (R16.1/R16.2).
//!
//! The actual flush+lock+dismount+eject sequence (with its watchdog) lives in
//! `platform::windows::WindowsEjector` (task 6.5); this module is the junk-removal pass that
//! runs before it, plus the orchestration that ties the two together.

use std::path::Path;

use walkman_platform::{EjectError, Ejector};

/// Per-directory junk *files* removed recursively: residual macOS artifacts (R16.1) and the
/// Windows equivalents. `._*` AppleDouble sidecar files are matched by prefix, not listed here.
const JUNK_FILE_NAMES: &[&str] = &[".DS_Store", "Thumbs.db", "desktop.ini"];

/// Top-level-only junk *folders*. `System Volume Information` is usually inaccessible without
/// elevated privileges — removal is best-effort and its failure is not an error (R16.1: "where
/// permitted").
const JUNK_FOLDER_NAMES: &[&str] = &[
    "$RECYCLE.BIN",
    "System Volume Information",
    ".Spotlight-V100",
    ".Trashes",
    ".fseventsd",
];

/// Counts produced by [`clean_junk`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CleanSummary {
    pub files_removed: usize,
    pub folders_removed: usize,
}

/// Recursively removes junk files from every directory under `root`, and the top-level junk
/// folders if present. Every removal is best-effort: a permission-denied folder (very commonly
/// `System Volume Information`) is skipped, not treated as a failure.
pub fn clean_junk(root: &Path) -> CleanSummary {
    let mut summary = CleanSummary::default();
    clean_junk_recursive(root, &mut summary);

    for name in JUNK_FOLDER_NAMES {
        let path = root.join(name);
        if path.is_dir() && std::fs::remove_dir_all(&path).is_ok() {
            summary.folders_removed += 1;
        }
    }

    summary
}

fn clean_junk_recursive(dir: &Path, summary: &mut CleanSummary) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return; // permission denied or vanished mid-scan — skip, don't fail the whole pass.
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else { continue };

        if file_type.is_dir() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if JUNK_FOLDER_NAMES.iter().any(|j| j.eq_ignore_ascii_case(&name)) {
                continue; // handled at the top level by `clean_junk`, not recursed into here.
            }
            clean_junk_recursive(&path, summary);
        } else if file_type.is_file() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_junk = JUNK_FILE_NAMES.iter().any(|j| j.eq_ignore_ascii_case(&name)) || name.starts_with("._");
            if is_junk && std::fs::remove_file(&path).is_ok() {
                summary.files_removed += 1;
            }
        }
    }
}

/// Result of [`clean_and_eject`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CleanAndEjectResult {
    pub cleaned: CleanSummary,
}

/// Runs the junk-removal pass over `mount`, then ejects it via `ejector` (which itself flushes
/// buffered writes before locking/dismounting — see `platform::windows::WindowsEjector`, and is
/// already watchdog-bounded so this function can't hang).
pub fn clean_and_eject(mount: &Path, ejector: &dyn Ejector) -> Result<CleanAndEjectResult, EjectError> {
    let cleaned = clean_junk(mount);
    ejector.eject(mount)?;
    Ok(CleanAndEjectResult { cleaned })
}

#[cfg(test)]
mod tests {
    use super::*;
    use walkman_platform::fake::FakeEjector;

    #[test]
    fn removes_known_junk_files_recursively() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".DS_Store"), b"").unwrap();
        std::fs::write(dir.path().join("Thumbs.db"), b"").unwrap();
        std::fs::write(dir.path().join("desktop.ini"), b"").unwrap();
        std::fs::write(dir.path().join("._SidecarFile"), b"").unwrap();
        std::fs::write(dir.path().join("real_song.flac"), b"keep me").unwrap();

        let sub = dir.path().join("MUSIC/Artist");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join(".DS_Store"), b"").unwrap();
        std::fs::write(sub.join("track.mp3"), b"keep me too").unwrap();

        let summary = clean_junk(dir.path());
        assert_eq!(summary.files_removed, 5);
        assert!(dir.path().join("real_song.flac").exists());
        assert!(sub.join("track.mp3").exists());
        assert!(!dir.path().join(".DS_Store").exists());
        assert!(!sub.join(".DS_Store").exists());
    }

    #[test]
    fn removes_top_level_junk_folders_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("$RECYCLE.BIN")).unwrap();
        std::fs::write(dir.path().join("$RECYCLE.BIN/junk.dat"), b"").unwrap();
        std::fs::create_dir(dir.path().join(".fseventsd")).unwrap();

        // A same-named folder nested deeper should NOT be treated as a top-level junk folder
        // (it's merely skipped from recursion by name, matching real Sony/Windows layouts where
        // these only ever appear at the volume root).
        std::fs::create_dir_all(dir.path().join("MUSIC/$RECYCLE.BIN")).unwrap();

        let summary = clean_junk(dir.path());
        assert_eq!(summary.folders_removed, 2);
        assert!(!dir.path().join("$RECYCLE.BIN").exists());
        assert!(!dir.path().join(".fseventsd").exists());
    }

    #[test]
    fn clean_and_eject_cleans_then_ejects_and_reports_both() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Thumbs.db"), b"").unwrap();

        let ejector = FakeEjector::new();
        let result = clean_and_eject(dir.path(), &ejector).unwrap();
        assert_eq!(result.cleaned.files_removed, 1);
        assert_eq!(ejector.calls(), vec![dir.path().to_path_buf()]);
    }

    #[test]
    fn clean_and_eject_propagates_an_eject_failure() {
        let dir = tempfile::tempdir().unwrap();
        let ejector = FakeEjector::new();
        ejector.fail_with(EjectError::Busy);

        let err = clean_and_eject(dir.path(), &ejector).unwrap_err();
        assert!(matches!(err, EjectError::Busy));
    }

    #[test]
    fn cleaning_a_nonexistent_directory_is_a_harmless_no_op() {
        let summary = clean_junk(Path::new(r"Z:\definitely\does\not\exist"));
        assert_eq!(summary, CleanSummary::default());
    }
}
