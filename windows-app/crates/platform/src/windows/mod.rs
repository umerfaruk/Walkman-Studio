//! Windows (Win32 / WinRT) implementations of the platform traits (spec task 6).
//!
//! - [`volumes::WindowsVolumeProvider`]      (`GetLogicalDrives` / `GetDiskFreeSpaceExW`)
//! - [`device_events::WindowsDeviceEvents`]  (periodic poll of `GetLogicalDrives`)
//! - [`usb::WindowsUsbEnumerator`]           (`Get-PnpDevice`)
//! - [`eject::WindowsEjector`]               (`IOCTL_STORAGE_EJECT_MEDIA`)
//! - [`dialogs::WindowsFileDialogs`]         (`SHBrowseForFolderW` / `GetSaveFileNameW`)
//! - [`media_controls::WindowsMediaControls`] (`SystemMediaTransportControls`)

mod util;

pub mod device_events;
pub mod dialogs;
pub mod eject;
pub mod fs_time;
pub mod media_controls;
pub mod usb;
pub mod volumes;

pub use device_events::WindowsDeviceEvents;
pub use dialogs::WindowsFileDialogs;
pub use eject::WindowsEjector;
pub use fs_time::set_file_times;
pub use media_controls::WindowsMediaControls;
pub use usb::WindowsUsbEnumerator;
pub use volumes::WindowsVolumeProvider;
