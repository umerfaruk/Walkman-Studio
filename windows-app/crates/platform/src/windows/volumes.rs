//! [`VolumeProvider`] for Windows (spec task 6.2).
//!
//! macOS enumerates `/Volumes` + `statvfs`; Windows has no mount-point directory, so we walk the
//! `GetLogicalDrives` bitmask of drive letters A–Z and query each with `GetDriveTypeW` +
//! `GetVolumeInformationW`, mirroring the same `VolumeInfo` shape the rest of the app expects.

use std::path::Path;

use windows::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
};
use windows::Win32::System::WindowsProgramming::{DRIVE_CDROM, DRIVE_FIXED, DRIVE_REMOVABLE};
use windows::core::PCWSTR;

use crate::windows::util::{drive_root, from_wide, to_wide};
use crate::{FreeSpace, VolumeInfo, VolumeProvider};

/// Real `VolumeProvider` backed by Win32 drive-letter enumeration.
#[derive(Debug, Default)]
pub struct WindowsVolumeProvider;

impl WindowsVolumeProvider {
    pub fn new() -> Self {
        Self
    }

    /// Queries one drive letter (`'A'..='Z'`). Returns `None` for drive types that cannot carry a
    /// Walkman volume (network shares, optical, RAM disks, unmapped letters) or that fail to
    /// report volume information (e.g. an empty optical/card-reader slot).
    fn inspect(letter: char) -> Option<VolumeInfo> {
        let root = drive_root(letter);
        let root_w = to_wide(&root);
        let root_pcwstr = PCWSTR(root_w.as_ptr());

        let drive_type = unsafe { GetDriveTypeW(root_pcwstr) };
        if drive_type != DRIVE_REMOVABLE && drive_type != DRIVE_FIXED && drive_type != DRIVE_CDROM
        {
            return None;
        }

        let mut volume_name = [0u16; 256];
        let mut fs_name = [0u16; 32];
        let mut fs_flags: u32 = 0;

        let ok = unsafe {
            GetVolumeInformationW(
                root_pcwstr,
                Some(&mut volume_name),
                None,
                None,
                Some(&mut fs_flags),
                Some(&mut fs_name),
            )
        };
        if ok.is_err() {
            // No media present (e.g. empty card-reader slot) — not a usable volume.
            return None;
        }

        let label = from_wide(&volume_name);
        let filesystem = from_wide(&fs_name);
        const FILE_READ_ONLY_VOLUME: u32 = 0x0008_0000;
        let is_read_only = fs_flags & FILE_READ_ONLY_VOLUME != 0;

        Some(VolumeInfo {
            path: std::path::PathBuf::from(&root),
            label: if label.is_empty() { None } else { Some(label) },
            filesystem,
            is_removable: drive_type == DRIVE_REMOVABLE,
            is_read_only,
        })
    }
}

impl VolumeProvider for WindowsVolumeProvider {
    fn list_volumes(&self) -> Vec<VolumeInfo> {
        let mask = unsafe { GetLogicalDrives() };
        (0..26u32)
            .filter(|bit| mask & (1 << bit) != 0)
            .filter_map(|bit| {
                let letter = (b'A' + bit as u8) as char;
                Self::inspect(letter)
            })
            .collect()
    }

    fn free_space(&self, path: &Path) -> std::io::Result<FreeSpace> {
        let mut root = path.to_string_lossy().into_owned();
        if !root.ends_with('\\') {
            root.push('\\');
        }
        let root_w = to_wide(&root);
        let root_pcwstr = PCWSTR(root_w.as_ptr());

        let mut free_bytes: u64 = 0;
        let mut total_bytes: u64 = 0;

        let result = unsafe {
            GetDiskFreeSpaceExW(
                root_pcwstr,
                Some(&mut free_bytes),
                Some(&mut total_bytes),
                None,
            )
        };
        result.map_err(std::io::Error::other)?;

        Ok(FreeSpace {
            total_bytes: total_bytes as i64,
            free_bytes: free_bytes as i64,
        })
    }
}
