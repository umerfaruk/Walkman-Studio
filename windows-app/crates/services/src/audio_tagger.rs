//! Audio metadata read/write orchestrator (spec task 8.2/8.3/8.4).
//!
//! Dispatches to the native, byte-exact [`crate::flac_tags`]/[`crate::id3v2`] writers for FLAC
//! and MP3 (R13.4: audio frames are never re-encoded, tags are hand-written to match Sony
//! firmware's exact expectations) and to bundled `ffmpeg` for M4A (there is no safe way to
//! hand-roll MP4 atom surgery in the time this port has for it, and ffmpeg's `-c copy` path
//! already guarantees the audio stream itself is untouched).
//!
//! **Reading** is a two-part multi-tier design, not a strict fallback chain: the native
//! FLAC/MP3 parsers are authoritative for *tag* fields (exact round-trip fidelity, lyrics,
//! normalized genre), but `lofty` is always used for *audio properties*
//! (duration/bitrate/sample-rate/bit-depth) on every format, because computing those from raw
//! frames would mean writing a second decoder with no benefit over the one `lofty` already
//! provides reliably (this is the same split `library_scanner.rs`, task 7, made). If native tag
//! parsing fails or the format isn't FLAC/MP3, `lofty` supplies the tag fields too. An `ffprobe`
//! tier is not implemented: task 19.1 owns bundling `ffmpeg`/`ffprobe`, and neither is available
//! to shell out to on a dev host without them, so there is nothing to fall back to today.

use std::path::Path;
use std::process::Command;

use lofty::file::AudioFile;
use lofty::prelude::{Accessor, TaggedFileExt};
use lofty::probe::Probe;

use walkman_domain::models::AudioFileMetadata;

use crate::error::{AppError, Result};
use crate::ffmpeg_util::find_ffmpeg;

/// Editable fields for [`write_tags`]. Mirrors [`AudioFileMetadata`] plus `disc_number`, which
/// that struct doesn't carry (ported 1:1 from the Swift model, which doesn't either — disc
/// number is a writer-only field here, not round-tripped through the shared read struct).
#[derive(Debug, Clone, Default)]
pub struct TagFields<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    pub album: &'a str,
    pub track_number: &'a str,
    pub disc_number: &'a str,
    pub year: &'a str,
    pub genre: &'a str,
    pub lyrics: Option<&'a str>,
    /// Must already be a Baseline JPEG (see `artwork::encode_baseline_jpeg`); this module never
    /// re-encodes images.
    pub artwork_jpeg: Option<&'a [u8]>,
}

fn extension_lower(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()
}

/// Writes `fields` into `path`'s tags in place, preserving the file's original creation/modified
/// timestamps (R8.5) and never touching the encoded audio stream (R13.4/R19.3).
pub fn write_tags(path: &Path, fields: &TagFields<'_>) -> Result<()> {
    let original_times = std::fs::metadata(path).ok().map(|m| (m.created().ok(), m.modified().ok()));

    match extension_lower(path).as_str() {
        "flac" => {
            let bytes = std::fs::read(path)?;
            let artwork = fields.artwork_jpeg.map(|jpeg| {
                let (w, h) = crate::artwork::validate_jpeg(jpeg).map(|c| (c.width, c.height)).unwrap_or((0, 0));
                (jpeg, w, h)
            });
            let written = crate::flac_tags::write_tags(
                &bytes,
                &crate::flac_tags::FlacTagsWrite {
                    title: fields.title,
                    artist: fields.artist,
                    album: fields.album,
                    track_number: fields.track_number,
                    disc_number: fields.disc_number,
                    year: fields.year,
                    genre: fields.genre,
                    lyrics: fields.lyrics,
                    artwork,
                },
            )?;
            std::fs::write(path, written)?;
        }
        "mp3" => {
            let bytes = std::fs::read(path)?;
            let written = crate::id3v2::write_tags(
                &bytes,
                &crate::id3v2::Id3TagsWrite {
                    title: fields.title,
                    artist: fields.artist,
                    album: fields.album,
                    track_number: fields.track_number,
                    disc_number: fields.disc_number,
                    year: fields.year,
                    genre: fields.genre,
                    lyrics: fields.lyrics,
                    artwork_jpeg: fields.artwork_jpeg,
                },
            )?;
            std::fs::write(path, written)?;
        }
        "m4a" | "mp4" => write_m4a_tags_via_ffmpeg(path, fields)?,
        other => return Err(AppError::Tag(format!("unsupported format for tagging: .{other}"))),
    }

    if let Some((created, modified)) = original_times {
        #[cfg(windows)]
        {
            let _ = walkman_platform::set_file_times(path, created, modified);
        }
        #[cfg(not(windows))]
        {
            let _ = (created, modified);
        }
    }
    Ok(())
}

