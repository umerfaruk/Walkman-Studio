//! Device attach/detach + library lifecycle (spec task 18): subscribes to `DeviceEvents` +
//! a periodic safety-net refresh to auto-detect Walkman attach/detach, triggers local-folder
//! and connected-device scans off the UI thread, and keeps the sidebar badges/Device
//! Overview/`AppState::walkman_tracks` in sync with whatever is actually plugged in.

use std::path::Path;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

use walkman_domain::models::{Playlist, Track, WalkmanDevice};
use walkman_platform::DeviceEvent;
use walkman_services::duplicate_finder::{self, DuplicateScope};
use walkman_services::library_scanner::LibraryScanner;
use walkman_services::local_library::PrecomputedCaches;
use walkman_services::walkman_engine::detect_devices;
use walkman_ui::views::recently_added::{ranked_tracks, TimeScope};
use walkman_ui::AppState;

use crate::services::AppServices;

/// How often the periodic safety-net re-scan for device attach/detach runs (R3.6), on top of
/// the real-time `DeviceEvents` channel — a bare safety net against a missed/coalesced event,
/// not the primary detection path.
const DEVICE_POLL_INTERVAL: Duration = Duration::from_secs(3);

/// The sidebar's "recently added" badge window. No scope is specified by R1.4's badge
/// wording, so this picks the same 7-day window the view's own placeholder tests used before
/// this task wired a real count in.
const RECENTLY_ADDED_BADGE_SCOPE: TimeScope = TimeScope::Past7Days;

/// Lifecycle bookkeeping that doesn't belong on [`AppState`] itself (it's this crate's own
/// scheduling state, not UI-presentable data).
pub struct Lifecycle {
    last_device_poll: Instant,
    /// The mount points currently scanned into `AppState::walkman_tracks`'s backing cache,
    /// so a device/drive that hasn't changed isn't re-scanned every tick.
    scanned_mounts: Vec<String>,
    local_library_loaded: bool,
    pending_local_rescan: Option<Receiver<walkman_services::error::Result<PrecomputedCaches>>>,
    pending_walkman_scan: Option<Receiver<Vec<Track>>>,
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self {
            // Elapsed-enough-to-poll-immediately on the very first tick.
            last_device_poll: Instant::now() - DEVICE_POLL_INTERVAL,
            scanned_mounts: Vec::new(),
            local_library_loaded: false,
            pending_local_rescan: None,
            pending_walkman_scan: None,
        }
    }
}

impl Lifecycle {
    /// Runs once per frame. Cheap when nothing changed (a channel `try_recv` and an `Instant`
    /// comparison); the actual scans run off-thread and are only picked up here once done.
    pub fn tick(&mut self, services: &mut AppServices, state: &mut AppState) {
        self.bind_player_once(services, state);
        self.load_local_library_once(services, state);
        self.poll_local_rescan(services, state);

        if self.device_poll_due(&services.device_events) {
            self.refresh_devices(services, state);
        }
        self.poll_walkman_scan(state);
    }

    fn bind_player_once(&self, services: &AppServices, state: &mut AppState) {
        if state.player.is_none() {
            state.player = Some(Arc::clone(&services.player));
        }
    }

    fn load_local_library_once(&mut self, services: &AppServices, state: &mut AppState) {
        if self.local_library_loaded {
            return;
        }
        self.local_library_loaded = true;
        if let Ok(folders) = services.local_library.list_folders() {
            state.local_folders = folders;
        }
        self.pending_local_rescan = Some(services.local_library.rescan_all_async());
    }

    fn poll_local_rescan(&mut self, services: &AppServices, state: &mut AppState) {
        let Some(rx) = &self.pending_local_rescan else { return };
        if let Ok(result) = rx.try_recv() {
            self.pending_local_rescan = None;
            match result {
                Ok(_caches) => {
                    // The precomputed albums/artists/recent/language groupings are recomputed
                    // on demand by each view's own pure functions (tasks 14.1-14.7) from
                    // `local_tracks`/`walkman_tracks` directly; what this tick needs from the
                    // rescan is the refreshed flat track list and folder metadata.
                    if let Ok(folders) = services.local_library.list_folders() {
                        state.local_folders = folders;
                    }
                    if let Ok(tracks) = services.local_library.tracks() {
                        state.local_tracks = tracks;
                    }
                }
                Err(e) => log::warn!("local library rescan failed: {e}"),
            }
        }
    }

