//! Audio playback engine (spec task 13, R7.1/7.4/7.5).
//!
//! Wraps `rodio` (`cpal`/WASAPI under the hood) for decode + output; queue/advance and SMTC
//! integration are this module's own logic on top. There's no background "watcher" thread for
//! detecting a finished track — [`Player::poll`] is a plain method the UI calls once per frame
//! (the same channel/poll threading model the rest of this crate's off-thread work uses, per
//! design.md), which avoids a second thread whose lifecycle would need managing alongside the
//! `Player` itself.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::{OutputStream, OutputStreamHandle, Sink};
use walkman_domain::models::Track;
use walkman_platform::{MediaControls, NowPlayingInfo};

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone)]
pub struct PlayerStatus {
    pub state: PlaybackState,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub current_index: Option<usize>,
}

/// Called when a track finishes and the queue advances (R7.5 — used by the SensMe highlight
/// mode to seek the next track to its highlight point rather than start from 0).
pub type AdvanceHook = Box<dyn Fn(&Track) + Send + Sync>;

struct Inner {
    _stream: OutputStream, // must outlive `sink`; never read, only kept alive.
    stream_handle: OutputStreamHandle,
    sink: Option<Sink>,
    queue: Vec<Track>,
    index: usize,
    duration_seconds: f64,
    advance_hook: Option<AdvanceHook>,
    /// Set right after a track starts; [`Player::poll`] uses it to tell "track finished
    /// naturally" (sink empty, was playing) apart from "nothing loaded yet".
    was_playing: bool,
}

/// The playback engine. Construction opens the default audio output device — see
/// [`Player::new`]'s doc for what happens if there isn't one.
pub struct Player {
    inner: Mutex<Inner>,
    media_controls: Option<Arc<dyn MediaControls>>,
}

impl Player {
    /// Opens the default output device. Fails with [`AppError::Device`] if there is none (e.g.
    /// a headless CI/VM host with no audio hardware) — callers on a real desktop should not
    /// expect this to fail, but should still surface the error rather than panic if it does.
    pub fn new(media_controls: Option<Arc<dyn MediaControls>>) -> Result<Self> {
        let (stream, stream_handle) =
            OutputStream::try_default().map_err(|e| AppError::Device(format!("no audio output device: {e}")))?;
        Ok(Self {
            inner: Mutex::new(Inner {
                _stream: stream,
                stream_handle,
                sink: None,
                queue: Vec::new(),
                index: 0,
                duration_seconds: 0.0,
                advance_hook: None,
                was_playing: false,
            }),
            media_controls,
        })
    }

    pub fn set_queue(&self, tracks: Vec<Track>) {
        let mut inner = self.inner.lock().unwrap();
        inner.queue = tracks;
        inner.index = 0;
    }

    pub fn queue(&self) -> Vec<Track> {
        self.inner.lock().unwrap().queue.clone()
    }

    /// R7.5's advance hook, invoked from [`Player::poll`] when the current track finishes.
    pub fn set_advance_hook(&self, hook: impl Fn(&Track) + Send + Sync + 'static) {
        self.inner.lock().unwrap().advance_hook = Some(Box::new(hook));
    }

    /// Loads and plays the queue item at `index`.
    pub fn play_index(&self, index: usize) -> Result<()> {
        let track = {
            let inner = self.inner.lock().unwrap();
            inner.queue.get(index).cloned().ok_or_else(|| AppError::Device("queue index out of range".into()))?
        };

        let file = std::fs::File::open(&track.full_path)?;
        let decoder = rodio::Decoder::new(std::io::BufReader::new(file))
            .map_err(|e| AppError::Device(format!("failed to decode {}: {e}", track.full_path)))?;
        let duration_seconds =
            rodio::Source::total_duration(&decoder).map(|d| d.as_secs_f64()).unwrap_or(track.duration_seconds);

        let mut inner = self.inner.lock().unwrap();
        let sink = Sink::try_new(&inner.stream_handle).map_err(|e| AppError::Device(e.to_string()))?;
        sink.append(decoder);
        sink.play();
        inner.sink = Some(sink);
        inner.index = index;
        inner.duration_seconds = duration_seconds;
        inner.was_playing = true;
        drop(inner);

        self.push_now_playing(&track, duration_seconds);
        Ok(())
    }

