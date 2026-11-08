//! Trim & Cut audio studio (spec task 12.1, R14.1-14.5).
//!
//! Waveform peaks come from decoding PCM directly via `symphonia` (pure Rust, no process
//! spawned); if that fails (an unsupported/corrupt file), [`generate_waveform`] falls back to
//! shelling out to bundled `ffmpeg` to decode to raw `f32` PCM on stdout. Trim/cut/channel-
//! routing/fade/gain processing always goes through `ffmpeg` (R14.5) — there's no safe way to
//! splice compressed audio by hand — built as a single `-filter_complex` graph so trim-vs-cut,
//! channel ops, fades, and gain compose into one pass regardless of which options are set.
//! Original tags and artwork are re-applied to the processed output afterward (via
//! `audio_tagger`), since `ffmpeg`'s filter graph necessarily re-encodes and drops them.

use std::path::Path;
use std::process::Command;

use symphonia::core::audio::{AudioBufferRef, SampleBuffer};
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::audio_tagger::{self, TagFields};
use crate::error::{AppError, Result};
use crate::ffmpeg_util::find_ffmpeg;

/// R14.2's two modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrimCutMode {
    /// Keep only the selected range.
    Trim,
    /// Remove the selected range, keeping everything before and after it.
    Cut,
}

/// R14.4's channel-routing options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChannelMode {
    #[default]
    None,
    /// Invert L/R (swap channels).
    SwapLr,
    /// Downmix to mono.
    Mono,
    /// Force 2-channel stereo (duplicates a mono source; leaves true stereo unchanged).
    Stereo,
    IsolateLeft,
    IsolateRight,
    /// Silences the left channel, leaving the right channel on its own side (distinct from
    /// [`ChannelMode::IsolateRight`], which duplicates the right channel to *both* ears) —
    /// `AudioChannelMode.muteLeft` ("Mute Left Channel (Right Only Output)") in the macOS
    /// source, added here after task 19.4's parity check found it (and `MuteRight`) missing.
    MuteLeft,
    /// Silences the right channel, leaving the left channel on its own side — macOS's
    /// `AudioChannelMode.muteRight`.
    MuteRight,
    /// 180° phase flip.
    PhaseFlip,
}

/// R14.3/14.4/14.5's processing options. Times are in milliseconds (the UI's steppers are
/// ±0.1/±1/±5s); `gain_db` is clamped by the caller to R14.5's −6dB..+6dB range (not enforced
/// here, so test fixtures can exercise out-of-range values if ever needed).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrimCutOptions {
    pub mode: TrimCutMode,
    pub start_ms: u64,
    pub end_ms: u64,
    pub channel_mode: ChannelMode,
    pub fade_in_ms: u64,
    pub fade_out_ms: u64,
    pub gain_db: f32,
}

fn channel_filter(mode: ChannelMode) -> Option<&'static str> {
    match mode {
        ChannelMode::None => None,
        ChannelMode::SwapLr => Some("pan=stereo|c0=c1|c1=c0"),
        ChannelMode::Mono => Some("aformat=channel_layouts=mono"),
        ChannelMode::Stereo => Some("aformat=channel_layouts=stereo"),
        ChannelMode::IsolateLeft => Some("pan=stereo|c0=c0|c1=c0"),
        ChannelMode::IsolateRight => Some("pan=stereo|c0=c1|c1=c1"),
        ChannelMode::MuteLeft => Some("pan=stereo|c0=0|c1=c1"),
        ChannelMode::MuteRight => Some("pan=stereo|c0=c0|c1=0"),
        ChannelMode::PhaseFlip => Some("aeval=-val(0)|-val(1):channel_layouts=stereo"),
    }
}

