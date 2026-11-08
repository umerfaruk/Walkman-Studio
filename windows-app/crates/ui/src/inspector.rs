//! Pure types/logic behind the trailing inspector, ported from `MetadataStudioView`
//! (`Sources/WalkmanStudio/Views/MetadataStudioView.swift`). The rendering itself lives
//! in `iced_ui::views` (the egui `render` this module used to carry was removed once the
//! iced migration reached parity — see Milestone 8 of the egui-to-iced migration plan).

/// Raised when the inspector's `Edit` button is pressed, carrying the selected track's id
/// ([`Track::id`]). Consumed by whoever opens the Song Metadata Editor sheet.
#[derive(Debug, Clone, PartialEq)]
pub enum InspectorCommand {
    EditRequested(String),
    EditLyricsRequested(String),
}

#[derive(Debug, Clone, Default)]
pub struct InspectorViewState {
    pub commands: Vec<InspectorCommand>,
}

/// Formats a `kbps` bitrate; `0`/negative renders as `"\u{2014}"` (unknown, e.g. lossless
/// formats that don't report one meaningfully).
pub fn format_bitrate(kbps: i32) -> String {
    if kbps <= 0 {
        "\u{2014}".to_owned()
    } else {
        format!("{kbps} kbps")
    }
}

/// Formats a sample rate in Hz as kHz (e.g. `44100` -> `"44.1 kHz"`).
pub fn format_sample_rate(hz: i32) -> String {
    if hz <= 0 {
        return "\u{2014}".to_owned();
    }
    format!("{:.1} kHz", hz as f64 / 1000.0)
}

/// Formats seconds as `m:ss`, matching the now-playing bar's own
/// [`crate::now_playing::format_time`] convention (duplicated as a tiny pure function
/// rather than depending across modules for one call).
pub fn format_duration(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "0:00".to_owned();
    }
    let total = seconds as u64;
    format!("{}:{:02}", total / 60, total % 60)
}

/// Formats a byte count as the largest whole unit (B/KB/MB/GB) with one decimal place.
pub fn format_size(bytes: i64) -> String {
    let bytes = bytes.max(0) as f64;
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes;
    let mut unit_index = 0;
    while value >= 1024.0 && unit_index < UNITS.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }
    if unit_index == 0 {
        format!("{value:.0} {}", UNITS[unit_index])
    } else {
        format!("{value:.1} {}", UNITS[unit_index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use walkman_domain::models::Track;

    fn sample_track() -> Track {
        Track {
            filename: "song.flac".into(),
            relative_path: "Artist/Album/song.flac".into(),
            full_path: "D:/Music/Artist/Album/song.flac".into(),
            format: "FLAC".into(),
            title: "Song Title".into(),
            artist: "The Artist".into(),
            album: "The Album".into(),
            track_num: "3".into(),
            year: "2021".into(),
            genre: "Rock".into(),
            duration_seconds: 245.0,
            bitrate_kbps: 900,
            sample_rate: 96_000,
            bits_per_sample: 24,
            has_artwork: true,
            size_bytes: 25_000_000,
            date_added: Utc::now(),
            is_senseme_analyzed: false,
            senseme_bpm: None,
            senseme_valence: None,
            senseme_arousal: None,
            senseme_channels: Vec::new(),
            senseme_highlight_sec: None,
        }
    }

    #[test]
    fn format_bitrate_handles_zero_and_positive() {
        assert_eq!(format_bitrate(0), "\u{2014}");
        assert_eq!(format_bitrate(-5), "\u{2014}");
        assert_eq!(format_bitrate(320), "320 kbps");
    }

    #[test]
    fn format_sample_rate_converts_to_khz() {
        assert_eq!(format_sample_rate(44_100), "44.1 kHz");
        assert_eq!(format_sample_rate(96_000), "96.0 kHz");
        assert_eq!(format_sample_rate(0), "\u{2014}");
    }

    #[test]
    fn format_duration_matches_mss() {
        assert_eq!(format_duration(65.0), "1:05");
        assert_eq!(format_duration(-1.0), "0:00");
        assert_eq!(format_duration(f64::NAN), "0:00");
    }

    #[test]
    fn format_size_picks_the_largest_whole_unit() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(2048), "2.0 KB");
        assert_eq!(format_size(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(format_size(3 * 1024 * 1024 * 1024), "3.0 GB");
    }

    #[test]
    fn edit_command_carries_the_tracks_id() {
        let track = sample_track();
        let mut state = InspectorViewState::default();
        state.commands.push(InspectorCommand::EditRequested(track.id().to_owned()));
        assert_eq!(state.commands, vec![InspectorCommand::EditRequested(track.full_path.clone())]);
    }
}
