//! Core Walkman engine (spec task 9.3): drive detection, target track paths, `.m3u8` playlist
//! I/O, byte/duration formatters, and the companion `.lrc` writer.
//!
//! macOS source: `WalkmanEngine.swift`. The sanitization/path-layout math itself already lives
//! in `domain::sanitize` (task 5.3); this module is the I/O layer around it — nothing here is
//! platform-specific beyond going through `walkman_platform::VolumeProvider` for drive
//! enumeration/free-space.

use std::path::{Path, PathBuf};

use walkman_domain::models::{Playlist, WalkmanDevice, WalkmanDeviceInfo, WalkmanDrive};
use walkman_domain::sanitize::{sanitize_filename, target_track_relative_path, DEFAULT_MAX_FILENAME_LEN};
use walkman_platform::VolumeProvider;

use crate::device_detector::{inspect_volume, resolve_model};
use crate::error::Result;

/// Filenames checked for a custom-firmware marker (R3.5's `has_cfw` flag). No authoritative
/// list of Sony Walkman CFW install artifacts is available to this port; this is a best-effort
/// placeholder set, documented as such rather than guessed silently.
const CFW_MARKER_NAMES: &[&str] = &["wmport", "CFW", "webdav_cfw"];

/// Enumerates every authentic Sony-signature volume (R3.2) into [`WalkmanDrive`]s, using
/// `volumes` for the raw drive-letter list and free-space figures.
pub fn detect_drives(volumes: &dyn VolumeProvider) -> Vec<WalkmanDrive> {
    volumes
        .list_volumes()
        .into_iter()
        .filter_map(|v| {
            let signature = inspect_volume(&v.path);
            if !signature.is_authentic {
                return None;
            }
            let free = volumes.free_space(&v.path).ok()?;
            Some(WalkmanDrive {
                name: v.label.unwrap_or_else(|| "WALKMAN".to_string()),
                mount_point: v.path.to_string_lossy().into_owned(),
                drive_type: signature.drive_type,
                total_bytes: free.total_bytes,
                used_bytes: free.used_bytes(),
                free_bytes: free.free_bytes,
                percent_used: free.percent_used(),
                has_cfw: has_cfw_marker(&v.path),
                music_dir: music_dir_for(&v.path),
            })
        })
        .collect()
}

