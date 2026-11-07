//! Preserves a file's creation/modified timestamps across an in-place rewrite (spec task 8.4,
//! requirement 8.5): the audio tagger reads these before writing new tag bytes and restores them
//! afterward so "Recently Added" smart views don't re-order a file just because its tags changed.
//!
//! This is a plain utility function, not one of the [`crate`] traits: it has no behavior to fake
//! in tests (real temporary files exercise it directly) and nothing else needs to swap its
//! implementation.

use std::path::Path;
use std::time::SystemTime;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, SetFileTime, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE,
    FILE_WRITE_ATTRIBUTES, OPEN_EXISTING,
};

use crate::windows::util::to_wide;

/// 100ns ticks per second, and the offset between the Windows (1601) and Unix (1970) epochs in
/// seconds — the standard `FILETIME` <-> `SystemTime` conversion constants.
const WINDOWS_TICKS_PER_SEC: u64 = 10_000_000;
const SEC_BETWEEN_1601_AND_1970: u64 = 11_644_473_600;

fn system_time_to_filetime(t: SystemTime) -> Option<FILETIME> {
    let dur = t.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    let ticks = (dur.as_secs() + SEC_BETWEEN_1601_AND_1970) * WINDOWS_TICKS_PER_SEC
        + u64::from(dur.subsec_nanos()) / 100;
    Some(FILETIME {
        dwLowDateTime: (ticks & 0xFFFF_FFFF) as u32,
        dwHighDateTime: (ticks >> 32) as u32,
    })
}

/// Sets `path`'s creation and/or last-write time. Either may be omitted to leave that timestamp
/// untouched.
pub fn set_file_times(
    path: &Path,
    created: Option<SystemTime>,
    modified: Option<SystemTime>,
) -> std::io::Result<()> {
    let wide = to_wide(&path.to_string_lossy());
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            FILE_WRITE_ATTRIBUTES.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            HANDLE::default(),
        )
    }
    .map_err(std::io::Error::other)?;

    let created_ft = created.and_then(system_time_to_filetime);
    let modified_ft = modified.and_then(system_time_to_filetime);
    let result = unsafe {
        SetFileTime(
            handle,
            created_ft.as_ref().map(|f| f as *const FILETIME),
            None,
            modified_ft.as_ref().map(|f| f as *const FILETIME),
        )
    };

    unsafe {
        let _ = CloseHandle(handle);
    }
    result.map_err(std::io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn restores_modified_time_after_rewriting_a_file() {
        let dir = std::env::temp_dir().join(format!("walkman-fs-time-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sample.txt");
        std::fs::write(&path, b"original").unwrap();

        let original_modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        // Make the timestamp distinguishable from "now" so the restore is actually verified.
        let target = original_modified - Duration::from_secs(10_000);

        std::fs::write(&path, b"rewritten, much longer content than before").unwrap();
        set_file_times(&path, None, Some(target)).unwrap();

        let restored = std::fs::metadata(&path).unwrap().modified().unwrap();
        let delta = restored
            .duration_since(target)
            .unwrap_or_else(|e| e.duration());
        assert!(delta < Duration::from_secs(1), "expected ~{target:?}, got {restored:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
