//! [`MediaControls`] for Windows (spec task 6.7).
//!
//! macOS source: `MPNowPlayingInfoCenter`. Windows equivalent: WinRT
//! `Windows.Media.SystemMediaTransportControls`, bound to the app's main window via the
//! `ISystemMediaTransportControlsInterop::GetForWindow` interop interface (the standard desktop-
//! app pattern; `SystemMediaTransportControls::GetForCurrentView` only works for UWP apps).
//!
//! Artwork (`SystemMediaTransportControlsDisplayUpdater::SetThumbnail`) is not wired yet — it
//! needs an async `StorageFile`/`RandomAccessStreamReference` round trip and is left as a
//! follow-up; title/artist/album and the play/pause state are fully wired.

use windows::core::HSTRING;
use windows::Media::{MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;

use crate::{MediaControls, NowPlayingInfo};

/// Real `MediaControls` backed by `SystemMediaTransportControls` for `hwnd`.
///
/// `hwnd` is the app's main window handle; the `app`/`ui` crate supplies it once the window is
/// created (task 4/13 wiring).
pub struct WindowsMediaControls {
    controls: Option<SystemMediaTransportControls>,
}

impl WindowsMediaControls {
    pub fn new(hwnd: HWND) -> Self {
        let controls = Self::bind(hwnd)
            .inspect_err(|e| log::warn!("SystemMediaTransportControls unavailable: {e:?}"))
            .ok();
        Self { controls }
    }

    fn bind(hwnd: HWND) -> windows::core::Result<SystemMediaTransportControls> {
        let interop: ISystemMediaTransportControlsInterop =
            windows::core::factory::<SystemMediaTransportControls, _>()?;
        unsafe { interop.GetForWindow(hwnd) }
    }
}

impl MediaControls for WindowsMediaControls {
    fn set_now_playing(&self, info: &NowPlayingInfo) {
        let Some(controls) = &self.controls else {
            return;
        };
        let result: windows::core::Result<()> = (|| {
            let updater = controls.DisplayUpdater()?;
            updater.SetType(MediaPlaybackType::Music)?;
            let music = updater.MusicProperties()?;
            music.SetTitle(&HSTRING::from(info.title.as_str()))?;
            music.SetArtist(&HSTRING::from(info.artist.as_str()))?;
            music.SetAlbumTitle(&HSTRING::from(info.album.as_str()))?;
            updater.Update()
        })();
        if let Err(e) = result {
            log::warn!("failed to update now-playing info: {e:?}");
        }
    }

    fn set_playback_state(&self, playing: bool) {
        let Some(controls) = &self.controls else {
            return;
        };
        let status = if playing {
            MediaPlaybackStatus::Playing
        } else {
            MediaPlaybackStatus::Paused
        };
        if let Err(e) = controls.SetPlaybackStatus(status) {
            log::warn!("failed to update playback status: {e:?}");
        }
    }
}