    /// `true` when either a real `DeviceEvent` arrived or the periodic safety-net interval
    /// has elapsed (R3.6). Draining every queued event (not just the first) means a rapid
    /// attach+detach during one slow frame still ends in the correct final state.
    fn device_poll_due(&mut self, events: &Receiver<DeviceEvent>) -> bool {
        let mut any_event = false;
        while let Ok(event) = events.try_recv() {
            any_event = true;
            log::info!("device event: {event:?}");
        }
        if any_event {
            self.last_device_poll = Instant::now();
            return true;
        }
        if self.last_device_poll.elapsed() >= DEVICE_POLL_INTERVAL {
            self.last_device_poll = Instant::now();
            return true;
        }
        false
    }

    fn refresh_devices(&mut self, services: &AppServices, state: &mut AppState) {
        let devices = detect_devices(&services.volumes);
        state.devices = devices.clone();
        state.walkman_connected = !devices.is_empty();
        state.device_count = devices.len();
        state.drives = devices.iter().flat_map(|d| d.drives.iter()).map(drive_row).collect();

        let active_mount = active_mount_point(&devices);
        match active_mount {
            Some(mount) if !self.scanned_mounts.contains(&mount) => {
                self.scanned_mounts = vec![mount.clone()];
                self.pending_walkman_scan = Some(scan_mount_async(Arc::clone(&services.scanner), mount));
            }
            None => {
                self.scanned_mounts.clear();
                state.walkman_tracks.clear();
            }
            _ => {} // already scanned this mount; nothing to do until it changes.
        }
    }

    fn poll_walkman_scan(&mut self, state: &mut AppState) {
        let Some(rx) = &self.pending_walkman_scan else { return };
        if let Ok(tracks) = rx.try_recv() {
            self.pending_walkman_scan = None;
            state.walkman_tracks = tracks;
            if let Some(mount) = self.scanned_mounts.first() {
                state.playlists = scan_playlists(mount);
            }
        }
    }
}

/// Lists every `.m3u8` under `mount`'s `MUSIC`/`Music` directory (R11.1) — a plain directory
/// listing + parse via the existing `walkman_engine::read_playlist`, not a second playlist
/// format reader. Best-effort: an unreadable directory (e.g. the drive was unplugged mid-scan)
/// yields an empty list rather than an error, matching this tick's "never block/panic the UI
/// on a flaky removable drive" posture.
fn scan_playlists(mount: &str) -> Vec<Playlist> {
    let music_dir = ["MUSIC", "Music"].iter().map(|n| Path::new(mount).join(n)).find(|p| p.is_dir());
    let Some(music_dir) = music_dir else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(&music_dir) else { return Vec::new() };

    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("m3u8")))
        .filter_map(|p| walkman_services::walkman_engine::read_playlist(&p).ok())
        .collect()
}

/// The primary drive to scan for `AppState::walkman_tracks` (R1.4/3.5: a single active
/// device/drive at a time) — the first device's internal drive, falling back to its first
/// drive at all (e.g. a microSD-only state is unusual but not impossible). `None` with no
/// device connected.
fn active_mount_point(devices: &[WalkmanDevice]) -> Option<String> {
    let device = devices.first()?;
    device.internal_drive().or_else(|| device.drives.first()).map(|d| d.mount_point.clone())
}

fn drive_row(drive: &walkman_domain::models::WalkmanDrive) -> walkman_ui::DriveRow {
    walkman_ui::DriveRow {
        name: drive.name.clone(),
        is_current: drive.drive_type == walkman_domain::models::DriveType::Internal,
        is_sd: drive.drive_type == walkman_domain::models::DriveType::Microsd,
    }
}