/// Builds the `ffmpeg` `-filter_complex` graph and full argument list for `options`, given the
/// source's total `duration_ms` (needed for `Cut`'s post-selection segment and for placing the
/// fade-out). Always maps a final `[out]` label, so the caller's `-map` target never depends on
/// which optional stages were applied.
pub fn build_ffmpeg_args(input: &Path, output: &Path, options: &TrimCutOptions, duration_ms: u64) -> Vec<String> {
    let start_s = options.start_ms as f64 / 1000.0;
    let end_s = options.end_ms as f64 / 1000.0;
    let duration_s = duration_ms as f64 / 1000.0;

    let mut parts: Vec<String> = Vec::new();
    let mut current = match options.mode {
        TrimCutMode::Trim => {
            parts.push(format!("[0:a]atrim=start={start_s:.3}:end={end_s:.3}[trimmed]"));
            "trimmed".to_string()
        }
        TrimCutMode::Cut => {
            parts.push("[0:a]asplit=2[pre][post]".to_string());
            parts.push(format!("[pre]atrim=start=0:end={start_s:.3}[preT]"));
            parts.push(format!("[post]atrim=start={end_s:.3}:end={duration_s:.3}[postT]"));
            parts.push("[preT][postT]concat=n=2:v=0:a=1[trimmed]".to_string());
            "trimmed".to_string()
        }
    };

    if let Some(filter) = channel_filter(options.channel_mode) {
        parts.push(format!("[{current}]{filter}[chan]"));
        current = "chan".to_string();
    }

    let selected_duration_s = match options.mode {
        TrimCutMode::Trim => end_s - start_s,
        TrimCutMode::Cut => duration_s - (end_s - start_s),
    };

    if options.fade_in_ms > 0 {
        let d = options.fade_in_ms as f64 / 1000.0;
        parts.push(format!("[{current}]afade=t=in:st=0:d={d:.3}[fadein]"));
        current = "fadein".to_string();
    }
    if options.fade_out_ms > 0 {
        let d = options.fade_out_ms as f64 / 1000.0;
        let start = (selected_duration_s - d).max(0.0);
        parts.push(format!("[{current}]afade=t=out:st={start:.3}:d={d:.3}[fadeout]"));
        current = "fadeout".to_string();
    }
    if options.gain_db != 0.0 {
        parts.push(format!("[{current}]volume={:.2}dB[gain]", options.gain_db));
        current = "gain".to_string();
    }
    // A trailing no-op keeps the final map target always "[out]" regardless of which optional
    // stages ran.
    parts.push(format!("[{current}]anull[out]"));

    vec![
        "-y".to_string(),
        "-i".to_string(),
        input.to_string_lossy().into_owned(),
        "-filter_complex".to_string(),
        parts.join(";"),
        "-map".to_string(),
        "[out]".to_string(),
        output.to_string_lossy().into_owned(),
    ]
}

/// Runs `ffmpeg` to process `input` into `output` per `options`, then re-applies `input`'s
/// original tags and artwork onto `output` (R14.5 — `ffmpeg`'s filter graph re-encodes, which
/// drops them).
pub fn apply_trim_cut(input: &Path, output: &Path, options: &TrimCutOptions) -> Result<()> {
    let original = audio_tagger::read_metadata(input)?;
    let artwork = extract_artwork(input);

    let ffmpeg = find_ffmpeg()
        .ok_or_else(|| AppError::Transcode("ffmpeg.exe not found (bundled assets/bin/ or PATH)".into()))?;
    let duration_ms = (original.duration_seconds * 1000.0).round() as u64;
    let args = build_ffmpeg_args(input, output, options, duration_ms);

    let result = Command::new(&ffmpeg)
        .args(&args)
        .output()
        .map_err(|e| AppError::Transcode(format!("failed to run ffmpeg: {e}")))?;
    if !result.status.success() {
        return Err(AppError::Transcode(format!(
            "ffmpeg exited with {:?}: {}",
            result.status.code(),
            String::from_utf8_lossy(&result.stderr)
        )));
    }

    audio_tagger::write_tags(
        output,
        &TagFields {
            title: &original.title,
            artist: &original.artist,
            album: &original.album,
            track_number: &original.track_number,
            disc_number: "",
            year: &original.year,
            genre: &original.genre,
            lyrics: original.lyrics.as_deref(),
            artwork_jpeg: artwork.as_deref(),
        },
    )
}

fn extract_artwork(path: &Path) -> Option<Vec<u8>> {
    use lofty::file::TaggedFileExt;
    use lofty::probe::Probe;
    let tagged_file = Probe::open(path).ok()?.read().ok()?;
    let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag())?;
    let picture = tag.pictures().first()?;
    crate::artwork::encode_baseline_jpeg(picture.data()).ok()
}

