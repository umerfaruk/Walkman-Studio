//! Baseline JPEG artwork engine (spec task 8.1).
//!
//! Sony Walkman firmware requires cover art to be a Baseline (non-progressive) JPEG, so every
//! image this app embeds is re-encoded through [`encode_baseline_jpeg`] regardless of its
//! source format or color model. [`validate_jpeg`] independently hand-parses the JPEG marker
//! stream (rather than relying on the `image` crate's decoder, which silently normalizes
//! CMYK/progressive inputs to RGB on decode) so compliance badges reflect what the *original*
//! bytes actually were — needed when a user drops in art we didn't encode ourselves.

use std::io::Cursor;
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView};

use crate::error::{AppError, Result};

/// Longest edge Walkman firmware accepts for cover art (R13.1).
pub const MAX_DIMENSION: u32 = 800;
/// Quality passed to the baseline JPEG encoder (R13.1: "~0.92").
pub const QUALITY: u8 = 92;

/// The color model a JPEG's `SOFn` marker declares via its component count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorModel {
    Grayscale,
    /// 3 components: YCbCr in a normal RGB-derived JPEG.
    YCbCrOrRgb,
    /// 4 components: CMYK or YCCK (Adobe), both non-compliant for Walkman artwork.
    CmykOrYcck,
    /// A component count we don't recognize.
    Other(u8),
}

/// Compliance facts extracted directly from a JPEG byte stream's markers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtworkCompliance {
    pub width: u32,
    pub height: u32,
    pub is_progressive: bool,
    pub color_model: ColorModel,
}

impl ArtworkCompliance {
    pub fn exceeds_max_dimension(&self) -> bool {
        self.width > MAX_DIMENSION || self.height > MAX_DIMENSION
    }

    /// Fully compliant: baseline, RGB/YCbCr, within the size cap.
    pub fn is_walkman_ready(&self) -> bool {
        !self.is_progressive
            && !self.exceeds_max_dimension()
            && matches!(self.color_model, ColorModel::YCbCrOrRgb | ColorModel::Grayscale)
    }

    /// Short badge strings for the UI (R13.2), most-important first. Empty when fully compliant
    /// and square-cornered at the cap, in which case the caller shows the single "Walkman Ready"
    /// badge via [`Self::is_walkman_ready`] instead.
    pub fn badges(&self) -> Vec<String> {
        let mut badges = Vec::new();
        if self.is_walkman_ready() {
            badges.push(format!("{}×{} • Walkman Ready", self.width, self.height));
            return badges;
        }
        if self.exceeds_max_dimension() {
            badges.push("Exceeds 800px".to_string());
        }
        if self.is_progressive {
            badges.push("Progressive JPEG (Advisory)".to_string());
        }
        match self.color_model {
            ColorModel::CmykOrYcck => badges.push("CMYK Color (Advisory)".to_string()),
            ColorModel::Other(n) => badges.push(format!("Unusual Color Encoding ({n}-component)")),
            _ => {}
        }
        badges
    }
}

/// Hand-parses the JPEG marker stream up to (and including) the first `SOFn` marker, which
/// always precedes the entropy-coded scan data — so this never has to deal with byte-stuffing or
/// restart markers inside compressed data.
pub fn validate_jpeg(bytes: &[u8]) -> Result<ArtworkCompliance> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return Err(AppError::Tag("not a JPEG file (missing SOI marker)".into()));
    }

    let mut pos = 2usize;
    while pos + 1 < bytes.len() {
        if bytes[pos] != 0xFF {
            pos += 1;
            continue;
        }
        let marker = bytes[pos + 1];
        pos += 2;

        // Markers with no payload/length field.
        if marker == 0x01 || (0xD0..=0xD9).contains(&marker) {
            if marker == 0xD9 {
                break; // EOI with no SOF found.
            }
            continue;
        }
        if pos + 1 >= bytes.len() {
            break;
        }
        let length = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
        let is_sof = matches!(
            marker,
            0xC0 | 0xC1 | 0xC2 | 0xC3 | 0xC5 | 0xC6 | 0xC7 | 0xC9 | 0xCA | 0xCB | 0xCD | 0xCE | 0xCF
        );
        if is_sof {
            if pos + length > bytes.len() || length < 7 {
                return Err(AppError::Tag("truncated JPEG SOF marker".into()));
            }
            let payload = &bytes[pos + 2..pos + length];
            let height = u16::from_be_bytes([payload[1], payload[2]]) as u32;
            let width = u16::from_be_bytes([payload[3], payload[4]]) as u32;
            let num_components = payload[5];
            let color_model = match num_components {
                1 => ColorModel::Grayscale,
                3 => ColorModel::YCbCrOrRgb,
                4 => ColorModel::CmykOrYcck,
                n => ColorModel::Other(n),
            };
            return Ok(ArtworkCompliance {
                width,
                height,
                is_progressive: marker == 0xC2,
                color_model,
            });
        }
        pos += length;
    }

    Err(AppError::Tag("JPEG has no SOF marker (corrupt or not a real JPEG)".into()))
}