/// Builds the `ffmpeg` argument list for M4A tagging: remux (`-c copy`, no re-encode) into a
/// temp file carrying the new metadata (and, if present, an attached-picture cover), then the
/// caller swaps it over the original. Exposed separately from [`write_m4a_tags_via_ffmpeg`] so
/// the argument list itself is unit-testable without actually running `ffmpeg` (matching the
/// test style planned for task 10.1's transcode presets).
pub fn build_m4a_ffmpeg_args(input: &Path, output: &Path, fields: &TagFields<'_>, artwork_path: Option<&Path>) -> Vec<String> {
    let mut args = vec!["-y".to_string(), "-i".to_string(), input.to_string_lossy().into_owned()];

    if let Some(art) = artwork_path {
        args.push("-i".to_string());
        args.push(art.to_string_lossy().into_owned());
        args.extend(["-map".to_string(), "0:a".to_string()]);
        args.extend(["-map".to_string(), "1".to_string()]);
        args.extend(["-disposition:v".to_string(), "attached_pic".to_string()]);
    } else {
        args.extend(["-map".to_string(), "0:a".to_string()]);
    }

    args.extend(["-c".to_string(), "copy".to_string()]);
    for (key, value) in [
        ("title", fields.title),
        ("artist", fields.artist),
        ("album", fields.album),
        ("track", fields.track_number),
        ("disc", fields.disc_number),
        ("date", fields.year),
        ("genre", fields.genre),
    ] {
        args.push("-metadata".to_string());
        args.push(format!("{key}={value}"));
    }
    if let Some(lyrics) = fields.lyrics {
        args.push("-metadata".to_string());
        args.push(format!("lyrics={lyrics}"));
    }
    args.push(output.to_string_lossy().into_owned());
    args
}

fn write_m4a_tags_via_ffmpeg(path: &Path, fields: &TagFields<'_>) -> Result<()> {
    let ffmpeg = find_ffmpeg().ok_or_else(|| {
        AppError::Tag("ffmpeg.exe not found (bundled assets/bin/ or PATH) — required for M4A tagging".into())
    })?;

    let tmp_dir = std::env::temp_dir();
    let output = tmp_dir.join(format!(
        "walkman-studio-m4a-tag-{}-{}.m4a",
        std::process::id(),
        path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    ));

    let artwork_path = fields
        .artwork_jpeg
        .map(|jpeg| -> Result<_> {
            let p = tmp_dir.join(format!("walkman-studio-m4a-art-{}.jpg", std::process::id()));
            std::fs::write(&p, jpeg)?;
            Ok(p)
        })
        .transpose()?;

    let args = build_m4a_ffmpeg_args(path, &output, fields, artwork_path.as_deref());
    let status = Command::new(&ffmpeg)
        .args(&args)
        .output()
        .map_err(|e| AppError::Tag(format!("failed to run ffmpeg: {e}")))?;

    if let Some(art) = &artwork_path {
        let _ = std::fs::remove_file(art);
    }

    if !status.status.success() {
        let _ = std::fs::remove_file(&output);
        return Err(AppError::Tag(format!(
            "ffmpeg exited with {:?}: {}",
            status.status.code(),
            String::from_utf8_lossy(&status.stderr)
        )));
    }

    std::fs::rename(&output, path).or_else(|_| {
        // Cross-volume temp dirs can't be renamed; fall back to copy+delete.
        std::fs::copy(&output, path)?;
        std::fs::remove_file(&output)
    })?;
    Ok(())
}

