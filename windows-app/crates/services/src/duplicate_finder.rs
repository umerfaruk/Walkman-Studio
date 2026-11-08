//! Same-storage duplicate finder (spec task 11.1, R10.1-10.5).
//!
//! Operates on a single partition's worth of tracks at a time (R10.1: "scoped strictly within
//! the same storage partition... to prevent cross-drive deletion") — callers pass the tracks for
//! one mount (from `library_scanner::tracks_for_mount`), never a mixed set. This is a different,
//! fuzzier matching pass than `duplicate_guard.rs`'s task-10.3 pre-transfer check: it clusters by
//! duration (±4s) within a normalized title/artist key rather than requiring an exact match,
//! since re-encoded or re-trimmed copies of the same recording rarely have byte-identical
//! durations.

use std::path::{Path, PathBuf};

use walkman_domain::models::Track;

use crate::error::Result;

/// R10.2's two matching scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateScope {
    /// Group only within the same album — the safe default.
    ExactSameAlbum,
    /// Group across albums too (e.g. a song that also appears on a greatest-hits compilation).
    CrossAlbum,
}

/// R10.4's sort options for the results view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateSortOption {
    SpaceRecoverable,
    TitleAZ,
    ArtistAZ,
    MostCopies,
}

/// One cluster of tracks considered duplicates of each other.
#[derive(Debug, Clone, PartialEq)]
pub struct DuplicateGroup {
    pub tracks: Vec<Track>,
    /// `true` when the member tracks differ in bitrate, sample rate, bit depth, or format
    /// (R10.3's "Quality Diff").
    pub has_quality_diff: bool,
}

impl DuplicateGroup {
    /// The track [`auto_resolve`] would keep: highest bit depth × sample rate, tie-broken by
    /// bitrate, then by largest file size.
    pub fn best_quality_track(&self) -> &Track {
        self.tracks
            .iter()
            .max_by_key(|t| {
                (
                    t.bits_per_sample as i64 * t.sample_rate as i64,
                    t.bitrate_kbps as i64,
                    t.size_bytes,
                )
            })
            .expect("DuplicateGroup is never constructed empty")
    }

    /// Bytes recoverable by removing every member except [`Self::best_quality_track`].
    pub fn recoverable_bytes(&self) -> i64 {
        let best = self.best_quality_track().full_path.clone();
        self.tracks.iter().filter(|t| t.full_path != best).map(|t| t.size_bytes).sum()
    }

    /// Representative title/artist for display (all members share these after normalization).
    pub fn title(&self) -> &str {
        &self.tracks[0].title
    }
    pub fn artist(&self) -> &str {
        &self.tracks[0].artist
    }
}

/// Lowercased, whitespace-collapsed/trimmed key. Deliberately does **not** strip version tags
/// like "(Live)"/"(Acoustic)"/"(Remix)"/"(Instrumental)" — R10.2 requires those stay distinct,
/// and leaving them in the string is exactly what keeps them from matching.
fn normalize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Maximum duration gap (seconds) within a single duplicate cluster (R10.2).
const DURATION_CLUSTER_TOLERANCE_SECONDS: f64 = 4.0;

/// Finds duplicate groups within `tracks` (already scoped to one storage partition) under
/// `scope`.
pub fn find_duplicates(tracks: &[Track], scope: DuplicateScope) -> Vec<DuplicateGroup> {
    use std::collections::BTreeMap;

    let mut buckets: BTreeMap<(String, String, String), Vec<Track>> = BTreeMap::new();
    for track in tracks {
        let album_key = match scope {
            DuplicateScope::ExactSameAlbum => normalize(&track.album),
            DuplicateScope::CrossAlbum => String::new(),
        };
        let key = (normalize(&track.title), normalize(&track.artist), album_key);
        buckets.entry(key).or_default().push(track.clone());
    }

    let mut groups = Vec::new();
    for mut bucket in buckets.into_values() {
        if bucket.len() < 2 {
            continue;
        }
        bucket.sort_by(|a, b| a.duration_seconds.total_cmp(&b.duration_seconds));

        let mut cluster: Vec<Track> = vec![bucket[0].clone()];
        for track in bucket.into_iter().skip(1) {
            let gap = track.duration_seconds - cluster.last().unwrap().duration_seconds;
            if gap > DURATION_CLUSTER_TOLERANCE_SECONDS {
                push_group_if_duplicate(&mut groups, std::mem::take(&mut cluster));
            }
            cluster.push(track);
        }
        push_group_if_duplicate(&mut groups, cluster);
    }

    groups
}

