//! Pure types/logic behind the now-playing bar + fullscreen overlay, ported from
//! `TrackPreviewBar.swift` / `FullScreenNowPlayingView.swift`. The rendering itself lives
//! in `iced_ui::now_playing` (the egui `render_now_playing_bar`/`render_fullscreen_overlay`
//! this module used to carry were removed once the iced migration reached parity — see
//! Milestone 8 of the egui-to-iced migration plan).

/// Minimal placeholder for the track the now-playing bar displays — only the fields the
/// bar renders are modelled here.
#[derive(Debug, Clone, PartialEq)]
pub struct NowPlayingPlaceholder {
    /// Track title (shown bold). Empty renders as `Unknown Title`, matching macOS.
    pub title: String,
    /// Track artist (shown muted after a bullet). Empty hides the artist run.
    pub artist: String,
    /// Total track duration in seconds (drives the scrubber range + the trailing time).
    pub duration_secs: f64,
    /// Current playback position in seconds (the scrubber thumb + the leading time).
    pub position_secs: f64,
    /// Whether playback is currently running (play vs. pause glyph).
    pub is_playing: bool,
    /// Whether the track is Hi-Res (shows the gold `HR` badge + tints the scrubber gold),
    /// mirroring `track.bitsPerSample > 16 || track.sampleRate > 48000`.
    pub is_hi_res: bool,
}

impl NowPlayingPlaceholder {
    /// A small demo track used by tests/previews.
    pub fn sample() -> Self {
        Self {
            title: "Blue Light".to_owned(),
            artist: "Mazzy Star".to_owned(),
            duration_secs: 214.0,
            position_secs: 73.0,
            is_playing: true,
            is_hi_res: true,
        }
    }
}

/// Height of the floating bar (matches the macOS `frame(height: 56)` TrackPreviewBar).
pub const PLAYER_BAR_HEIGHT: f32 = 56.0;

/// Formats `seconds` as `m:ss` (or `h:mm:ss` past an hour), matching the macOS
/// `TrackScrubberRow.formatTime`. Negative / non-finite inputs render as `0:00`.
pub fn format_time(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "0:00".to_owned();
    }
    let total = seconds as u64;
    let hours = total / 3600;
    let mins = (total % 3600) / 60;
    let secs = total % 60;
    if hours > 0 {
        format!("{hours}:{mins:02}:{secs:02}")
    } else {
        format!("{mins}:{secs:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_time_minutes_and_seconds() {
        assert_eq!(format_time(0.0), "0:00");
        assert_eq!(format_time(5.0), "0:05");
        assert_eq!(format_time(65.0), "1:05");
        assert_eq!(format_time(214.0), "3:34");
        assert_eq!(format_time(599.0), "9:59");
    }

    #[test]
    fn format_time_hours() {
        assert_eq!(format_time(3600.0), "1:00:00");
        assert_eq!(format_time(3661.0), "1:01:01");
        assert_eq!(format_time(7265.0), "2:01:05");
    }

    #[test]
    fn format_time_handles_invalid_input() {
        assert_eq!(format_time(-1.0), "0:00");
        assert_eq!(format_time(f64::NAN), "0:00");
        assert_eq!(format_time(f64::INFINITY), "0:00");
    }
}