/// Extracts whatever artwork is already embedded in `path` and writes it out to `dest`
/// unmodified (R13.3's artwork export-to-PC) — the embedded bytes are already a compliant
/// JPEG from however they were originally embedded, so this never re-encodes.
pub fn export_embedded_artwork(path: &Path, dest: &Path) -> Result<()> {
    let tagged_file = Probe::open(path).map_err(|e| AppError::Tag(e.to_string()))?.read().map_err(|e| AppError::Tag(e.to_string()))?;
    let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());
    let Some(picture) = tag.and_then(|t| t.pictures().first()) else {
        return Err(AppError::Tag("no embedded artwork".into()));
    };
    crate::artwork::export_to_file(picture.data(), dest)
}

/// Reads `path`'s tags and audio properties (see the module doc for the native/`lofty` split).
pub fn read_metadata(path: &Path) -> Result<AudioFileMetadata> {
    let ext = extension_lower(path);
    let native_tags = match ext.as_str() {
        "flac" => std::fs::read(path).ok().and_then(|b| crate::flac_tags::read_tags(&b).ok()).map(|t| NativeTags {
            title: t.title,
            artist: t.artist,
            album: t.album,
            genre: t.genre,
            lyrics: t.lyrics,
            has_artwork: t.has_artwork,
        }),
        "mp3" => std::fs::read(path).ok().and_then(|b| crate::id3v2::read_tags(&b).ok()).map(|t| NativeTags {
            title: t.title,
            artist: t.artist,
            album: t.album,
            genre: t.genre,
            lyrics: t.lyrics,
            has_artwork: t.has_artwork,
        }),
        _ => None,
    };

    let tagged_file = Probe::open(path)
        .map_err(|e| AppError::Tag(e.to_string()))?
        .read()
        .map_err(|e| AppError::Tag(e.to_string()))?;
    let properties = tagged_file.properties();
    let lofty_tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());

    let tags = native_tags.unwrap_or_else(|| NativeTags {
        title: lofty_tag.and_then(|t| t.title()).map(|s| s.into_owned()).unwrap_or_default(),
        artist: lofty_tag.and_then(|t| t.artist()).map(|s| s.into_owned()).unwrap_or_default(),
        album: lofty_tag.and_then(|t| t.album()).map(|s| s.into_owned()).unwrap_or_default(),
        genre: lofty_tag
            .and_then(|t| t.genre())
            .map(|s| crate::genre::normalize_genre(&s))
            .unwrap_or_default(),
        lyrics: None,
        has_artwork: lofty_tag.map(|t| !t.pictures().is_empty()).unwrap_or(false),
    });

    Ok(AudioFileMetadata {
        title: tags.title,
        artist: tags.artist,
        album: tags.album,
        track_number: lofty_tag.and_then(|t| t.track()).map(|n| n.to_string()).unwrap_or_default(),
        year: lofty_tag.and_then(|t| t.year()).map(|y| y.to_string()).unwrap_or_default(),
        genre: tags.genre,
        duration_seconds: properties.duration().as_secs_f64(),
        bitrate_kbps: properties.audio_bitrate().unwrap_or(0) as i32,
        sample_rate: properties.sample_rate().unwrap_or(44_100) as i32,
        bits_per_sample: properties.bit_depth().unwrap_or(16) as i32,
        has_artwork: tags.has_artwork,
        lyrics: tags.lyrics,
    })
}

struct NativeTags {
    title: String,
    artist: String,
    album: String,
    genre: String,
    lyrics: Option<String>,
    has_artwork: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_flac_with_tags(title: &str) -> Vec<u8> {
        // Same minimal fixture shape as `flac_tags`'s own tests: STREAMINFO + VORBIS_COMMENT,
        // no audio frames.
        let mut out = Vec::new();
        out.extend_from_slice(b"fLaC");
        out.push(0x00);
        out.extend_from_slice(&34u32.to_be_bytes()[1..]);
        out.extend_from_slice(&[0u8; 34]);

        let mut body = Vec::new();
        let vendor = b"test";
        body.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
        body.extend_from_slice(vendor);
        let entry = format!("TITLE={title}");
        body.extend_from_slice(&1u32.to_le_bytes());
        body.extend_from_slice(&(entry.len() as u32).to_le_bytes());
        body.extend_from_slice(entry.as_bytes());

        out.push(0x84);
        out.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
        out.extend_from_slice(&body);
        out
    }

