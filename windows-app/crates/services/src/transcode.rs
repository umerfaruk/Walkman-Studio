//! Transcoding presets and the `ffmpeg` argument builder (spec task 10.1).
//!
//! macOS source: `TranscodeService`'s preset table. Every preset but [`Preset::CopyRaw`] runs
//! through bundled `ffmpeg` (`ffmpeg_util::find_ffmpeg`, bundled-first discovery, task 8.3);
//! `CopyRaw` is a plain filesystem copy with no `ffmpeg` invocation at all, handled by the
//! caller (`transfer_queue.rs`, task 10.2) rather than here.

use std::path::Path;
use std::process::Command;

use crate::error::{AppError, Result};
use crate::ffmpeg_util::find_ffmpeg;

/// Every transcode preset the engine recognizes (R9.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Preset {
    /// Bit-perfect remux (`-c copy`): no re-encode, tags are sanitized separately by the
    /// tagger. Matches R9.3's "Bit-Perfect Copy (tag sanitize, no re-encode)".
    Copy,
    /// Bit-perfect remux plus an artwork-optimization pass (still no audio re-encode).
    CopyOptimize,
    /// A plain filesystem copy, bypassing `ffmpeg` entirely — not an `ffmpeg` preset at all; see
    /// the module doc.
    CopyRaw,
    Mp3_128,
    Mp3_192,
    Mp3_256,
    Mp3_320,
    Mp3V0,
    Aac128,
    Aac256,
    Aac320,
    Flac16_44_1,
    Flac24_48,
    Flac24_96,
    Flac24_192,
    Alac16,
    Wav16,
}

impl Preset {
    /// File extension the preset's output should use.
    pub fn output_extension(self) -> &'static str {
        match self {
            Preset::Copy | Preset::CopyOptimize | Preset::CopyRaw => "", // caller keeps the input's own extension
            Preset::Mp3_128 | Preset::Mp3_192 | Preset::Mp3_256 | Preset::Mp3_320 | Preset::Mp3V0 => "mp3",
            Preset::Aac128 | Preset::Aac256 | Preset::Aac320 => "m4a",
            Preset::Flac16_44_1 | Preset::Flac24_48 | Preset::Flac24_96 | Preset::Flac24_192 => "flac",
            Preset::Alac16 => "m4a",
            Preset::Wav16 => "wav",
        }
    }

    /// `true` when this preset never re-encodes the audio stream (so e.g. loudness
    /// normalization — which requires decoding — can't be combined with it).
    pub fn is_bit_perfect(self) -> bool {
        matches!(self, Preset::Copy | Preset::CopyOptimize | Preset::CopyRaw)
    }

    /// `false` only for [`Preset::CopyRaw`], which never invokes `ffmpeg` at all.
    pub fn requires_ffmpeg(self) -> bool {
        !matches!(self, Preset::CopyRaw)
    }
}

/// Builds the `ffmpeg` argument list for `preset`, converting `input` to `output`. `loudnorm`
/// requests the loudness-normalization filter (R9.3); it is silently ignored for a
/// [`Preset::is_bit_perfect`] preset, since `-c copy` and an audio filter are mutually exclusive
/// in `ffmpeg` (a filter needs to decode first). Every preset strips any video/attached-picture
/// stream (`-vn`) so cover art embedded as a video stream never ends up in a converted audio
/// file.
pub fn build_ffmpeg_args(preset: Preset, input: &Path, output: &Path, loudnorm: bool) -> Vec<String> {
    let mut args = vec!["-y".to_string(), "-i".to_string(), input.to_string_lossy().into_owned(), "-vn".to_string()];

    if loudnorm && !preset.is_bit_perfect() {
        args.extend(["-af".to_string(), "loudnorm".to_string()]);
    }

    match preset {
        Preset::Copy | Preset::CopyOptimize => {
            args.extend(["-c".to_string(), "copy".to_string()]);
        }
        Preset::CopyRaw => unreachable!("CopyRaw never builds ffmpeg args; see requires_ffmpeg()"),
        Preset::Mp3_128 => args.extend(codec_bitrate("libmp3lame", "128k")),
        Preset::Mp3_192 => args.extend(codec_bitrate("libmp3lame", "192k")),
        Preset::Mp3_256 => args.extend(codec_bitrate("libmp3lame", "256k")),
        Preset::Mp3_320 => args.extend(codec_bitrate("libmp3lame", "320k")),
        Preset::Mp3V0 => args.extend([
            "-c:a".to_string(),
            "libmp3lame".to_string(),
            "-q:a".to_string(),
            "0".to_string(),
        ]),
        Preset::Aac128 => args.extend(codec_bitrate("aac", "128k")),
        Preset::Aac256 => args.extend(codec_bitrate("aac", "256k")),
        Preset::Aac320 => args.extend(codec_bitrate("aac", "320k")),
        Preset::Flac16_44_1 => args.extend(flac_args("s16", "44100")),
        Preset::Flac24_48 => args.extend(flac_args("s32", "48000")),
        Preset::Flac24_96 => args.extend(flac_args("s32", "96000")),
        Preset::Flac24_192 => args.extend(flac_args("s32", "192000")),
        Preset::Alac16 => args.extend([
            "-c:a".to_string(),
            "alac".to_string(),
            "-sample_fmt".to_string(),
            "s16p".to_string(),
        ]),
        Preset::Wav16 => args.extend(["-c:a".to_string(), "pcm_s16le".to_string()]),
    }

    args.push(output.to_string_lossy().into_owned());
    args
}

fn codec_bitrate(codec: &str, bitrate: &str) -> Vec<String> {
    vec!["-c:a".to_string(), codec.to_string(), "-b:a".to_string(), bitrate.to_string()]
}