/// Scans `mount`'s music directory on a background thread and delivers the resulting track
/// list over the returned channel — the same off-thread + channel pattern
/// `LocalLibrary::rescan_all_async` already established, applied here to a single external
/// mount (which has no library-level async wrapper of its own, since only the Local Library
/// needed the "rescan every registered folder" shape).
fn scan_mount_async(scanner: Arc<LibraryScanner>, mount: String) -> Receiver<Vec<Track>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let music_dir = Path::new(&mount).join("MUSIC");
        let music_dir = if music_dir.is_dir() { music_dir } else { Path::new(&mount).join("Music") };
        if music_dir.is_dir() {
            if let Err(e) = scanner.scan_tree(&music_dir, &mount) {
                log::warn!("scan of {mount} failed: {e}");
            }
        }
        let tracks = scanner.tracks_for_mount(&mount).unwrap_or_default();
        let _ = tx.send(tracks);
    });
    rx
}

/// Recomputes every sidebar count badge from the current `walkman_tracks`/`local_tracks`/
/// `transfer_queue_items` (R1.4's count badges). Cheap enough to call every frame (it's a
/// handful of linear scans over already-in-memory `Vec`s, not I/O); called from
/// [`crate::dispatch::drain_all`] after any command that could have changed the underlying
/// lists.
pub fn refresh_sidebar_counts(state: &mut AppState) {
    state.local_track_count = state.local_tracks.len();

    let stats = walkman_services::local_library::precompute(state.walkman_tracks.clone()).stats;
    state.track_count = stats.track_count;
    state.album_count = stats.album_count;
    state.artist_count = stats.artist_count;
    state.hi_res_count = stats.hi_res_count;
    state.language_study_count = stats.language_study_count;

    state.recently_added_count = ranked_tracks(&state.walkman_tracks, RECENTLY_ADDED_BADGE_SCOPE, chrono::Utc::now()).len();
    state.duplicate_count = duplicate_finder::find_duplicates(&state.walkman_tracks, DuplicateScope::ExactSameAlbum).len();
    state.queue_count = state.transfer_queue_items.len();
}

#[cfg(test)]
mod tests {
    use super::*;
    use walkman_domain::models::{DriveType, WalkmanDeviceInfo, WalkmanDrive};

    fn drive(mount: &str, drive_type: DriveType) -> WalkmanDrive {
        WalkmanDrive {
            name: "Drive".into(),
            mount_point: mount.into(),
            drive_type,
            total_bytes: 1,
            used_bytes: 0,
            free_bytes: 1,
            percent_used: 0.0,
            has_cfw: false,
            music_dir: format!("{mount}MUSIC"),
        }
    }

    fn device(drives: Vec<WalkmanDrive>) -> WalkmanDevice {
        WalkmanDevice::new(
            WalkmanDeviceInfo {
                model_raw: String::new(),
                marketing_name: String::new(),
                display_name: "Sony Walkman".into(),
                series_name: String::new(),
                specific_model: String::new(),
                vendor: String::new(),
                firmware_version: String::new(),
                is_non_android: true,
                has_cfw: false,
                display_resolution: String::new(),
                internal_capacity_gb: 16.0,
                supported_audio_formats: Vec::new(),
            },
            drives,
        )
    }

    #[test]
    fn active_mount_point_prefers_the_internal_drive() {
        let devices = vec![device(vec![drive("F:\\", DriveType::Microsd), drive("E:\\", DriveType::Internal)])];
        assert_eq!(active_mount_point(&devices), Some("E:\\".to_owned()));
    }

    #[test]
    fn active_mount_point_falls_back_to_the_first_drive_with_no_internal() {
        let devices = vec![device(vec![drive("F:\\", DriveType::Microsd)])];
        assert_eq!(active_mount_point(&devices), Some("F:\\".to_owned()));
    }

    #[test]
    fn active_mount_point_is_none_with_no_devices() {
        assert_eq!(active_mount_point(&[]), None);
    }

    #[test]
    fn drive_row_maps_internal_and_microsd_flags() {
        let internal = drive_row(&drive("E:\\", DriveType::Internal));
        assert!(internal.is_current);
        assert!(!internal.is_sd);

        let sd = drive_row(&drive("F:\\", DriveType::Microsd));
        assert!(!sd.is_current);
        assert!(sd.is_sd);
    }
}
