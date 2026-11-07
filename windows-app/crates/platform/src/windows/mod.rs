//! Windows (Win32 / WinRT) implementations of the platform traits.
//!
//! Implemented across spec tasks 6.2–6.7:
//! - `WindowsVolumeProvider`  (GetLogicalDrives / GetDiskFreeSpaceExW)
//! - `WindowsDeviceEvents`    (WM_DEVICECHANGE / RegisterDeviceNotification)
//! - `WindowsUsbEnumerator`   (SetupAPI / WMI)
//! - `WindowsEjector`         (IOCTL_STORAGE_EJECT_MEDIA)
//! - `WindowsFileDialogs`     (IFileOpenDialog / IFileSaveDialog)
//! - `WindowsMediaControls`   (SystemMediaTransportControls)
//!
//! This module is a placeholder during bootstrap (task 1) so the crate compiles on
//! Windows; the implementations land in task 6.