/// Groups every authentic Sony-signature volume into [`WalkmanDevice`]s (design.md's
/// `detect_devices`: "group internal+microsd into devices"). Grouping key is the model token
/// read from each volume's `default-capability.xml` (via [`inspect_volume`]); a microSD card
/// normally carries no capability XML of its own, so an untokened `Microsd`-type drive is
/// attached to the sole identified device when there is exactly one (the common
/// single-Walkman-attached case). **Known limitations**: with more than one *identified* device
/// attached at once, an untokened microSD card has no signal to say which one it's in and is
/// left unattached; an authentic but completely unidentifiable `Internal` drive (no token, no
/// resolvable PID) can't become a device at all, since there's nothing to name it with; and this
/// can't tell two physically distinct Walkmans of the *same* model apart, since nothing here
/// correlates a volume back to a specific USB device instance (that would need SetupAPI
/// device-to-volume correlation, not implemented) — it only splits devices that resolve to
/// *different* models. `firmware_version` and `supported_audio_formats` aren't derivable from
/// the (unknown, best-effort-scanned) capability-XML schema either, so they're placeholder
/// values — see `inspect_volume`'s doc comment for the same honesty note on that schema gap.
pub fn detect_devices(volumes: &dyn VolumeProvider) -> Vec<WalkmanDevice> {
    struct Grouped {
        model_token: Option<String>,
        drives: Vec<WalkmanDrive>,
        max_capacity_gb: f64,
    }

    // Pass 1: build a `WalkmanDrive` for every authentic volume, keeping whichever model token
    // `inspect_volume` found (microSD cards normally carry no capability XML of their own, so
    // theirs is usually `None` — handled in pass 2).
    let mut untokened_microsd: Vec<WalkmanDrive> = Vec::new();
    let mut groups: std::collections::BTreeMap<String, Grouped> = std::collections::BTreeMap::new();

    for v in volumes.list_volumes() {
        let signature = inspect_volume(&v.path);
        if !signature.is_authentic {
            continue;
        }
        let Ok(free) = volumes.free_space(&v.path) else { continue };
        let capacity_gb = free.total_bytes as f64 / 1_000_000_000.0;
        let drive = WalkmanDrive {
            name: v.label.clone().unwrap_or_else(|| "WALKMAN".to_string()),
            mount_point: v.path.to_string_lossy().into_owned(),
            drive_type: signature.drive_type,
            total_bytes: free.total_bytes,
            used_bytes: free.used_bytes(),
            free_bytes: free.free_bytes,
            percent_used: free.percent_used(),
            has_cfw: has_cfw_marker(&v.path),
            music_dir: music_dir_for(&v.path),
        };

        match &signature.model_token {
            Some(token) => {
                let group = groups.entry(token.clone()).or_insert_with(|| Grouped {
                    model_token: Some(token.clone()),
                    drives: Vec::new(),
                    max_capacity_gb: 0.0,
                });
                group.max_capacity_gb = group.max_capacity_gb.max(capacity_gb);
                group.drives.push(drive);
            }
            None if signature.drive_type == walkman_domain::models::DriveType::Microsd => {
                // Deferred to pass 2: attach to the sole identified device, if there is one.
                untokened_microsd.push(drive);
            }
            None => {
                // An authentic but unidentified Internal drive: nothing to attach it to or name
                // it with, so it can't become a `WalkmanDevice` — see the function doc.
            }
        }
    }

    // Pass 2: an untokened microSD card almost always belongs to the single device that *was*
    // identified. With more than one identified device attached at once there's no signal here
    // to say which one it's in, so it's left unattached in that (rarer) ambiguous case.
    if untokened_microsd.len() == 1 && groups.len() == 1 {
        let group = groups.values_mut().next().unwrap();
        let drive = untokened_microsd.remove(0);
        group.max_capacity_gb = group.max_capacity_gb.max(drive.total_bytes as f64 / 1_000_000_000.0);
        group.drives.push(drive);
    }

    groups
        .into_values()
        .filter_map(|group| {
            let (catalog, specific_model) =
                resolve_model(None, group.model_token.as_deref(), Some(group.max_capacity_gb))?;
            let has_cfw = group.drives.iter().any(|d| d.has_cfw);
            let device_info = WalkmanDeviceInfo {
                model_raw: group.model_token.unwrap_or_else(|| specific_model.clone()),
                marketing_name: catalog.marketing_name.to_string(),
                display_name: format!("Sony Walkman {specific_model}"),
                series_name: catalog.series_name.to_string(),
                specific_model,
                vendor: "Sony Corporation".to_string(),
                firmware_version: String::new(), // not derivable from the (unknown) XML schema
                is_non_android: catalog.is_non_android,
                has_cfw,
                display_resolution: catalog.display_resolution.to_string(),
                internal_capacity_gb: group.max_capacity_gb,
                supported_audio_formats: default_supported_formats(catalog.is_non_android),
            };
            Some(WalkmanDevice::new(device_info, group.drives))
        })
        .collect()
}

fn default_supported_formats(is_non_android: bool) -> Vec<String> {
    let base = ["MP3", "FLAC", "WAV", "AAC"];
    // Android Walkman OS exposes the broader Android media framework's codec set.
    let android_extra = ["ALAC", "OGG"];
    base.iter()
        .chain((!is_non_android).then_some(android_extra.as_slice()).into_iter().flatten())
        .map(|s| s.to_string())
        .collect()
}

fn music_dir_for(mount: &Path) -> String {
    for name in ["MUSIC", "Music"] {
        let candidate = mount.join(name);
        if candidate.is_dir() {
            return candidate.to_string_lossy().into_owned();
        }
    }
    mount.join("MUSIC").to_string_lossy().into_owned()
}

fn has_cfw_marker(mount: &Path) -> bool {
    CFW_MARKER_NAMES.iter().any(|n| mount.join(n).exists())
}

/// A user-selected custom storage folder, for the Android MTP/custom-folder bridge (R3.7).
/// macOS stores a security-scoped bookmark here; Windows has no such sandboxing concept, so a
/// plain path is enough (design.md: "custom-storage folder (plain stored path, no bookmark)").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomStorageFolder {
    pub path: PathBuf,
}

impl CustomStorageFolder {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn music_dir(&self) -> PathBuf {
        self.path.join("MUSIC")
    }