/// Generates up to `target_points` peak-amplitude samples (`0.0..=1.0`) for the waveform
/// display (R14.1). Tries `symphonia` first; if that fails, falls back to decoding via bundled
/// `ffmpeg`.
pub fn generate_waveform(path: &Path, target_points: usize) -> Result<Vec<f32>> {
    match generate_waveform_symphonia(path, target_points) {
        Ok(peaks) => Ok(peaks),
        Err(symphonia_err) => generate_waveform_ffmpeg(path, target_points).map_err(|ffmpeg_err| {
            AppError::Tag(format!(
                "waveform generation failed (symphonia: {symphonia_err}; ffmpeg fallback: {ffmpeg_err})"
            ))
        }),
    }
}

fn generate_waveform_symphonia(path: &Path, target_points: usize) -> Result<Vec<f32>> {
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| AppError::Tag(format!("waveform probe failed: {e}")))?;
    let mut format = probed.format;
    let track_id = format
        .default_track()
        .ok_or_else(|| AppError::Tag("no audio track found".into()))?
        .id;
    let codec_params = format.tracks().iter().find(|t| t.id == track_id).unwrap().codec_params.clone();
    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|e| AppError::Tag(format!("no decoder available: {e}")))?;

    let mut interleaved: Vec<f32> = Vec::new();
    let mut channel_count = 1usize;

    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }
        let decoded: AudioBufferRef = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(_) => continue,
        };
        channel_count = decoded.spec().channels.count().max(1);
        let mut sample_buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
        sample_buf.copy_interleaved_ref(decoded);
        interleaved.extend_from_slice(sample_buf.samples());
    }

    if interleaved.is_empty() {
        return Err(AppError::Tag("no decodable audio packets".into()));
    }

    let mono: Vec<f32> = interleaved
        .chunks_exact(channel_count)
        .map(|frame| frame.iter().sum::<f32>() / channel_count as f32)
        .collect();

    Ok(downsample_to_peaks(&mono, target_points))
}

fn generate_waveform_ffmpeg(path: &Path, target_points: usize) -> Result<Vec<f32>> {
    let ffmpeg = find_ffmpeg()
        .ok_or_else(|| AppError::Transcode("ffmpeg.exe not found (bundled assets/bin/ or PATH)".into()))?;
    let output = Command::new(&ffmpeg)
        .args([
            "-i",
            &path.to_string_lossy(),
            "-f",
            "f32le",
            "-ac",
            "1",
            "-ar",
            "44100",
            "-",
        ])
        .output()
        .map_err(|e| AppError::Transcode(format!("failed to run ffmpeg: {e}")))?;
    if !output.status.success() {
        return Err(AppError::Transcode(String::from_utf8_lossy(&output.stderr).into_owned()));
    }
    let mono: Vec<f32> = output
        .stdout
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    Ok(downsample_to_peaks(&mono, target_points))
}