/// Input-size guard (R19.3) against a decompression-bomb-style artwork file (a tiny byte
/// stream whose header claims an enormous image) before any decode is attempted at all — a
/// legitimate cover image is at most a few MB, so this is generous, not a real-world limit.
/// `image::ImageReader`'s own `Limits::default()` (a 512MB decode-allocation cap) already
/// backs this up at the decode step itself; this is the cheaper, earlier rejection.
const MAX_ARTWORK_INPUT_BYTES: usize = 64 * 1024 * 1024;

/// Decodes `bytes` (any format `image` supports), resizes to fit within [`MAX_DIMENSION`] on the
/// longest edge (never upscales), strips alpha/CMYK by converting to RGB8, and re-encodes as a
/// Baseline JPEG at [`QUALITY`] (R13.1).
pub fn encode_baseline_jpeg(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.len() > MAX_ARTWORK_INPUT_BYTES {
        return Err(AppError::Tag(format!(
            "artwork input too large ({} bytes, max {MAX_ARTWORK_INPUT_BYTES})",
            bytes.len()
        )));
    }
    let img = image::load_from_memory(bytes).map_err(|e| AppError::Tag(e.to_string()))?;
    encode_baseline_jpeg_from_image(&img)
}

/// Same as [`encode_baseline_jpeg`] but takes an already-decoded image (e.g. a frame grabbed
/// from a different source than a file/byte buffer).
pub fn encode_baseline_jpeg_from_image(img: &DynamicImage) -> Result<Vec<u8>> {
    let resized = fit_within_max_dimension(img);
    let rgb = resized.to_rgb8();

    let mut out = Vec::new();
    {
        let mut encoder = JpegEncoder::new_with_quality(Cursor::new(&mut out), QUALITY);
        encoder
            .encode_image(&rgb)
            .map_err(|e| AppError::Tag(e.to_string()))?;
    }
    Ok(out)
}

fn fit_within_max_dimension(img: &DynamicImage) -> DynamicImage {
    let (w, h) = img.dimensions();
    if w <= MAX_DIMENSION && h <= MAX_DIMENSION {
        return img.clone();
    }
    let scale = f64::from(MAX_DIMENSION) / f64::from(w.max(h));
    let new_w = ((f64::from(w) * scale).round() as u32).max(1);
    let new_h = ((f64::from(h) * scale).round() as u32).max(1);
    img.resize(new_w, new_h, FilterType::Lanczos3)
}

