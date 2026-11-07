//! [`Ejector`] for Windows (spec task 6.5).
//!
//! Mirrors `diskutil unmount`/`NSWorkspace.unmountAndEject`: open the volume, lock it, force a
//! dismount, then ask the storage stack to eject the media — each step via `DeviceIoControl`. A
//! watchdog thread bounds the whole sequence so a wedged drive can never hang the caller.

use std::path::Path;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, GENERIC_READ, GENERIC_WRITE, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Ioctl::{FSCTL_DISMOUNT_VOLUME, FSCTL_LOCK_VOLUME};
use windows::Win32::System::IO::DeviceIoControl;
use windows::core::PCWSTR;

use crate::windows::util::to_wide;
use crate::{Ejector, EjectError};

/// Matches the macOS app's eject watchdog (design doc: "10s watchdog").
const EJECT_TIMEOUT: Duration = Duration::from_secs(10);

/// `IOCTL_STORAGE_EJECT_MEDIA`: not exposed as a named constant by the `windows` crate's
/// `Ioctl` module at this version, so it is defined here from `ntddstor.h`
/// (`FILE_DEVICE_MASS_STORAGE << 16 | 0x0202 << 2 | METHOD_BUFFERED | FILE_READ_ACCESS`).
const IOCTL_STORAGE_EJECT_MEDIA: u32 = 0x2D4808;

/// Real `Ejector` backed by `DeviceIoControl` lock+dismount+eject.
#[derive(Debug, Default)]
pub struct WindowsEjector;

impl WindowsEjector {
    pub fn new() -> Self {
        Self
    }

    fn do_eject(mount: &Path) -> Result<(), EjectError> {
        let letter = mount
            .to_string_lossy()
            .chars()
            .next()
            .ok_or_else(|| EjectError::NotFound(mount.display().to_string()))?;
        let device_path = format!(r"\\.\{letter}:");
        let device_path_w = to_wide(&device_path);

        let handle = unsafe {
            CreateFileW(
                PCWSTR(device_path_w.as_ptr()),
                (GENERIC_READ | GENERIC_WRITE).0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                HANDLE::default(),
            )
        }
        .map_err(|_| EjectError::NotFound(mount.display().to_string()))?;

        let result = (|| -> Result<(), EjectError> {
            send_ioctl(handle, FSCTL_LOCK_VOLUME)?;
            send_ioctl(handle, FSCTL_DISMOUNT_VOLUME)?;
            send_ioctl(handle, IOCTL_STORAGE_EJECT_MEDIA)?;
            Ok(())
        })();

        unsafe {
            let _ = CloseHandle(handle);
        }
        result
    }
}

fn send_ioctl(handle: HANDLE, code: u32) -> Result<(), EjectError> {
    let result = unsafe { DeviceIoControl(handle, code, None, 0, None, 0, None, None) };
    result.map_err(|e| {
        if e.code() == ERROR_ACCESS_DENIED.to_hresult() {
            EjectError::Busy
        } else {
            EjectError::Platform(format!("DeviceIoControl(0x{code:x}) failed: {e}"))
        }
    })
}

impl Ejector for WindowsEjector {
    fn eject(&self, mount: &Path) -> Result<(), EjectError> {
        let mount = mount.to_path_buf();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(Self::do_eject(&mount));
        });

        match rx.recv_timeout(EJECT_TIMEOUT) {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout) => Err(EjectError::Timeout),
            Err(RecvTimeoutError::Disconnected) => {
                Err(EjectError::Platform("eject worker thread panicked".into()))
            }
        }
    }
}
