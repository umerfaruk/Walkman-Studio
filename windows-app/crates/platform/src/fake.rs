//! In-memory fake implementations of the platform traits (spec task 6.1).
//!
//! These carry no OS dependency so they compile and run on every host, letting
//! `services`/`ui` unit tests exercise device-detection, eject, and dialog flows without
//! touching real hardware. Each fake exposes extra methods (not part of the trait) for
//! tests to seed state and assert on calls made through the trait.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;

use crate::{
    DeviceEvent, DeviceEvents, Ejector, EjectError, FileDialogs, FreeSpace, MediaControls,
    NowPlayingInfo, UsbDevice, UsbEnumerator, VolumeInfo, VolumeProvider,
};

/// A [`VolumeProvider`] backed by an in-memory list, for tests.
#[derive(Debug, Default)]
pub struct FakeVolumeProvider {
    volumes: Mutex<Vec<VolumeInfo>>,
    free_space: Mutex<HashMap<PathBuf, FreeSpace>>,
}

impl FakeVolumeProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_volumes(&self, volumes: Vec<VolumeInfo>) {
        *self.volumes.lock().unwrap() = volumes;
    }

    pub fn set_free_space(&self, path: impl Into<PathBuf>, space: FreeSpace) {
        self.free_space.lock().unwrap().insert(path.into(), space);
    }
}

impl VolumeProvider for FakeVolumeProvider {
    fn list_volumes(&self) -> Vec<VolumeInfo> {
        self.volumes.lock().unwrap().clone()
    }

    fn free_space(&self, path: &Path) -> std::io::Result<FreeSpace> {
        self.free_space
            .lock()
            .unwrap()
            .get(path)
            .copied()
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("no fake free-space entry for {}", path.display()),
                )
            })
    }
}

/// A [`DeviceEvents`] fake: `emit` fans an event out to every live subscriber.
#[derive(Default)]
pub struct FakeDeviceEvents {
    subscribers: Mutex<Vec<Sender<DeviceEvent>>>,
}

impl FakeDeviceEvents {
    pub fn new() -> Self {
        Self::default()
    }

    /// Delivers `event` to every receiver created via [`DeviceEvents::subscribe`] so far.
    /// Senders whose receiver has been dropped are pruned.
    pub fn emit(&self, event: DeviceEvent) {
        let mut subscribers = self.subscribers.lock().unwrap();
        subscribers.retain(|tx| tx.send(event.clone()).is_ok());
    }
}

impl DeviceEvents for FakeDeviceEvents {
    fn subscribe(&self) -> Receiver<DeviceEvent> {
        let (tx, rx) = channel();
        self.subscribers.lock().unwrap().push(tx);
        rx
    }
}

/// A [`UsbEnumerator`] backed by an in-memory device list.
#[derive(Debug, Default)]
pub struct FakeUsbEnumerator {
    devices: Mutex<Vec<UsbDevice>>,
}

impl FakeUsbEnumerator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_devices(&self, devices: Vec<UsbDevice>) {
        *self.devices.lock().unwrap() = devices;
    }
}

impl UsbEnumerator for FakeUsbEnumerator {
    fn scan(&self) -> Vec<UsbDevice> {
        self.devices.lock().unwrap().clone()
    }
}

/// An [`Ejector`] fake whose outcome and call history are inspectable.
#[derive(Default)]
pub struct FakeEjector {
    result: Mutex<Option<EjectError>>,
    calls: Mutex<Vec<PathBuf>>,
}

impl FakeEjector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes the next (and all subsequent) `eject` calls fail with `error`.
    pub fn fail_with(&self, error: EjectError) {
        *self.result.lock().unwrap() = Some(error);
    }

    pub fn calls(&self) -> Vec<PathBuf> {
        self.calls.lock().unwrap().clone()
    }
}

impl Ejector for FakeEjector {
    fn eject(&self, mount: &Path) -> Result<(), EjectError> {
        self.calls.lock().unwrap().push(mount.to_path_buf());
        match &*self.result.lock().unwrap() {
            Some(EjectError::Busy) => Err(EjectError::Busy),
            Some(EjectError::Timeout) => Err(EjectError::Timeout),
            Some(EjectError::NotFound(s)) => Err(EjectError::NotFound(s.clone())),
            Some(EjectError::Platform(s)) => Err(EjectError::Platform(s.clone())),
            None => Ok(()),
        }
    }
}

/// A [`FileDialogs`] fake returning pre-seeded answers and recording reveal calls.
#[derive(Default)]
pub struct FakeFileDialogs {
    pick_folder_answer: Mutex<Option<PathBuf>>,
    save_file_answer: Mutex<Option<PathBuf>>,
    revealed: Mutex<Vec<PathBuf>>,
}