/// Writes raw artwork bytes to `dest` on the PC (R13.3's "export to PC"). The native save-file
/// dialog that supplies `dest` lives in `platform::FileDialogs` (task 6.6); this is the plain
/// file write the UI layer calls once the user has picked a destination.
pub fn export_to_file(bytes: &[u8], dest: &Path) -> Result<()> {
    std::fs::write(dest, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn solid_image(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, Rgb([200, 100, 50])))
    }

    #[test]
    fn encodes_a_small_rgb_image_as_baseline_jpeg() {
        let img = solid_image(100, 50);
        let jpeg = encode_baseline_jpeg_from_image(&img).unwrap();
        let compliance = validate_jpeg(&jpeg).unwrap();
        assert_eq!(compliance.width, 100);
        assert_eq!(compliance.height, 50);
        assert!(!compliance.is_progressive);
        assert_eq!(compliance.color_model, ColorModel::YCbCrOrRgb);
        assert!(compliance.is_walkman_ready());
        assert_eq!(compliance.badges(), vec!["100×50 • Walkman Ready".to_string()]);
    }

    #[test]
    fn downscales_oversized_artwork_to_the_800px_cap() {
        let img = solid_image(1600, 800);
        let jpeg = encode_baseline_jpeg_from_image(&img).unwrap();
        let compliance = validate_jpeg(&jpeg).unwrap();
        assert_eq!(compliance.width, 800);
        assert_eq!(compliance.height, 400);
        assert!(!compliance.exceeds_max_dimension());
    }

    #[test]
    fn never_upscales_small_artwork() {
        let img = solid_image(32, 32);
        let jpeg = encode_baseline_jpeg_from_image(&img).unwrap();
        let compliance = validate_jpeg(&jpeg).unwrap();
        assert_eq!(compliance.width, 32);
        assert_eq!(compliance.height, 32);
    }

    #[test]
    fn validate_rejects_non_jpeg_bytes() {
        assert!(validate_jpeg(b"not a jpeg at all").is_err());
    }

    #[test]
    fn encode_baseline_jpeg_rejects_an_oversized_input_before_decoding() {
        // R19.3: a decompression-bomb-style input is rejected on byte length alone, long
        // before any decode attempt — this must return quickly, not allocate/hang.
        let huge = vec![0u8; MAX_ARTWORK_INPUT_BYTES + 1];
        let err = encode_baseline_jpeg(&huge).unwrap_err();
        assert!(err.to_string().contains("too large"));
    }

    #[test]
    fn encode_baseline_jpeg_rejects_a_png_claiming_an_implausible_decoded_size() {
        // A well-formed PNG header declaring a 60,000x60,000 image (3.6 billion pixels) but
        // with no real pixel data following it. `image::ImageReader`'s default `Limits`
        // (R19.3's "image decode bounds") caps decode allocation at 512MB, so this must
        // fail cleanly via `encode_baseline_jpeg`'s normal error path rather than
        // allocating ~14GB of RGBA and OOMing the process.
        let png = build_huge_png_header(60_000, 60_000);
        assert!(encode_baseline_jpeg(&png).is_err());
    }

    /// A minimal, structurally valid PNG `IHDR` chunk (8-byte signature + a correctly
    /// CRC'd `IHDR` declaring `width`x`height`, 8-bit RGBA) with no `IDAT`/`IEND` — enough
    /// for the PNG decoder to read the claimed dimensions and hit the allocation limit
    /// before it ever needs real pixel data.
    fn build_huge_png_header(width: u32, height: u32) -> Vec<u8> {
        let mut png = Vec::new();
        png.extend_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]); // PNG signature
        let mut ihdr_data = Vec::new();
        ihdr_data.extend_from_slice(&width.to_be_bytes());
        ihdr_data.extend_from_slice(&height.to_be_bytes());
        ihdr_data.extend_from_slice(&[8, 6, 0, 0, 0]); // bit depth 8, color type 6 (RGBA), no interlace

        let mut chunk_type_and_data = b"IHDR".to_vec();
        chunk_type_and_data.extend_from_slice(&ihdr_data);
        let crc = crc32(&chunk_type_and_data);

        png.extend_from_slice(&(ihdr_data.len() as u32).to_be_bytes());
        png.extend_from_slice(&chunk_type_and_data);
        png.extend_from_slice(&crc.to_be_bytes());
        png
    }

    /// Plain CRC-32 (IEEE 802.3 polynomial), matching PNG's own chunk checksum — just enough
    /// so the decoder accepts the hand-built `IHDR` chunk as well-formed.
    fn crc32(data: &[u8]) -> u32 {
        let mut crc: u32 = 0xFFFF_FFFF;
        for &byte in data {
            crc ^= byte as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
            }
        }
        !crc
    }

    #[test]
    fn validate_flags_a_hand_built_progressive_and_oversized_cmyk_marker() {
        // SOI, then a minimal SOF2 (progressive) marker declaring 4 components (CMYK) at
        // 1000x1000, then EOI. No real scan data needed: `validate_jpeg` never reads past SOFn.
        let mut bytes = vec![0xFF, 0xD8]; // SOI
        bytes.extend_from_slice(&[0xFF, 0xC2]); // SOF2 = progressive
        let height: u16 = 1000;
        let width: u16 = 1000;
        let payload_len: u16 = 2 + 1 + 2 + 2 + 1; // length field + precision + h + w + ncomp
        bytes.extend_from_slice(&payload_len.to_be_bytes());
        bytes.push(8); // precision
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.push(4); // 4 components = CMYK/YCCK
        bytes.extend_from_slice(&[0xFF, 0xD9]); // EOI

        let compliance = validate_jpeg(&bytes).unwrap();
        assert!(compliance.is_progressive);
        assert!(compliance.exceeds_max_dimension());
        assert_eq!(compliance.color_model, ColorModel::CmykOrYcck);
        assert!(!compliance.is_walkman_ready());
        let badges = compliance.badges();
        assert!(badges.iter().any(|b| b.contains("Exceeds 800px")));
        assert!(badges.iter().any(|b| b.contains("Progressive")));
        assert!(badges.iter().any(|b| b.contains("CMYK")));
    }

    #[test]
    fn export_to_file_writes_exact_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cover.jpg");
        export_to_file(&[1, 2, 3, 4], &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), vec![1, 2, 3, 4]);
    }
}