    pub fn play(&self) {
        let inner = self.inner.lock().unwrap();
        if let Some(sink) = &inner.sink {
            sink.play();
        }
        drop(inner);
        self.push_playback_state(true);
    }

    pub fn pause(&self) {
        let inner = self.inner.lock().unwrap();
        if let Some(sink) = &inner.sink {
            sink.pause();
        }
        drop(inner);
        self.push_playback_state(false);
    }

    pub fn toggle_play_pause(&self) {
        let is_paused = self.inner.lock().unwrap().sink.as_ref().map(Sink::is_paused).unwrap_or(true);
        if is_paused {
            self.play();
        } else {
            self.pause();
        }
    }

    pub fn seek(&self, position_seconds: f64) -> Result<()> {
        let inner = self.inner.lock().unwrap();
        if let Some(sink) = &inner.sink {
            sink.try_seek(Duration::from_secs_f64(position_seconds.max(0.0)))
                .map_err(|e| AppError::Device(e.to_string()))?;
        }
        Ok(())
    }

    pub fn next(&self) -> Result<()> {
        let index = self.inner.lock().unwrap().index + 1;
        self.play_index(index)
    }

    pub fn previous(&self) -> Result<()> {
        let index = self.inner.lock().unwrap().index.saturating_sub(1);
        self.play_index(index)
    }

    pub fn status(&self) -> PlayerStatus {
        let inner = self.inner.lock().unwrap();
        let (state, position_seconds) = match &inner.sink {
            None => (PlaybackState::Stopped, 0.0),
            Some(sink) => {
                let position = sink.get_pos().as_secs_f64();
                if sink.empty() {
                    (PlaybackState::Stopped, position)
                } else if sink.is_paused() {
                    (PlaybackState::Paused, position)
                } else {
                    (PlaybackState::Playing, position)
                }
            }
        };
        PlayerStatus {
            state,
            position_seconds,
            duration_seconds: inner.duration_seconds,
            current_index: (!inner.queue.is_empty()).then_some(inner.index),
        }
    }

    /// Call once per UI frame: detects a track that finished naturally (the sink went empty
    /// while it was playing) and, if so, runs the advance hook (R7.5) and auto-advances to the
    /// next queued track, if there is one. Returns the track that just started, if any.
    pub fn poll(&self) -> Option<Track> {
        let (finished_track, next_index) = {
            let mut inner = self.inner.lock().unwrap();
            let sink_empty = inner.sink.as_ref().map(Sink::empty).unwrap_or(false);
            if !(sink_empty && inner.was_playing) {
                return None;
            }
            inner.was_playing = false;
            let finished = inner.queue.get(inner.index).cloned();
            (finished, inner.index + 1)
        };

        if let (Some(track), Some(hook)) = (&finished_track, self.inner.lock().unwrap().advance_hook.as_ref()) {
            hook(track);
        }

        if next_index < self.inner.lock().unwrap().queue.len() {
            let _ = self.play_index(next_index);
            return self.inner.lock().unwrap().queue.get(next_index).cloned();
        }
        None
    }

    fn push_now_playing(&self, track: &Track, duration_seconds: f64) {
        let Some(controls) = &self.media_controls else { return };
        controls.set_now_playing(&NowPlayingInfo {
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            artwork_path: None,
            duration_seconds: duration_seconds.max(0.0) as u64,
            position_seconds: 0,
        });
        controls.set_playback_state(true);
    }