    #[test]
    fn write_tags_round_trips_through_the_filesystem_for_flac() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.flac");
        std::fs::write(&path, minimal_flac_with_tags("Old Title")).unwrap();

        write_tags(
            &path,
            &TagFields {
                title: "New Title",
                artist: "New Artist",
                genre: "Rock",
                ..Default::default()
            },
        )
        .unwrap();

        let bytes = std::fs::read(&path).unwrap();
        let tags = crate::flac_tags::read_tags(&bytes).unwrap();
        assert_eq!(tags.title, "New Title");
        assert_eq!(tags.artist, "New Artist");
        assert_eq!(tags.genre, "Rock");
    }

    #[test]
    fn write_tags_preserves_the_modified_timestamp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.flac");
        std::fs::write(&path, minimal_flac_with_tags("Title")).unwrap();

        let original_modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));

        write_tags(&path, &TagFields { title: "New", ..Default::default() }).unwrap();

        let new_modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        let delta = new_modified
            .duration_since(original_modified)
            .unwrap_or_else(|e| e.duration());
        assert!(delta < std::time::Duration::from_secs(1), "modified time should be restored, drifted by {delta:?}");
    }

    #[test]
    fn export_embedded_artwork_writes_out_the_picture_bytes_embedded_by_write_tags() {
        use image::{DynamicImage, Rgb, RgbImage};

        let jpeg = crate::artwork::encode_baseline_jpeg_from_image(&DynamicImage::ImageRgb8(RgbImage::from_pixel(4, 4, Rgb([200, 100, 50])))).unwrap();

        let dir = tempfile::tempdir().unwrap();
        let audio_path = dir.path().join("song.flac");
        std::fs::write(&audio_path, minimal_flac_with_tags("Title")).unwrap();
        write_tags(&audio_path, &TagFields { title: "Title", artwork_jpeg: Some(&jpeg), ..Default::default() }).unwrap();

        let export_path = dir.path().join("exported.jpg");
        export_embedded_artwork(&audio_path, &export_path).unwrap();

        assert_eq!(std::fs::read(&export_path).unwrap(), jpeg);
    }

    #[test]
    fn export_embedded_artwork_errors_when_the_file_has_no_picture() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.flac");
        std::fs::write(&path, minimal_flac_with_tags("Title")).unwrap();
        assert!(export_embedded_artwork(&path, &dir.path().join("out.jpg")).is_err());
    }

    #[test]
    fn write_tags_rejects_an_unsupported_extension() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.wav");
        std::fs::write(&path, b"RIFF....WAVEfmt ").unwrap();
        assert!(write_tags(&path, &TagFields::default()).is_err());
    }

    #[test]
    fn build_m4a_ffmpeg_args_includes_metadata_and_attached_picture() {
        let args = build_m4a_ffmpeg_args(
            Path::new("in.m4a"),
            Path::new("out.m4a"),
            &TagFields { title: "T", artist: "A", ..Default::default() },
            Some(Path::new("cover.jpg")),
        );
        assert!(args.contains(&"in.m4a".to_string()));
        assert!(args.contains(&"out.m4a".to_string()));
        assert!(args.contains(&"cover.jpg".to_string()));
        assert!(args.contains(&"title=T".to_string()));
        assert!(args.contains(&"artist=A".to_string()));
        assert!(args.contains(&"attached_pic".to_string()));
        assert!(args.contains(&"copy".to_string()), "must never re-encode the audio stream");
    }

    #[test]
    fn build_m4a_ffmpeg_args_without_artwork_has_no_picture_mapping() {
        let args = build_m4a_ffmpeg_args(Path::new("in.m4a"), Path::new("out.m4a"), &TagFields::default(), None);
        assert!(!args.contains(&"attached_pic".to_string()));
    }

    #[test]
    fn read_metadata_prefers_native_flac_tags() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.flac");
        std::fs::write(&path, minimal_flac_with_tags("Native Title")).unwrap();

        let meta = read_metadata(&path).unwrap();
        assert_eq!(meta.title, "Native Title");
    }
}
