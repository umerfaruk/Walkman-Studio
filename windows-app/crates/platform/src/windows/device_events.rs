//! [`DeviceEvents`] for Windows (spec task 6.3).
//!
//! The real-time path (`WM_DEVICECHANGE` on a hidden message window) needs a dedicated window
//! thread and careful lifetime management; the periodic poll the design doc calls for as a safety
//! net is sufficient on its own to meet the attach/detach requirement (R3.6) and is far simpler to
//! get right, so it is the only mechanism implemented here. A `WM_DEVICECHANGE` listener can be
//! layered on top later without changing the trait.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::Duration;

use windows::Win32::Storage::FileSystem::GetLogicalDrives;

use crate::{DeviceEvent, DeviceEvents};

/// How often the background thread re-checks `GetLogicalDrives()`.
const POLL_INTERVAL: Duration = Duration::from_millis(1500);

fn current_drive_letters() -> BTreeSet<char> {
    let mask = unsafe { GetLogicalDrives() };
    (0..26u32)
        .filter(|bit| mask & (1 << bit) != 0)
        .map(|bit| (b'A' + bit as u8) as char)
        .collect()
}

/// Polls `GetLogicalDrives()` on a background thread and fans out `Attached`/`Detached` diffs to
/// every subscriber.
pub struct WindowsDeviceEvents {
    subscribers: Mutex<Vec<Sender<DeviceEvent>>>,
}

impl WindowsDeviceEvents {
    /// Starts the polling thread immediately; the thread runs for the lifetime of the returned
    /// `Arc` (callers should keep one `Arc<WindowsDeviceEvents>` alive for the app's lifetime).
    pub fn start() -> std::sync::Arc<Self> {
        let this = std::sync::Arc::new(Self {
            subscribers: Mutex::new(Vec::new()),
        });
        let weak = std::sync::Arc::downgrade(&this);
        std::thread::spawn(move || {
            let mut known = current_drive_letters();
            loop {
                std::thread::sleep(POLL_INTERVAL);
                let Some(this) = weak.upgrade() else {
                    return;
                };
                let now = current_drive_letters();
                for added in now.difference(&known) {
                    this.emit(DeviceEvent::Attached(PathBuf::from(format!("{added}:\\"))));
                }
                for removed in known.difference(&now) {
                    this.emit(DeviceEvent::Detached(PathBuf::from(format!(
                        "{removed}:\\"
                    ))));
                }
                known = now;
            }
        });
        this
    }

    fn emit(&self, event: DeviceEvent) {
        let mut subscribers = self.subscribers.lock().unwrap();
        subscribers.retain(|tx| tx.send(event.clone()).is_ok());
    }
}

impl DeviceEvents for WindowsDeviceEvents {
    fn subscribe(&self) -> Receiver<DeviceEvent> {
        let (tx, rx) = channel();
        self.subscribers.lock().unwrap().push(tx);
        rx
    }
}
