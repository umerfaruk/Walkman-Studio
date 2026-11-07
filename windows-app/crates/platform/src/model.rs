//! Plain data types exchanged across the platform boundary.

use std::path::PathBuf;
use thiserror::Error;

/// A mounted volume discovered by [`crate::VolumeProvider`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeInfo {
    /// Mount path / drive root (e.g. `E:\` on Windows, `/Volumes/WALKMAN` on macOS).
    pub path: PathBuf,
    /// Volume label, if any.
    pub label: Option<String>,
    /// Filesystem name (e.g. "exFAT", "FAT32", "NTFS").
    pub filesystem: String,
    pub is_removable: bool,
    pub is_read_only: bool,
}

/// Capacity figures for a volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreeSpace {
    pub total_bytes: i64,
    pub free_bytes: i64,
}

impl FreeSpace {
    pub fn used_bytes(&self) -> i64 {
        (self.total_bytes - self.free_bytes).max(0)
    }
    pub fn percent_used(&self) -> f64 {
        if self.total_bytes > 0 {
            (self.used_bytes() as f64 / self.total_bytes as f64) * 100.0
        } else {
            0.0
        }
    }
}

/// A device attach/detach notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceEvent {
    Attached(PathBuf),
    Detached(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
}

/// A connected USB device (subset of descriptors relevant to Walkman matching).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbDevice {
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial: Option<String>,
    pub product_name: Option<String>,
}

/// Now-playing metadata pushed to the OS media controls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NowPlayingInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Optional path to artwork on disk.
    pub artwork_path: Option<PathBuf>,
    pub duration_seconds: u64,
    pub position_seconds: u64,
}

#[derive(Debug, Error)]
pub enum EjectError {
    #[error("volume is busy or in use")]
    Busy,
    #[error("volume not found: {0}")]
    NotFound(String),
    #[error("eject timed out")]
    Timeout,
    #[error("platform error: {0}")]
    Platform(String),
}