impl FakeFileDialogs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_pick_folder_answer(&self, answer: Option<PathBuf>) {
        *self.pick_folder_answer.lock().unwrap() = answer;
    }

    pub fn set_save_file_answer(&self, answer: Option<PathBuf>) {
        *self.save_file_answer.lock().unwrap() = answer;
    }

    pub fn revealed(&self) -> Vec<PathBuf> {
        self.revealed.lock().unwrap().clone()
    }
}

impl FileDialogs for FakeFileDialogs {
    fn pick_folder(&self, _title: &str) -> Option<PathBuf> {
        self.pick_folder_answer.lock().unwrap().clone()
    }

    fn save_file(&self, _suggested_name: &str, _filters: &[(&str, &str)]) -> Option<PathBuf> {
        self.save_file_answer.lock().unwrap().clone()
    }

    fn reveal_in_file_manager(&self, path: &Path) {
        self.revealed.lock().unwrap().push(path.to_path_buf());
    }
}

/// A [`MediaControls`] fake capturing the last now-playing info and playback state pushed.
#[derive(Default)]
pub struct FakeMediaControls {
    now_playing: Mutex<Option<NowPlayingInfo>>,
    playing: Mutex<Option<bool>>,
}

impl FakeMediaControls {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn last_now_playing(&self) -> Option<NowPlayingInfo> {
        self.now_playing.lock().unwrap().clone()
    }

    pub fn last_playback_state(&self) -> Option<bool> {
        *self.playing.lock().unwrap()
    }
}

impl MediaControls for FakeMediaControls {
    fn set_now_playing(&self, info: &NowPlayingInfo) {
        *self.now_playing.lock().unwrap() = Some(info.clone());
    }

    fn set_playback_state(&self, playing: bool) {
        *self.playing.lock().unwrap() = Some(playing);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_volume_provider_round_trips() {
        let provider = FakeVolumeProvider::new();
        let volume = VolumeInfo {
            path: PathBuf::from(r"E:\"),
            label: Some("WALKMAN".into()),
            filesystem: "exFAT".into(),
            is_removable: true,
            is_read_only: false,
        };
        provider.set_volumes(vec![volume.clone()]);
        provider.set_free_space(
            r"E:\",
            FreeSpace {
                total_bytes: 1000,
                free_bytes: 400,
            },
        );

        assert_eq!(provider.list_volumes(), vec![volume]);
        let space = provider.free_space(Path::new(r"E:\")).unwrap();
        assert_eq!(space.used_bytes(), 600);
        assert!(provider.free_space(Path::new(r"Z:\")).is_err());
    }

    #[test]
    fn fake_device_events_fans_out_to_subscribers() {
        let events = FakeDeviceEvents::new();
        let rx1 = events.subscribe();
        let rx2 = events.subscribe();

        events.emit(DeviceEvent::Attached(PathBuf::from(r"E:\")));

        assert_eq!(
            rx1.recv().unwrap(),
            DeviceEvent::Attached(PathBuf::from(r"E:\"))
        );
        assert_eq!(
            rx2.recv().unwrap(),
            DeviceEvent::Attached(PathBuf::from(r"E:\"))
        );
    }

    #[test]
    fn fake_usb_enumerator_returns_seeded_devices() {
        let usb = FakeUsbEnumerator::new();
        let device = UsbDevice {
            vendor_id: 0x054c,
            product_id: 0x0d00,
            serial: Some("abc".into()),
            product_name: Some("NW-A105".into()),
        };
        usb.set_devices(vec![device.clone()]);
        assert_eq!(usb.scan(), vec![device]);
    }

    #[test]
    fn fake_ejector_reports_configured_failure_and_records_calls() {
        let ejector = FakeEjector::new();
        assert!(ejector.eject(Path::new(r"E:\")).is_ok());

        ejector.fail_with(EjectError::Busy);
        let err = ejector.eject(Path::new(r"E:\")).unwrap_err();
        assert!(matches!(err, EjectError::Busy));

        assert_eq!(ejector.calls(), vec![PathBuf::from(r"E:\"), PathBuf::from(r"E:\")]);
    }

    #[test]
    fn fake_file_dialogs_returns_seeded_answers_and_records_reveals() {
        let dialogs = FakeFileDialogs::new();
        dialogs.set_pick_folder_answer(Some(PathBuf::from(r"C:\Music")));
        assert_eq!(
            dialogs.pick_folder("Choose a folder"),
            Some(PathBuf::from(r"C:\Music"))
        );

        dialogs.reveal_in_file_manager(Path::new(r"C:\Music\song.flac"));
        assert_eq!(dialogs.revealed(), vec![PathBuf::from(r"C:\Music\song.flac")]);
    }

    #[test]
    fn fake_media_controls_captures_last_state() {
        let controls = FakeMediaControls::new();
        let info = NowPlayingInfo {
            title: "Title".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            artwork_path: None,
            duration_seconds: 180,
            position_seconds: 10,
        };
        controls.set_now_playing(&info);
        controls.set_playback_state(true);

        assert_eq!(controls.last_now_playing(), Some(info));
        assert_eq!(controls.last_playback_state(), Some(true));
    }
}