fn push_group_if_duplicate(groups: &mut Vec<DuplicateGroup>, cluster: Vec<Track>) {
    if cluster.len() < 2 {
        return;
    }
    let has_quality_diff = cluster.iter().any(|t| {
        let first = &cluster[0];
        t.bitrate_kbps != first.bitrate_kbps
            || t.sample_rate != first.sample_rate
            || t.bits_per_sample != first.bits_per_sample
            || t.format != first.format
    });
    groups.push(DuplicateGroup { tracks: cluster, has_quality_diff });
}

/// Sorts `groups` in place per [`DuplicateSortOption`] (R10.4). `MostCopies` and
/// `SpaceRecoverable` sort descending (most relevant first); the A–Z options sort ascending.
pub fn sort_groups(groups: &mut [DuplicateGroup], option: DuplicateSortOption) {
    match option {
        DuplicateSortOption::SpaceRecoverable => {
            groups.sort_by_key(|g| std::cmp::Reverse(g.recoverable_bytes()));
        }
        DuplicateSortOption::TitleAZ => groups.sort_by_key(|g| normalize(g.title())),
        DuplicateSortOption::ArtistAZ => groups.sort_by_key(|g| normalize(g.artist())),
        DuplicateSortOption::MostCopies => groups.sort_by_key(|g| std::cmp::Reverse(g.tracks.len())),
    }
}

/// Total recoverable bytes across every group (R10.4's "recoverable-space totals").
pub fn total_recoverable_bytes(groups: &[DuplicateGroup]) -> i64 {
    groups.iter().map(DuplicateGroup::recoverable_bytes).sum()
}

/// R10.4's `Auto-Resolve Quality`: for each group, every track except
/// [`DuplicateGroup::best_quality_track`] is deleted (audio file + companion `.lrc`/`.txt`, if
/// present). Returns the full-paths actually removed. A failure to delete one track's files is
/// logged and skipped rather than aborting the whole batch.
///
/// `confirmed` must be `true` (R19.3/19.6: delete operations require an explicit confirmation
/// flag) — the caller (the UI's own confirm sheet, already shown before this is ever reached)
/// is the thing asserting the user actually agreed; this is this service's own
/// defense-in-depth check against a caller that skipped it, not a second UI prompt.
pub fn auto_resolve_quality(groups: &[DuplicateGroup], confirmed: bool) -> Vec<String> {
    if !confirmed {
        log::warn!("auto_resolve_quality called without confirmation; refusing to delete anything");
        return Vec::new();
    }
    let mut removed = Vec::new();
    for group in groups {
        let best = group.best_quality_track().full_path.clone();
        for track in &group.tracks {
            if track.full_path == best {
                continue;
            }
            match delete_track_and_companions(Path::new(&track.full_path), true) {
                Ok(()) => removed.push(track.full_path.clone()),
                Err(e) => log::warn!("auto-resolve: failed to delete {}: {e}", track.full_path),
            }
        }
    }
    removed
}

/// Deletes `audio_path` plus any companion `.lrc`/`.txt` lyrics file beside it (R10.4).
///
/// `confirmed` must be `true` (R19.3/19.6: delete operations require an explicit confirmation
/// flag), the same defense-in-depth reasoning as [`auto_resolve_quality`]'s own parameter —
/// refuses with an error and touches nothing on disk when `false`, rather than silently
/// no-oping, so a caller that forgets to confirm gets a loud failure instead of a quiet one.
pub fn delete_track_and_companions(audio_path: &Path, confirmed: bool) -> Result<()> {
    if !confirmed {
        return Err(crate::error::AppError::Tag(format!("refusing to delete {} without explicit confirmation", audio_path.display())));
    }
    std::fs::remove_file(audio_path)?;
    for ext in ["lrc", "txt"] {
        let companion = audio_path.with_extension(ext);
        if companion.exists() {
            let _ = std::fs::remove_file(companion); // best-effort; the audio file is the one that matters
        }
    }
    Ok(())
}

