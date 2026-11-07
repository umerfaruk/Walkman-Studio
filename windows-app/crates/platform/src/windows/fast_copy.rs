//! Fast file copy with buffer flushing (spec task 11.2, R12.6): `CopyFileExW` (the Windows copy
//! API, which can use a more efficient path than a manual read/write loop) followed by
//! `FlushFileBuffers` on the destination so a backup/restore copy is durable on disk before the
//! next step (e.g. verifying it, or ejecting the drive) proceeds.

use std::path::Path;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, GENERIC_WRITE, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CopyFileExW, FlushFileBuffers, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, OPEN_EXISTING,
};

use crate::windows::util::to_wide;

/// Copies `src` to `dst` via `CopyFileExW`, then flushes `dst`'s buffers to disk.
pub fn fast_copy(src: &Path, dst: &Path) -> std::io::Result<()> {
    let src_w = to_wide(&src.to_string_lossy());
    let dst_w = to_wide(&dst.to_string_lossy());

    unsafe {
        CopyFileExW(
            PCWSTR(src_w.as_ptr()),
            PCWSTR(dst_w.as_ptr()),
            None,
            None,
            None,
            0,
        )
    }
    .map_err(std::io::Error::other)?;

    flush_file(dst)
}

fn flush_file(path: &Path) -> std::io::Result<()> {
    let path_w = to_wide(&path.to_string_lossy());
    let handle = unsafe {
        windows::Win32::Storage::FileSystem::CreateFileW(
            PCWSTR(path_w.as_ptr()),
            GENERIC_WRITE.0,
            FILE_SHARE_READ,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            HANDLE::default(),
        )
    }
    .map_err(std::io::Error::other)?;

    let result = unsafe { FlushFileBuffers(handle) };
    unsafe {
        let _ = CloseHandle(handle);
    }
    result.map_err(std::io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_file_contents_exactly() {
        let dir = std::env::temp_dir().join(format!("walkman-fast-copy-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.bin");
        let dst = dir.join("dst.bin");
        std::fs::write(&src, b"hello walkman").unwrap();

        fast_copy(&src, &dst).unwrap();
        assert_eq!(std::fs::read(&dst).unwrap(), b"hello walkman");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