    fn push_playback_state(&self, playing: bool) {
        if let Some(controls) = &self.media_controls {
            controls.set_playback_state(playing);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use walkman_platform::fake::FakeMediaControls;

    fn sample_track(path: &str, duration: f64) -> Track {
        Track {
            filename: "x".into(),
            relative_path: "x".into(),
            full_path: path.into(),
            format: "WAV".into(),
            title: "Title".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            track_num: "1".into(),
            year: "2024".into(),
            genre: String::new(),
            duration_seconds: duration,
            bitrate_kbps: 900,
            sample_rate: 44_100,
            bits_per_sample: 16,
            has_artwork: false,
            size_bytes: 1,
            date_added: Utc::now(),
            is_senseme_analyzed: false,
            senseme_bpm: None,
            senseme_valence: None,
            senseme_arousal: None,
            senseme_channels: Vec::new(),
            senseme_highlight_sec: None,
        }
    }

    fn write_tiny_wav(path: &std::path::Path, num_samples: usize) {
        let samples: Vec<i16> = (0..num_samples).map(|i| ((i % 100) as i16) * 100).collect();
        let data_len = (samples.len() * 2) as u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&8_000u32.to_le_bytes());
        bytes.extend_from_slice(&16_000u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            bytes.extend_from_slice(&s.to_le_bytes());
        }
        std::fs::write(path, bytes).unwrap();
    }

    /// These tests need a real default audio output device (`OutputStream::try_default`); on a
    /// host with none (e.g. some headless CI runners), they're skipped rather than failed,
    /// since that's an environment property, not something this code controls.
    macro_rules! player_or_skip {
        ($controls:expr) => {
            match Player::new($controls) {
                Ok(p) => p,
                Err(_) => {
                    eprintln!("skipping: no default audio output device on this host");
                    return;
                }
            }
        };
    }

    #[test]
    fn status_is_stopped_before_anything_is_played() {
        let player = player_or_skip!(None);
        let status = player.status();
        assert_eq!(status.state, PlaybackState::Stopped);
        assert_eq!(status.current_index, None);
    }

    #[test]
    fn play_index_loads_and_starts_the_track_and_pushes_now_playing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        write_tiny_wav(&path, 80_000); // ~10s at 8kHz mono, long enough not to finish mid-test

        let controls = Arc::new(FakeMediaControls::new());
        let player = player_or_skip!(Some(controls.clone() as Arc<dyn MediaControls>));
        player.set_queue(vec![sample_track(path.to_str().unwrap(), 10.0)]);
        player.play_index(0).unwrap();

        let status = player.status();
        assert_eq!(status.current_index, Some(0));
        assert_ne!(status.state, PlaybackState::Stopped);

        let now_playing = controls.last_now_playing().unwrap();
        assert_eq!(now_playing.title, "Title");
        assert_eq!(controls.last_playback_state(), Some(true));
    }

    #[test]
    fn pause_and_toggle_update_state_and_media_controls() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        write_tiny_wav(&path, 80_000);

        let controls = Arc::new(FakeMediaControls::new());
        let player = player_or_skip!(Some(controls.clone() as Arc<dyn MediaControls>));
        player.set_queue(vec![sample_track(path.to_str().unwrap(), 10.0)]);
        player.play_index(0).unwrap();

        player.pause();
        assert_eq!(controls.last_playback_state(), Some(false));
        assert!(player.status().state == PlaybackState::Paused || player.status().state == PlaybackState::Stopped);

        player.toggle_play_pause();
        assert_eq!(controls.last_playback_state(), Some(true));
    }

    #[test]
    fn poll_auto_advances_to_the_next_track_and_runs_the_advance_hook() {
        let dir = tempfile::tempdir().unwrap();
        let short_path = dir.path().join("short.wav");
        let next_path = dir.path().join("next.wav");
        write_tiny_wav(&short_path, 50); // a handful of ms: finishes almost immediately
        write_tiny_wav(&next_path, 80_000);

        let player = player_or_skip!(None);
        player.set_queue(vec![
            sample_track(short_path.to_str().unwrap(), 0.01),
            sample_track(next_path.to_str().unwrap(), 10.0),
        ]);

        let advanced = Arc::new(Mutex::new(None));
        let advanced_clone = Arc::clone(&advanced);
        player.set_advance_hook(move |track| {
            *advanced_clone.lock().unwrap() = Some(track.full_path.clone());
        });

        player.play_index(0).unwrap();
        // Give the short clip real wall-clock time to finish playing before polling.
        std::thread::sleep(Duration::from_millis(500));

        let started = player.poll();
        assert!(started.is_some(), "expected poll() to auto-advance to the next track");
        assert_eq!(advanced.lock().unwrap().as_deref(), Some(short_path.to_str().unwrap()));
        assert_eq!(player.status().current_index, Some(1));
    }

    #[test]
    fn next_and_previous_move_the_queue_index() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.wav");
        let b = dir.path().join("b.wav");
        write_tiny_wav(&a, 80_000);
        write_tiny_wav(&b, 80_000);

        let player = player_or_skip!(None);
        player.set_queue(vec![sample_track(a.to_str().unwrap(), 10.0), sample_track(b.to_str().unwrap(), 10.0)]);
        player.play_index(0).unwrap();

        player.next().unwrap();
        assert_eq!(player.status().current_index, Some(1));
        player.previous().unwrap();
        assert_eq!(player.status().current_index, Some(0));
    }
}
