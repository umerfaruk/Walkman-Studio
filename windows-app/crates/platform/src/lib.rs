//! Platform abstraction layer for Walkman Studio.
//!
//! The rest of the app depends only on the traits defined here; concrete OS access
//! lives in the `windows` submodule (spec task 6). This mirrors the macOS→Windows
//! mapping table in the design document.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

pub mod model;
pub use model::*;

pub mod fake;

#[cfg(windows)]
pub mod windows;
#[cfg(windows)]
pub use windows::{fast_copy, set_file_times};

/// Enumerates mounted volumes and reports free space.
/// macOS source: `/Volumes` scan + `statvfs`. Windows: `GetLogicalDrives` +
/// `GetDiskFreeSpaceExW` (task 6.2).
pub trait VolumeProvider: Send + Sync {
    fn list_volumes(&self) -> Vec<VolumeInfo>;
    fn free_space(&self, path: &Path) -> std::io::Result<FreeSpace>;
}

/// Publishes device attach/detach events.
/// macOS source: `NSWorkspace` notifications. Windows: `WM_DEVICECHANGE` /
/// `RegisterDeviceNotification` (task 6.3).
pub trait DeviceEvents: Send + Sync {
    fn subscribe(&self) -> Receiver<DeviceEvent>;
}

/// Enumerates connected USB devices (for Walkman VID/PID matching).
/// macOS source: IOKit. Windows: SetupAPI / WMI (task 6.4).
pub trait UsbEnumerator: Send + Sync {
    fn scan(&self) -> Vec<UsbDevice>;
}

/// Safely ejects/unmounts a removable volume.
/// macOS source: `diskutil` / `NSWorkspace.unmountAndEject`. Windows:
/// `IOCTL_STORAGE_EJECT_MEDIA` / lock+dismount, `mountvol /p` fallback (task 6.5).
pub trait Ejector: Send + Sync {
    fn eject(&self, mount: &Path) -> Result<(), EjectError>;
}

/// Native file dialogs and reveal-in-file-manager.
/// macOS source: `NSOpenPanel` / `NSSavePanel`. Windows: `IFileOpenDialog` /
/// `IFileSaveDialog` (task 6.6).
pub trait FileDialogs: Send + Sync {
    fn pick_folder(&self, title: &str) -> Option<PathBuf>;
    fn save_file(&self, suggested_name: &str, filters: &[(&str, &str)]) -> Option<PathBuf>;
    fn reveal_in_file_manager(&self, path: &Path);
}

/// OS media transport controls / now-playing surface.
/// macOS source: `MPNowPlayingInfoCenter`. Windows: WinRT
/// `SystemMediaTransportControls` (task 6.7).
pub trait MediaControls: Send + Sync {
    fn set_now_playing(&self, info: &NowPlayingInfo);
    fn set_playback_state(&self, playing: bool);
}