/// Resolves the on-disk folder to reveal for `track` (R10.4's "reveal-in-Explorer") — its
/// parent directory, since `platform::FileDialogs::reveal_in_file_manager` expects a file path
/// to select within its containing folder, which is simply `track.full_path` itself.
pub fn reveal_path(track: &Track) -> PathBuf {
    PathBuf::from(&track.full_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[allow(clippy::too_many_arguments)]
    fn track(path: &str, title: &str, artist: &str, album: &str, duration: f64, bits: i32, rate: i32, size: i64) -> Track {
        Track {
            filename: path.to_string(),
            relative_path: path.to_string(),
            full_path: path.to_string(),
            format: "FLAC".into(),
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            track_num: "1".into(),
            year: "2024".into(),
            genre: String::new(),
            duration_seconds: duration,
            bitrate_kbps: 900,
            sample_rate: rate,
            bits_per_sample: bits,
            has_artwork: false,
            size_bytes: size,
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
    fn finds_an_exact_duplicate_within_the_duration_tolerance() {
        let tracks = vec![
            track("/a.flac", "Dawn", "Nova", "Horizon", 200.0, 16, 44_100, 30_000_000),
            track("/b.flac", "dawn", "NOVA", "horizon", 202.5, 24, 96_000, 60_000_000),
        ];
        let groups = find_duplicates(&tracks, DuplicateScope::ExactSameAlbum);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].tracks.len(), 2);
        assert!(groups[0].has_quality_diff);
    }

    #[test]
    fn does_not_group_tracks_with_too_large_a_duration_gap() {
        let tracks = vec![
            track("/a.flac", "Dawn", "Nova", "Horizon", 60.0, 16, 44_100, 1),
            track("/b.flac", "Dawn", "Nova", "Horizon", 180.0, 16, 44_100, 1),
        ];
        assert!(find_duplicates(&tracks, DuplicateScope::ExactSameAlbum).is_empty());
    }

    #[test]
    fn preserves_distinct_version_tags_as_separate_tracks() {
        let tracks = vec![
            track("/a.flac", "Song", "Artist", "Album", 200.0, 16, 44_100, 1),
            track("/b.flac", "Song (Live)", "Artist", "Album", 200.0, 16, 44_100, 1),
        ];
        assert!(find_duplicates(&tracks, DuplicateScope::ExactSameAlbum).is_empty());
    }

    #[test]
    fn exact_scope_does_not_cross_albums_but_cross_album_scope_does() {
        let tracks = vec![
            track("/a.flac", "Song", "Artist", "Album One", 200.0, 16, 44_100, 1),
            track("/b.flac", "Song", "Artist", "Album Two", 200.0, 16, 44_100, 1),
        ];
        assert!(find_duplicates(&tracks, DuplicateScope::ExactSameAlbum).is_empty());
        assert_eq!(find_duplicates(&tracks, DuplicateScope::CrossAlbum).len(), 1);
    }

    #[test]
    fn recoverable_bytes_excludes_only_the_best_quality_copy() {
        let tracks = vec![
            track("/a.flac", "Dawn", "Nova", "Horizon", 200.0, 16, 44_100, 10_000_000),
            track("/b.flac", "Dawn", "Nova", "Horizon", 200.0, 24, 96_000, 60_000_000),
            track("/c.flac", "Dawn", "Nova", "Horizon", 200.0, 16, 44_100, 12_000_000),
        ];
        let groups = find_duplicates(&tracks, DuplicateScope::ExactSameAlbum);
        assert_eq!(groups[0].best_quality_track().full_path, "/b.flac");
        assert_eq!(groups[0].recoverable_bytes(), 10_000_000 + 12_000_000);
    }

    #[test]
    fn sort_by_most_copies_and_title_az() {
        let pair = vec![
            track("/a.flac", "Zeta", "Artist", "Album", 200.0, 16, 44_100, 1),
            track("/b.flac", "Zeta", "Artist", "Album", 200.0, 16, 44_100, 1),
        ];
        let triple = vec![
            track("/c.flac", "Alpha", "Artist", "Album", 100.0, 16, 44_100, 1),
            track("/d.flac", "Alpha", "Artist", "Album", 100.0, 16, 44_100, 1),
            track("/e.flac", "Alpha", "Artist", "Album", 100.0, 16, 44_100, 1),
        ];
        let mut all = pair;
        all.extend(triple);
        let mut groups = find_duplicates(&all, DuplicateScope::ExactSameAlbum);

        sort_groups(&mut groups, DuplicateSortOption::MostCopies);
        assert_eq!(groups[0].title(), "Alpha"); // the triple

        sort_groups(&mut groups, DuplicateSortOption::TitleAZ);
        assert_eq!(groups[0].title(), "Alpha");
        assert_eq!(groups[1].title(), "Zeta");
    }

    #[test]
    fn auto_resolve_deletes_lower_quality_copies_and_their_lrc_companions() {
        let dir = tempfile::tempdir().unwrap();
        let low = dir.path().join("low.flac");
        let high = dir.path().join("high.flac");
        let low_lrc = dir.path().join("low.lrc");
        std::fs::write(&low, b"low quality").unwrap();
        std::fs::write(&high, b"high quality").unwrap();
        std::fs::write(&low_lrc, b"[00:00.00]lyrics").unwrap();

        let tracks = vec![
            track(low.to_str().unwrap(), "Dawn", "Nova", "Horizon", 200.0, 16, 44_100, 1),
            track(high.to_str().unwrap(), "Dawn", "Nova", "Horizon", 200.0, 24, 96_000, 2),
        ];
        let groups = find_duplicates(&tracks, DuplicateScope::ExactSameAlbum);
        let removed = auto_resolve_quality(&groups, true);

        assert_eq!(removed, vec![low.to_str().unwrap().to_string()]);
        assert!(!low.exists());
        assert!(!low_lrc.exists());
        assert!(high.exists());
    }

    #[test]
    fn auto_resolve_quality_without_confirmation_deletes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let low = dir.path().join("low.flac");
        let high = dir.path().join("high.flac");
        std::fs::write(&low, b"low quality").unwrap();
        std::fs::write(&high, b"high quality").unwrap();

        let tracks = vec![
            track(low.to_str().unwrap(), "Dawn", "Nova", "Horizon", 200.0, 16, 44_100, 1),
            track(high.to_str().unwrap(), "Dawn", "Nova", "Horizon", 200.0, 24, 96_000, 2),
        ];
        let groups = find_duplicates(&tracks, DuplicateScope::ExactSameAlbum);
        let removed = auto_resolve_quality(&groups, false);

        assert!(removed.is_empty());
        assert!(low.exists(), "nothing should be deleted without confirmation");
        assert!(high.exists());
    }

    #[test]
    fn delete_track_and_companions_without_confirmation_errors_and_deletes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let audio = dir.path().join("song.flac");
        std::fs::write(&audio, b"audio bytes").unwrap();

        assert!(delete_track_and_companions(&audio, false).is_err());
        assert!(audio.exists(), "the file must survive an unconfirmed delete call");
    }

    #[test]
    fn total_recoverable_bytes_sums_across_groups() {
        let tracks = vec![
            track("/a.flac", "Dawn", "Nova", "Horizon", 200.0, 16, 44_100, 5),
            track("/b.flac", "Dawn", "Nova", "Horizon", 200.0, 24, 96_000, 50),
            track("/c.flac", "Dusk", "Nova", "Horizon", 100.0, 16, 44_100, 7),
            track("/d.flac", "Dusk", "Nova", "Horizon", 100.0, 24, 96_000, 70),
        ];
        let groups = find_duplicates(&tracks, DuplicateScope::ExactSameAlbum);
        assert_eq!(total_recoverable_bytes(&groups), 5 + 7);
    }
}