fn downsample_to_peaks(mono: &[f32], target_points: usize) -> Vec<f32> {
    if target_points == 0 || mono.is_empty() {
        return Vec::new();
    }
    let bucket_size = (mono.len() as f64 / target_points as f64).ceil().max(1.0) as usize;
    mono.chunks(bucket_size)
        .map(|bucket| bucket.iter().fold(0.0f32, |acc, &s| acc.max(s.abs())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(mode: TrimCutMode) -> TrimCutOptions {
        TrimCutOptions {
            mode,
            start_ms: 1_000,
            end_ms: 3_000,
            channel_mode: ChannelMode::None,
            fade_in_ms: 0,
            fade_out_ms: 0,
            gain_db: 0.0,
        }
    }

    #[test]
    fn trim_builds_a_single_atrim_stage_mapped_to_out() {
        let args = build_ffmpeg_args(Path::new("in.flac"), Path::new("out.flac"), &opts(TrimCutMode::Trim), 10_000);
        let graph = args.iter().find(|a| a.contains("atrim")).unwrap();
        assert!(graph.contains("atrim=start=1.000:end=3.000"));
        assert!(!graph.contains("concat"));
        assert!(args.contains(&"[out]".to_string()));
    }

    #[test]
    fn cut_builds_a_split_trim_concat_graph() {
        let args = build_ffmpeg_args(Path::new("in.flac"), Path::new("out.flac"), &opts(TrimCutMode::Cut), 10_000);
        let graph = args.iter().find(|a| a.contains("atrim")).unwrap();
        assert!(graph.contains("asplit=2"));
        assert!(graph.contains("atrim=start=0:end=1.000"));
        assert!(graph.contains("atrim=start=3.000:end=10.000"));
        assert!(graph.contains("concat=n=2:v=0:a=1"));
    }

    #[test]
    fn channel_modes_each_produce_a_distinct_filter() {
        for (mode, expect) in [
            (ChannelMode::SwapLr, "c0=c1|c1=c0"),
            (ChannelMode::Mono, "channel_layouts=mono"),
            (ChannelMode::Stereo, "channel_layouts=stereo"),
            (ChannelMode::IsolateLeft, "c0=c0|c1=c0"),
            (ChannelMode::IsolateRight, "c0=c1|c1=c1"),
            (ChannelMode::MuteLeft, "c0=0|c1=c1"),
            (ChannelMode::MuteRight, "c0=c0|c1=0"),
            (ChannelMode::PhaseFlip, "-val(0)"),
        ] {
            let mut o = opts(TrimCutMode::Trim);
            o.channel_mode = mode;
            let args = build_ffmpeg_args(Path::new("in.flac"), Path::new("out.flac"), &o, 10_000);
            let graph = args.iter().find(|a| a.contains("atrim")).unwrap();
            assert!(graph.contains(expect), "{mode:?}: {graph}");
        }
    }

    #[test]
    fn fades_and_gain_are_chained_in_order() {
        let mut o = opts(TrimCutMode::Trim);
        o.fade_in_ms = 500;
        o.fade_out_ms = 500;
        o.gain_db = -3.0;
        let args = build_ffmpeg_args(Path::new("in.flac"), Path::new("out.flac"), &o, 10_000);
        let graph = args.iter().find(|a| a.contains("atrim")).unwrap();
        assert!(graph.contains("afade=t=in:st=0:d=0.500"));
        // Selected duration is 2.0s (3.000 - 1.000); fade-out starts at 1.5s into the selection.
        assert!(graph.contains("afade=t=out:st=1.500:d=0.500"));
        assert!(graph.contains("volume=-3.00dB"));
    }

    #[test]
    fn no_optional_stages_still_maps_a_final_out_label() {
        let args = build_ffmpeg_args(Path::new("in.flac"), Path::new("out.flac"), &opts(TrimCutMode::Trim), 10_000);
        let graph = args.iter().find(|a| a.contains("atrim")).unwrap();
        assert!(graph.ends_with("[out]"));
    }

    #[test]
    fn downsample_to_peaks_returns_the_requested_point_count_and_captures_peaks() {
        let mono = vec![0.1, 0.9, -0.2, 0.05, 0.05, 0.05, 0.05, 0.3, -0.8, 0.1];
        let peaks = downsample_to_peaks(&mono, 5);
        assert_eq!(peaks.len(), 5);
        assert!((peaks[0] - 0.9).abs() < 1e-6); // bucket [0.1, 0.9] -> 0.9
    }

    #[test]
    fn generate_waveform_decodes_a_real_minimal_wav_via_symphonia() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tone.wav");
        write_minimal_wav(&path);

        let peaks = generate_waveform(&path, 8).unwrap();
        assert_eq!(peaks.len(), 8);
        assert!(peaks.iter().any(|&p| p > 0.0), "expected some non-silent peaks: {peaks:?}");
    }

    /// A minimal, valid 16-bit mono PCM WAV: a few cycles of a simple triangle wave, enough for
    /// `symphonia` to decode real non-zero samples.
    fn write_minimal_wav(path: &Path) {
        let sample_rate: u32 = 8_000;
        let samples: Vec<i16> = (0..400)
            .map(|i| {
                let t = i % 40;
                let v = if t < 20 { t - 10 } else { 30 - t };
                (v * 1000) as i16
            })
            .collect();

        let data_len = (samples.len() * 2) as u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
        bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
        bytes.extend_from_slice(&2u16.to_le_bytes()); // block align
        bytes.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            bytes.extend_from_slice(&s.to_le_bytes());
        }
        std::fs::write(path, bytes).unwrap();
    }
}
