//! Device attach/detach + local-library lifecycle for the iced shell. Milestone 4 of the
//! egui-to-iced migration plan: ported from `lifecycle.rs`'s reducer logic — the *async
//! mechanism* changes from "spawn a thread, stash its `Receiver` in an `Option`, poll it
//! every frame" to iced's `Task` (one-shot work) + `Subscription` (periodic/streaming
//! work), which removes the manual `pending_*: Option<Receiver<_>>` + `poll_*()` state
//! `lifecycle.rs` needs under egui's "redraw every 500ms regardless" model.
//!
//! Documented simplification (same spirit as this repo's other honestly-scoped gaps,
//! see `windows-rust-port/tasks.md`): the real-time `DeviceEvents` channel
//! (`AppServices::device_events`) isn't bridged into a `Subscription` yet — only the 3s
//! safety-net poll below runs. `lifecycle.rs` treats that poll as a fallback behind the
//! real-time channel, not the primary path; here it's temporarily the *only* path, so a
//! device change can take up to 3s to show up instead of being near-instant. Bridging the
//! blocking `mpsc::Receiver` into a proper iced `Subscription` stream is a follow-up, not
//! a structural blocker for the rest of this milestone.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use walkman_domain::models::{LocalMusicFolder, Track, WalkmanDevice};
use walkman_services::library_scanner::LibraryScanner;
use walkman_services::walkman_engine::detect_devices;

use crate::services::AppServices;

/// Safety-net device poll interval — matches `lifecycle.rs::DEVICE_POLL_INTERVAL`.
pub const DEVICE_POLL_INTERVAL: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
pub enum Message {
    LocalLibraryLoaded {
        folders: Vec<LocalMusicFolder>,
        tracks: Vec<Track>,
    },
    DevicePollTick,
    WalkmanMountScanned(Vec<Track>),
}

/// One-shot local-library load, kicked off from `App::new` — ports
/// `lifecycle.rs::load_local_library_once` + its `poll_local_rescan` follow-up into a
/// single `Task` instead of a load flag + a polled `Option<Receiver<_>>`.
pub fn startup_task(services: &AppServices) -> iced::Task<Message> {
    let local_library = Arc::clone(&services.local_library);
    iced::Task::perform(
        async move {
            let folders = local_library.list_folders().unwrap_or_default();
            let rx = local_library.rescan_all_async();
            // `rescan_all_async` already spawns its own OS thread; block *that* thread's
            // completion off the async executor's worker threads via `spawn_blocking`
            // rather than busy-polling `try_recv()` the way `lifecycle.rs` does every frame.
            let _ = tokio::task::spawn_blocking(move || rx.recv()).await;
            let tracks = local_library.tracks().unwrap_or_default();
            (folders, tracks)
        },
        |(folders, tracks)| Message::LocalLibraryLoaded { folders, tracks },
    )
}

/// The periodic device-poll subscription (`lifecycle.rs`'s safety-net timer, now the only
/// detection path per the simplification noted above).
pub fn subscription() -> iced::Subscription<Message> {
    iced::time::every(DEVICE_POLL_INTERVAL).map(|_| Message::DevicePollTick)
}

/// Runs `detect_devices` synchronously — a fast local WMI/registry query, same as
/// `lifecycle.rs` calls it inline on every tick, not worth a `Task` of its own — and
/// returns the devices plus, if a not-yet-scanned mount appeared, the scan `Task` for it.
/// `scanned_mounts` is the same "don't rescan a mount that hasn't changed" bookkeeping
/// `lifecycle.rs` keeps outside `AppState`.
pub fn refresh_devices(
    services: &AppServices,
    scanned_mounts: &mut Vec<String>,
) -> (Vec<WalkmanDevice>, Option<iced::Task<Message>>) {
    let devices = detect_devices(&services.volumes);

    let task = match active_mount_point(&devices) {
        Some(mount) if !scanned_mounts.contains(&mount) => {
            *scanned_mounts = vec![mount.clone()];
            let scanner = Arc::clone(&services.scanner);
            Some(iced::Task::perform(
                scan_mount(scanner, mount),
                Message::WalkmanMountScanned,
            ))
        }
        None => {
            scanned_mounts.clear();
            None
        }
        _ => None, // already-scanned mount unchanged; nothing to do.
    };

    (devices, task)
}

/// The primary drive to scan (R1.4/3.5: a single active device/drive at a time) — ported
/// verbatim from `lifecycle.rs::active_mount_point` (pure logic, no egui dependency).
fn active_mount_point(devices: &[WalkmanDevice]) -> Option<String> {
    let device = devices.first()?;
    device.internal_drive().or_else(|| device.drives.first()).map(|d| d.mount_point.clone())
}

/// Scans `mount`'s music directory off the async executor's worker threads and returns the
/// resulting track list — ports `lifecycle.rs::scan_mount_async`'s body into a plain `async
/// fn` for `Task::perform`, dropping its manual channel plumbing.
async fn scan_mount(scanner: Arc<LibraryScanner>, mount: String) -> Vec<Track> {
    tokio::task::spawn_blocking(move || {
        let music_dir = Path::new(&mount).join("MUSIC");
        let music_dir = if music_dir.is_dir() { music_dir } else { Path::new(&mount).join("Music") };
        if music_dir.is_dir() {
            if let Err(e) = scanner.scan_tree(&music_dir, &mount) {
                log::warn!("scan of {mount} failed: {e}");
            }
        }
        scanner.tracks_for_mount(&mount).unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}