/// FLAC is encoded at 32-bit sample format with `-bits_per_raw_sample 24` for the 24-bit
/// presets, since `ffmpeg`'s FLAC encoder has no native 24-bit sample format of its own.
fn flac_args(sample_fmt: &str, sample_rate: &str) -> Vec<String> {
    let mut args = vec![
        "-c:a".to_string(),
        "flac".to_string(),
        "-sample_fmt".to_string(),
        sample_fmt.to_string(),
        "-ar".to_string(),
        sample_rate.to_string(),
    ];
    if sample_fmt == "s32" {
        args.extend(["-bits_per_raw_sample".to_string(), "24".to_string()]);
    }
    args
}

/// Runs bundled `ffmpeg` for `preset`, converting `input` to `output`. Returns the captured
/// stderr on failure (design.md: "run bundled ffmpeg, capture stderr"). Panics if called with
/// [`Preset::CopyRaw`] — check [`Preset::requires_ffmpeg`] first.
pub fn transcode(preset: Preset, input: &Path, output: &Path, loudnorm: bool) -> Result<()> {
    assert!(preset.requires_ffmpeg(), "CopyRaw must be handled as a filesystem copy, not transcode()");

    let ffmpeg = find_ffmpeg().ok_or_else(|| AppError::Transcode("ffmpeg.exe not found (bundled assets/bin/ or PATH)".into()))?;
    let args = build_ffmpeg_args(preset, input, output, loudnorm);

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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_preset_uses_c_copy_and_never_adds_loudnorm() {
        let args = build_ffmpeg_args(Preset::Copy, Path::new("in.flac"), Path::new("out.flac"), true);
        assert!(args.contains(&"copy".to_string()));
        assert!(!args.iter().any(|a| a == "loudnorm"));
        assert!(args.contains(&"-vn".to_string()));
    }

    #[test]
    fn mp3_320_sets_libmp3lame_and_bitrate() {
        let args = build_ffmpeg_args(Preset::Mp3_320, Path::new("in.flac"), Path::new("out.mp3"), false);
        assert!(args.contains(&"libmp3lame".to_string()));
        assert!(args.contains(&"320k".to_string()));
    }

    #[test]
    fn mp3_v0_uses_quality_flag_not_a_fixed_bitrate() {
        let args = build_ffmpeg_args(Preset::Mp3V0, Path::new("in.flac"), Path::new("out.mp3"), false);
        assert!(args.contains(&"-q:a".to_string()));
        assert!(args.contains(&"0".to_string()));
        assert!(!args.contains(&"-b:a".to_string()));
    }

    #[test]
    fn flac_24_bit_presets_set_bits_per_raw_sample_24() {
        for (preset, rate) in [
            (Preset::Flac24_48, "48000"),
            (Preset::Flac24_96, "96000"),
            (Preset::Flac24_192, "192000"),
        ] {
            let args = build_ffmpeg_args(preset, Path::new("in.flac"), Path::new("out.flac"), false);
            assert!(args.contains(&"-bits_per_raw_sample".to_string()), "{preset:?}");
            assert!(args.contains(&"24".to_string()), "{preset:?}");
            assert!(args.contains(&rate.to_string()), "{preset:?}");
        }
    }

    #[test]
    fn flac_16_44_1_uses_s16_and_does_not_set_bits_per_raw_sample() {
        let args = build_ffmpeg_args(Preset::Flac16_44_1, Path::new("in.flac"), Path::new("out.flac"), false);
        assert!(args.contains(&"s16".to_string()));
        assert!(args.contains(&"44100".to_string()));
        assert!(!args.contains(&"-bits_per_raw_sample".to_string()));
    }

    #[test]
    fn loudnorm_is_applied_for_a_real_encode_preset_when_requested() {
        let args = build_ffmpeg_args(Preset::Mp3_320, Path::new("in.flac"), Path::new("out.mp3"), true);
        assert!(args.contains(&"loudnorm".to_string()));
        assert!(args.contains(&"-af".to_string()));
    }

    #[test]
    fn wav16_and_alac16_select_the_right_codec() {
        let wav = build_ffmpeg_args(Preset::Wav16, Path::new("in.flac"), Path::new("out.wav"), false);
        assert!(wav.contains(&"pcm_s16le".to_string()));
        let alac = build_ffmpeg_args(Preset::Alac16, Path::new("in.flac"), Path::new("out.m4a"), false);
        assert!(alac.contains(&"alac".to_string()));
    }

    #[test]
    fn copy_raw_never_requires_ffmpeg_and_every_other_preset_does() {
        assert!(!Preset::CopyRaw.requires_ffmpeg());
        for preset in [
            Preset::Copy,
            Preset::CopyOptimize,
            Preset::Mp3_128,
            Preset::Flac16_44_1,
            Preset::Alac16,
            Preset::Wav16,
        ] {
            assert!(preset.requires_ffmpeg(), "{preset:?}");
        }
    }

    #[test]
    #[should_panic(expected = "CopyRaw")]
    fn transcode_panics_if_called_with_copy_raw() {
        let _ = transcode(Preset::CopyRaw, Path::new("in"), Path::new("out"), false);
    }

    #[test]
    fn output_extensions_match_the_preset_family() {
        assert_eq!(Preset::Mp3_320.output_extension(), "mp3");
        assert_eq!(Preset::Aac256.output_extension(), "m4a");
        assert_eq!(Preset::Flac24_96.output_extension(), "flac");
        assert_eq!(Preset::Alac16.output_extension(), "m4a");
        assert_eq!(Preset::Wav16.output_extension(), "wav");
        assert_eq!(Preset::Copy.output_extension(), "");
    }
}
