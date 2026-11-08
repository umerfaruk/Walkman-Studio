//! Real service/platform instances the shell is wired against (spec task 18).
//!
//! Everything in [`AppServices`] is the concrete, Windows-backed counterpart of a trait the
//! `platform`/`services` crates defined — this module's only job is construction and
//! lifetime (most of it lives behind one `Arc` per service so background threads/receivers
//! can outlive a single frame).

use std::path::PathBuf;
use std::sync::Arc;

use walkman_platform::windows::{WindowsFileDialogs, WindowsVolumeProvider};
use walkman_platform::DeviceEvents;
use walkman_services::library_scanner::LibraryScanner;
use walkman_services::local_library::LocalLibrary;
use walkman_services::player::Player;
use walkman_services::transfer_queue::TransferQueue;

/// `%APPDATA%\Walkman Studio\library.sqlite3` — the persistent cache database shared by the
/// Local Library and every connected Walkman mount (`LibraryScanner`'s mount-keyed tables,
/// task 7.1), falling back to the system temp directory if `APPDATA` isn't set (never
/// expected on a real Windows session, but keeps this function total like
/// `backup_restore::default_backup_dir` does for the same reason).
pub fn library_db_path() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let dir = base.join("Walkman Studio");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("library.sqlite3")
}

/// Every real service/platform instance the app owns for its lifetime.
pub struct AppServices {
    pub scanner: Arc<LibraryScanner>,
    pub local_library: Arc<LocalLibrary>,
    // The `walkman-studio-iced` binary doesn't read `player`/`transfer_queue`/
    // `device_events`/`file_dialogs` yet — the egui binary's `dispatch.rs` does. They're
    // wired into the iced shell as the views/sheets that need them are ported (player:
    // the now-playing milestone; transfer_queue/file_dialogs: the views/sheets
    // milestones; device_events: once bridged into a real-time `Subscription`, see
    // `lifecycle_iced.rs`'s doc comment on its current poll-only simplification).
    #[allow(dead_code)]
    pub player: Arc<Player>,
    #[allow(dead_code)]
    pub transfer_queue: Arc<TransferQueue>,
    pub volumes: WindowsVolumeProvider,
    /// Kept alive for the app's lifetime (never otherwise read) — its background poll
    /// thread stops once every `Arc` (including this one) is dropped, see
    /// `WindowsDeviceEvents::start`'s own doc comment.
    #[allow(dead_code)]
    pub device_events_source: Arc<walkman_platform::windows::WindowsDeviceEvents>,
    #[allow(dead_code)]
    pub device_events: std::sync::mpsc::Receiver<walkman_platform::DeviceEvent>,
    #[allow(dead_code)]
    pub file_dialogs: WindowsFileDialogs,
}

impl AppServices {
    /// Opens the shared cache database and starts every platform service. The only failure
    /// mode is the cache database (a corrupt/unwritable `library.sqlite3`); every other
    /// service here is infallible to construct (the audio device itself is opened lazily by
    /// `Player::play_index`, not `Player::new` — see that type's own doc).
    pub fn new() -> anyhow::Result<Self> {
        let scanner = Arc::new(LibraryScanner::open(&library_db_path())?);
        let local_library = Arc::new(LocalLibrary::new(Arc::clone(&scanner)));
        let player = Arc::new(Player::new(None).map_err(anyhow::Error::from)?);
        let device_events_source = walkman_platform::windows::WindowsDeviceEvents::start();
        let device_events = device_events_source.subscribe();

        Ok(Self {
            scanner,
            local_library,
            player,
            transfer_queue: Arc::new(TransferQueue::new()),
            volumes: WindowsVolumeProvider::new(),
            device_events_source,
            device_events,
            file_dialogs: WindowsFileDialogs::new(),
        })
    }
}