    pub fn learning_dir(&self) -> PathBuf {
        self.path.join("LEARNING")
    }
}

/// Builds the on-disk path for a track under `music_dir`: `Artist/Album/NN - Title.ext`
/// (sanitized, zero-padded), joined with native path separators.
pub fn target_track_fs_path(music_dir: &Path, artist: &str, album: &str, track_num: &str, title: &str, ext: &str) -> PathBuf {
    let relative = target_track_relative_path(artist, album, track_num, title, ext);
    let mut path = music_dir.to_path_buf();
    for part in relative.split('/') {
        path.push(part);
    }
    path
}

/// Formats a byte count the way the macOS app's storage gauges do: whole bytes under 1 KB, one
/// decimal place from KB upward.
pub fn format_bytes(bytes: i64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes.max(0) as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", value as i64, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Formats a duration in seconds as `h:mm:ss` (when an hour or more) or `m:ss`.
pub fn format_duration(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as i64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Writes `tracks` (relative POSIX path, `#EXTINF` display text, duration seconds) to `path` as
/// a Sony-compliant `.m3u8`: `#EXTM3U` header, `#EXTINF:<seconds>,<text>` before each entry,
/// strict CRLF line endings (R11.1).
pub fn write_playlist(path: &Path, tracks: &[(String, String, f64)]) -> Result<()> {
    let mut content = String::from("#EXTM3U\r\n");
    for (relative_path, display_text, duration_seconds) in tracks {
        content.push_str(&format!("#EXTINF:{},{display_text}\r\n", duration_seconds.round() as i64));
        content.push_str(relative_path);
        content.push_str("\r\n");
    }
    std::fs::write(path, content)?;
    Ok(())
}

/// Reads a `.m3u8` back into a [`Playlist`], ignoring `#EXTM3U`/`#EXTINF` directive lines.
pub fn read_playlist(path: &Path) -> Result<Playlist> {
    let content = std::fs::read_to_string(path)?;
    let tracks: Vec<String> = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect();

    let filename = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let name = filename.trim_end_matches(".m3u8").trim_end_matches(".M3U8").to_string();

    Ok(Playlist {
        name,
        filename,
        track_count: tracks.len(),
        tracks,
        full_path: path.to_string_lossy().into_owned(),
    })
}

/// Renames a playlist file (sanitizing `new_name`) and returns its new path.
pub fn rename_playlist(path: &Path, new_name: &str) -> Result<PathBuf> {
    let sanitized = sanitize_filename(new_name, DEFAULT_MAX_FILENAME_LEN);
    let new_path = path.with_file_name(format!("{sanitized}.m3u8"));
    std::fs::rename(path, &new_path)?;
    Ok(new_path)
}

pub fn delete_playlist(path: &Path) -> Result<()> {
    std::fs::remove_file(path)?;
    Ok(())
}

/// One synchronized lyrics line for [`write_lrc`].
#[derive(Debug, Clone, PartialEq)]
pub struct LrcLine {
    pub timestamp_seconds: f64,
    pub text: String,
}

/// Writes `lines` as a companion `.lrc` next to `audio_path` (same stem, `.lrc` extension):
/// chronological order, strict `[mm:ss.xx]` timestamps, an optional `[offset:+/-ms]` header
/// line, CRLF, UTF-8 without a BOM (R15.4). `audio_path` itself is never touched, so its
/// timestamps are preserved by construction.
pub fn write_lrc(audio_path: &Path, lines: &[LrcLine], offset_ms: Option<i64>) -> Result<PathBuf> {
    let lrc_path = audio_path.with_extension("lrc");

    let mut sorted = lines.to_vec();
    sorted.sort_by(|a, b| a.timestamp_seconds.total_cmp(&b.timestamp_seconds));

    let mut content = String::new();
    if let Some(offset) = offset_ms {
        let sign = if offset >= 0 { "+" } else { "-" };
        content.push_str(&format!("[offset:{sign}{}]\r\n", offset.abs()));
    }
    for line in &sorted {
        content.push_str(&format_lrc_timestamp(line.timestamp_seconds));
        content.push_str(&sanitize_lrc_line_text(&line.text));
        content.push_str("\r\n");
    }

    // `String`'s bytes are UTF-8 with no BOM by construction; written as-is.
    std::fs::write(&lrc_path, content.as_bytes())?;
    Ok(lrc_path)
}

/// Sanitizes one lyric line's text before it's written into the strict `[mm:ss.xx]text\r\n`
/// `.lrc` format (R19.3): a line's text might come from a pasted lyrics source, an LRCLIB/
/// Lyrics.ovh response, or the built-in editor's free-text field (task 18), any of which
/// could contain an embedded `\r`/`\n` — collapsed to a single space here so one logical line
/// can never become two or more physical lines (which would desync every subsequent
/// timestamp from the text that follows it) or inject a bare line with no timestamp prefix.
fn sanitize_lrc_line_text(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\r', '\n'], " ")
}

fn format_lrc_timestamp(seconds: f64) -> String {
    let total_centiseconds = (seconds.max(0.0) * 100.0).round() as i64;
    let minutes = total_centiseconds / 100 / 60;
    let secs = (total_centiseconds / 100) % 60;
    let centis = total_centiseconds % 100;
    format!("[{minutes:02}:{secs:02}.{centis:02}]")
}

/// Parses `.lrc` content (as written by [`write_lrc`], or any standard synced-lyrics file) back
/// into [`LrcLine`]s, sorted chronologically — the read half of the `write_lrc`/`parse_lrc` pair,
/// used by the fullscreen player's karaoke auto-scroll (task 15.2, R7.3) and the SensMe channels
/// view's live lyrics (task 15.3, R6.5). A single line may carry more than one `[mm:ss.xx]` tag
/// (a common `.lrc` convention for a line repeated at several timestamps); each tag produces its
/// own [`LrcLine`]. Lines with no recognizable tag (including a leading `[offset:...]` header)
/// are skipped.
pub fn parse_lrc(content: &str) -> Vec<LrcLine> {
    let mut lines = Vec::new();
    for raw_line in content.lines() {
        let mut rest = raw_line.trim_end_matches(['\r', '\n']);
        let mut timestamps = Vec::new();
        while let Some(stripped) = rest.strip_prefix('[') {
            let Some(end) = stripped.find(']') else { break };
            let tag = &stripped[..end];
            if let Some(seconds) = parse_lrc_timestamp(tag) {
                timestamps.push(seconds);
            } else {
                break; // not a timestamp tag (e.g. `[offset:...]`, `[ar:...]`) — stop consuming.
            }
            rest = &stripped[end + 1..];
        }
        let text = rest.to_string();
        for seconds in timestamps {
            lines.push(LrcLine { timestamp_seconds: seconds, text: text.clone() });
        }
    }
    lines.sort_by(|a, b| a.timestamp_seconds.total_cmp(&b.timestamp_seconds));
    lines
}

/// Parses one `mm:ss.xx` or `mm:ss` tag body (without the brackets) into seconds.
fn parse_lrc_timestamp(tag: &str) -> Option<f64> {
    let (minutes_str, rest) = tag.split_once(':')?;
    let minutes: f64 = minutes_str.trim().parse().ok()?;
    let seconds: f64 = rest.trim().parse().ok()?;
    Some(minutes * 60.0 + seconds)
}

/// Finds the index of the line that should be highlighted at `position_seconds`, given a
/// `lead_seconds` lookahead (R7.3's "~0.3s vocal lead-time" — the active line switches slightly
/// *before* its timestamp, so the highlight feels anticipatory rather than late). `lines` must
/// already be chronologically sorted (as [`parse_lrc`] returns them). Returns `None` before the
/// first line's (lead-adjusted) timestamp.
pub fn active_lyric_line_index(lines: &[LrcLine], position_seconds: f64, lead_seconds: f64) -> Option<usize> {
    let adjusted = position_seconds + lead_seconds;
    lines.iter().rposition(|l| l.timestamp_seconds <= adjusted)
}

/// Synthesizes evenly-spaced timestamps for plain (unsynced) lyrics text split into lines, so a
/// companion `.lrc` can still be written when only plain lyrics are available (R15.4). Lines are
/// spaced evenly across `duration_seconds` starting at `start_seconds`.
pub fn synthesize_lrc_lines(plain_text: &str, start_seconds: f64, duration_seconds: f64) -> Vec<LrcLine> {
    let lines: Vec<&str> = plain_text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return Vec::new();
    }
    let span = (duration_seconds - start_seconds).max(0.0);
    let step = if lines.len() > 1 { span / lines.len() as f64 } else { 0.0 };
    lines
        .into_iter()
        .enumerate()
        .map(|(i, text)| LrcLine {
            timestamp_seconds: start_seconds + step * i as f64,
            text: text.trim().to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use walkman_platform::fake::FakeVolumeProvider;
    use walkman_platform::{FreeSpace, VolumeInfo};

    #[test]
    fn format_bytes_uses_whole_numbers_under_1kb_and_one_decimal_above() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(5 * 1024 * 1024 * 1024), "5.0 GB");
    }

    #[test]
    fn format_duration_switches_to_hms_past_one_hour() {
        assert_eq!(format_duration(65.0), "1:05");
        assert_eq!(format_duration(3661.0), "1:01:01");
        assert_eq!(format_duration(-5.0), "0:00");
    }

    #[test]
    fn target_track_fs_path_joins_sanitized_components() {
        let path = target_track_fs_path(Path::new(r"E:\MUSIC"), "AC/DC", "Album", "3", "Song", "flac");
        assert_eq!(path, PathBuf::from(r"E:\MUSIC\ACDC\Album\03 - Song.flac"));
    }

    #[test]
    fn write_then_read_playlist_round_trips_with_crlf() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("My Mix.m3u8");

        write_playlist(
            &path,
            &[
                ("Artist/Album/01 - Song.flac".to_string(), "Artist - Song".to_string(), 125.0),
                ("Artist/Album/02 - Next.flac".to_string(), "Artist - Next".to_string(), 90.4),
            ],
        )
        .unwrap();

        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.starts_with("#EXTM3U\r\n"));
        assert!(raw.contains("#EXTINF:125,Artist - Song\r\n"));
        assert!(!raw.contains("#EXTINF:125,Artist - Song\n\n")); // sanity: no bare \n sneaking in
        for line in raw.split("\r\n") {
            assert!(!line.ends_with('\r'), "no double line endings");
        }

        let playlist = read_playlist(&path).unwrap();
        assert_eq!(playlist.name, "My Mix");
        assert_eq!(playlist.track_count, 2);
        assert_eq!(playlist.tracks[0], "Artist/Album/01 - Song.flac");
        assert_eq!(playlist.tracks[1], "Artist/Album/02 - Next.flac");
    }

    #[test]
    fn rename_and_delete_playlist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Old Name.m3u8");
        write_playlist(&path, &[]).unwrap();

        let renamed = rename_playlist(&path, "New: Name?").unwrap();
        assert!(renamed.file_name().unwrap().to_string_lossy().starts_with("New Name"));
        assert!(!path.exists());
        assert!(renamed.exists());

        delete_playlist(&renamed).unwrap();
        assert!(!renamed.exists());
    }

    #[test]
    fn write_lrc_produces_strict_timestamps_crlf_and_optional_offset() {
        let dir = tempfile::tempdir().unwrap();
        let audio_path = dir.path().join("song.flac");
        std::fs::write(&audio_path, b"fake audio").unwrap();

        let lines = vec![
            LrcLine { timestamp_seconds: 65.5, text: "second line".to_string() },
            LrcLine { timestamp_seconds: 1.25, text: "first line".to_string() },
        ];
        let lrc_path = write_lrc(&audio_path, &lines, Some(-150)).unwrap();
        assert_eq!(lrc_path, dir.path().join("song.lrc"));

        let raw = std::fs::read(&lrc_path).unwrap();
        assert!(!raw.starts_with(&[0xEF, 0xBB, 0xBF]), "must not write a UTF-8 BOM");
        let text = String::from_utf8(raw).unwrap();
        assert!(text.starts_with("[offset:-150]\r\n"));
        // Chronological order despite insertion order above.
        let first_line_pos = text.find("first line").unwrap();
        let second_line_pos = text.find("second line").unwrap();
        assert!(first_line_pos < second_line_pos);
        assert!(text.contains("[00:01.25]first line\r\n"));
        assert!(text.contains("[01:05.50]second line\r\n"));
    }

    #[test]
    fn write_lrc_never_touches_the_audio_files_own_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let audio_path = dir.path().join("song.mp3");
        std::fs::write(&audio_path, b"original audio bytes").unwrap();

        write_lrc(&audio_path, &[LrcLine { timestamp_seconds: 0.0, text: "x".into() }], None).unwrap();

        assert_eq!(std::fs::read(&audio_path).unwrap(), b"original audio bytes");
    }

    #[test]
    fn write_lrc_collapses_embedded_crlf_in_a_lines_text_to_a_single_line() {
        // R19.3: an embedded `\r`/`\n` in lyric text (e.g. from a pasted/fetched source)
        // must never split into extra physical lines, which would desync every later
        // timestamp from the line it belongs to.
        let dir = tempfile::tempdir().unwrap();
        let audio_path = dir.path().join("song.flac");
        std::fs::write(&audio_path, b"fake audio").unwrap();

        let lines = vec![
            LrcLine { timestamp_seconds: 0.0, text: "line one\r\ninjected".to_string() },
            LrcLine { timestamp_seconds: 1.0, text: "line two".to_string() },
        ];
        write_lrc(&audio_path, &lines, None).unwrap();

        let text = std::fs::read_to_string(audio_path.with_extension("lrc")).unwrap();
        // Exactly 2 physical lines (each ending in CRLF), not 3.
        assert_eq!(text.matches("\r\n").count(), 2);
        assert!(text.contains("[00:00.00]line one injected\r\n"));
        assert!(text.contains("[00:01.00]line two\r\n"));
    }

    #[test]
    fn synthesize_lrc_lines_spaces_lines_evenly_across_the_duration() {
        let lines = synthesize_lrc_lines("line one\nline two\nline three", 0.0, 90.0);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].timestamp_seconds, 0.0);
        assert_eq!(lines[1].timestamp_seconds, 30.0);
        assert_eq!(lines[2].timestamp_seconds, 60.0);
    }

    #[test]
    fn detect_drives_skips_volumes_with_no_sony_signature() {
        let dir = tempfile::tempdir().unwrap(); // no MUSIC/, no signature files
        let provider = FakeVolumeProvider::new();
        provider.set_volumes(vec![VolumeInfo {
            path: dir.path().to_path_buf(),
            label: Some("UNRELATED".into()),
            filesystem: "NTFS".into(),
            is_removable: true,
            is_read_only: false,
        }]);
        provider.set_free_space(dir.path(), FreeSpace { total_bytes: 1000, free_bytes: 500 });

        assert!(detect_drives(&provider).is_empty());
    }

    #[test]
    fn detect_drives_builds_a_walkman_drive_for_an_authentic_internal_volume() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("MUSIC")).unwrap();
        let provider = FakeVolumeProvider::new();
        provider.set_volumes(vec![VolumeInfo {
            path: dir.path().to_path_buf(),
            label: Some("WALKMAN".into()),
            filesystem: "exFAT".into(),
            is_removable: true,
            is_read_only: false,
        }]);
        provider.set_free_space(dir.path(), FreeSpace { total_bytes: 16_000_000_000, free_bytes: 4_000_000_000 });

        let drives = detect_drives(&provider);
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].drive_type, walkman_domain::models::DriveType::Internal);
        assert_eq!(drives[0].total_bytes, 16_000_000_000);
        assert!(!drives[0].has_cfw);
        assert!(drives[0].music_dir.ends_with("MUSIC"));
    }

    #[test]
    fn detect_devices_groups_internal_and_microsd_for_one_recognized_model() {
        let internal = tempfile::tempdir().unwrap();
        std::fs::write(
            internal.path().join("default-capability.xml"),
            b"<capability model=\"NW-A105\"/>",
        )
        .unwrap();
        let microsd = tempfile::tempdir().unwrap();
        std::fs::write(microsd.path().join("Sony_Card.IND"), b"").unwrap();

        let provider = FakeVolumeProvider::new();
        provider.set_volumes(vec![
            VolumeInfo {
                path: internal.path().to_path_buf(),
                label: Some("WALKMAN".into()),
                filesystem: "exFAT".into(),
                is_removable: true,
                is_read_only: false,
            },
            VolumeInfo {
                path: microsd.path().to_path_buf(),
                label: Some("SDCARD".into()),
                filesystem: "exFAT".into(),
                is_removable: true,
                is_read_only: false,
            },
        ]);
        provider.set_free_space(internal.path(), FreeSpace { total_bytes: 16_000_000_000, free_bytes: 1 });
        provider.set_free_space(microsd.path(), FreeSpace { total_bytes: 64_000_000_000, free_bytes: 1 });

        let devices = detect_devices(&provider);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].drives.len(), 2);
        assert_eq!(devices[0].specific_model(), "NW-A105");
        // NW-A100 series is an Android Walkman (per the catalog's `is_non_android: false`).
        assert!(devices[0].device_info.is_android());
        assert_eq!(devices[0].device_info.os_family(), "Android Walkman OS");
    }

    #[test]
    fn detect_devices_skips_an_unresolvable_volume_with_no_model_token() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("MUSIC")).unwrap(); // authentic, but no capability XML

        let provider = FakeVolumeProvider::new();
        provider.set_volumes(vec![VolumeInfo {
            path: dir.path().to_path_buf(),
            label: Some("WALKMAN".into()),
            filesystem: "exFAT".into(),
            is_removable: true,
            is_read_only: false,
        }]);
        provider.set_free_space(dir.path(), FreeSpace { total_bytes: 16_000_000_000, free_bytes: 1 });

        // No model token and no PID to resolve against: `resolve_model` can't identify it, so
        // it's correctly omitted rather than reported as a phantom device.
        assert!(detect_devices(&provider).is_empty());
    }

    #[test]
    fn custom_storage_folder_derives_music_and_learning_dirs() {
        let folder = CustomStorageFolder::new(PathBuf::from(r"D:\MyWalkman"));
        assert_eq!(folder.music_dir(), PathBuf::from(r"D:\MyWalkman\MUSIC"));
        assert_eq!(folder.learning_dir(), PathBuf::from(r"D:\MyWalkman\LEARNING"));
    }

    #[test]
    fn parse_lrc_round_trips_write_lrc_output() {
        let lines = vec![
            LrcLine { timestamp_seconds: 65.5, text: "second line".to_string() },
            LrcLine { timestamp_seconds: 1.25, text: "first line".to_string() },
        ];
        let written = {
            let mut sorted = lines.clone();
            sorted.sort_by(|a, b| a.timestamp_seconds.total_cmp(&b.timestamp_seconds));
            let mut content = String::new();
            for line in &sorted {
                content.push_str(&format_lrc_timestamp(line.timestamp_seconds));
                content.push_str(&line.text);
                content.push_str("\r\n");
            }
            content
        };

        let parsed = parse_lrc(&written);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].text, "first line");
        assert!((parsed[0].timestamp_seconds - 1.25).abs() < 0.01);
        assert_eq!(parsed[1].text, "second line");
        assert!((parsed[1].timestamp_seconds - 65.5).abs() < 0.01);
    }

    #[test]
    fn parse_lrc_skips_a_leading_offset_header() {
        let content = "[offset:-150]\r\n[00:01.00]hello\r\n";
        let parsed = parse_lrc(content);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].text, "hello");
    }

    #[test]
    fn parse_lrc_expands_a_line_with_multiple_timestamp_tags() {
        let content = "[00:01.00][00:05.00]repeated line\r\n";
        let parsed = parse_lrc(content);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].text, "repeated line");
        assert_eq!(parsed[1].text, "repeated line");
        assert!((parsed[1].timestamp_seconds - 5.0).abs() < 0.01);
    }

    #[test]
    fn active_lyric_line_index_applies_the_lead_and_picks_the_most_recent_line() {
        let lines = vec![
            LrcLine { timestamp_seconds: 0.0, text: "a".into() },
            LrcLine { timestamp_seconds: 10.0, text: "b".into() },
            LrcLine { timestamp_seconds: 20.0, text: "c".into() },
        ];
        // At 9.8s with a 0.3s lead, the effective position is 10.1s, which is past line b's
        // 10.0s timestamp but not yet at c's 20.0s.
        assert_eq!(active_lyric_line_index(&lines, 9.8, 0.3), Some(1));
        assert_eq!(active_lyric_line_index(&lines, -1.0, 0.3), None);
        assert_eq!(active_lyric_line_index(&lines, 25.0, 0.3), Some(2));
    }

    #[test]
    fn active_lyric_line_index_handles_empty_lines() {
        assert_eq!(active_lyric_line_index(&[], 10.0, 0.3), None);
    }
}
