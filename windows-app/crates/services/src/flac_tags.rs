//! Native FLAC metadata block reader/writer (spec task 8.2).
//!
//! Parses and rewrites the metadata-block chain of a FLAC stream directly (not through a tag
//! library) so writes are byte-exact and never touch the audio frames: every block except
//! `VORBIS_COMMENT` and the Sony-required `PICTURE` (type 3, front cover) is preserved verbatim,
//! in its original position, and the entropy-coded audio stream after the last metadata block is
//! copied byte-for-byte (R13.4/R19.3: "audio frames SHALL never be re-encoded during tagging").

use crate::error::{AppError, Result};

const FLAC_MAGIC: &[u8; 4] = b"fLaC";

/// FLAC metadata block type codes (spec, section 8).
mod block_type {
    pub const STREAMINFO: u8 = 0;
    pub const VORBIS_COMMENT: u8 = 4;
    pub const PICTURE: u8 = 6;
}

/// Front-cover picture type per the FLAC `PICTURE` block / ID3v2 APIC convention (R13.4).
const PICTURE_TYPE_FRONT_COVER: u32 = 3;

/// One raw metadata block as found in the file: its type byte (without the last-block flag) and
/// its body bytes (without the 4-byte block header).
struct RawBlock {
    block_type: u8,
    body: Vec<u8>,
}

/// A parsed FLAC file: every metadata block in original order, plus the untouched audio stream
/// that followed them.
struct ParsedFlac {
    blocks: Vec<RawBlock>,
    audio_data: Vec<u8>,
}

fn parse(bytes: &[u8]) -> Result<ParsedFlac> {
    if bytes.len() < 4 || &bytes[0..4] != FLAC_MAGIC {
        return Err(AppError::Tag("not a FLAC file (missing 'fLaC' marker)".into()));
    }

    let mut pos = 4usize;
    let mut blocks = Vec::new();
    loop {
        if pos + 4 > bytes.len() {
            return Err(AppError::Tag("truncated FLAC metadata block header".into()));
        }
        let header = bytes[pos];
        let is_last = header & 0x80 != 0;
        let block_type = header & 0x7F;
        let length = u32::from_be_bytes([0, bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]) as usize;
        pos += 4;
        if pos + length > bytes.len() {
            return Err(AppError::Tag("truncated FLAC metadata block body".into()));
        }
        blocks.push(RawBlock {
            block_type,
            body: bytes[pos..pos + length].to_vec(),
        });
        pos += length;
        if is_last {
            break;
        }
    }

    Ok(ParsedFlac {
        blocks,
        audio_data: bytes[pos..].to_vec(),
    })
}

/// Vorbis-comment field names the app reads lyrics from, in priority order (R15.5 / design.md's
/// FLAC lyrics extraction: `LYRICS`/`UNSYNCEDLYRICS`/`SYNCEDLYRICS`).
const LYRICS_FIELDS: &[&str] = &["SYNCEDLYRICS", "LYRICS", "UNSYNCEDLYRICS"];

/// A Vorbis comment block's fields, case-insensitively keyed (Vorbis comment field names are
/// defined to be matched case-insensitively) but preserving first-seen casing when re-serialized
/// for fields this reader/writer doesn't touch.
struct VorbisComments {
    vendor: String,
    /// `(ORIGINAL_KEY, value)` pairs, in file order; unrecognized/未-rewritten fields are kept
    /// as-is so round-tripping never drops custom tags.
    fields: Vec<(String, String)>,
}

impl VorbisComments {
    fn parse(body: &[u8]) -> Result<Self> {
        if body.len() < 4 {
            return Err(AppError::Tag("truncated VORBIS_COMMENT block".into()));
        }
        let vendor_len = u32::from_le_bytes(body[0..4].try_into().unwrap()) as usize;
        let mut pos = 4 + vendor_len;
        if pos > body.len() {
            return Err(AppError::Tag("truncated VORBIS_COMMENT vendor string".into()));
        }
        let vendor = String::from_utf8_lossy(&body[4..pos]).into_owned();

        if pos + 4 > body.len() {
            return Err(AppError::Tag("truncated VORBIS_COMMENT field count".into()));
        }
        let count = u32::from_le_bytes(body[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;

        let mut fields = Vec::with_capacity(count);
        for _ in 0..count {
            if pos + 4 > body.len() {
                return Err(AppError::Tag("truncated VORBIS_COMMENT field length".into()));
            }
            let len = u32::from_le_bytes(body[pos..pos + 4].try_into().unwrap()) as usize;
            pos += 4;
            if pos + len > body.len() {
                return Err(AppError::Tag("truncated VORBIS_COMMENT field".into()));
            }
            let raw = String::from_utf8_lossy(&body[pos..pos + len]).into_owned();
            pos += len;
            if let Some((key, value)) = raw.split_once('=') {
                fields.push((key.to_string(), value.to_string()));
            }
        }

        Ok(Self { vendor, fields })
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    fn lyrics(&self) -> Option<String> {
        LYRICS_FIELDS.iter().find_map(|f| self.get(f).map(str::to_string))
    }

    /// Removes every existing entry for `key` (case-insensitive) then appends `value` if it's
    /// non-empty.
    fn set(&mut self, key: &str, value: &str) {
        self.fields.retain(|(k, _)| !k.eq_ignore_ascii_case(key));
        if !value.is_empty() {
            self.fields.push((key.to_string(), value.to_string()));
        }
    }

    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.vendor.len() as u32).to_le_bytes());
        out.extend_from_slice(self.vendor.as_bytes());
        out.extend_from_slice(&(self.fields.len() as u32).to_le_bytes());
        for (key, value) in &self.fields {
            let entry = format!("{key}={value}");
            out.extend_from_slice(&(entry.len() as u32).to_le_bytes());
            out.extend_from_slice(entry.as_bytes());
        }
        out
    }
}

fn encode_picture_block(jpeg_bytes: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mime = b"image/jpeg";
    let description = b"";
    let mut out = Vec::new();
    out.extend_from_slice(&PICTURE_TYPE_FRONT_COVER.to_be_bytes());
    out.extend_from_slice(&(mime.len() as u32).to_be_bytes());
    out.extend_from_slice(mime);
    out.extend_from_slice(&(description.len() as u32).to_be_bytes());
    out.extend_from_slice(description);
    out.extend_from_slice(&width.to_be_bytes());
    out.extend_from_slice(&height.to_be_bytes());
    out.extend_from_slice(&24u32.to_be_bytes()); // color depth (RGB baseline JPEG)
    out.extend_from_slice(&0u32.to_be_bytes()); // indexed colors used (0 = not indexed)
    out.extend_from_slice(&(jpeg_bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(jpeg_bytes);
    out
}

fn serialize(blocks: &[RawBlock], audio_data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(FLAC_MAGIC);
    let last_index = blocks.len() - 1;
    for (i, block) in blocks.iter().enumerate() {
        let mut header = block.block_type & 0x7F;
        if i == last_index {
            header |= 0x80;
        }
        out.push(header);
        out.extend_from_slice(&(block.body.len() as u32).to_be_bytes()[1..]);
        out.extend_from_slice(&block.body);
    }
    out.extend_from_slice(audio_data);
    out
}

/// Fields read out of a FLAC's `VORBIS_COMMENT` block, plus whether a `PICTURE` block is present.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlacTags {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track_number: String,
    pub disc_number: String,
    pub year: String,
    pub genre: String,
    pub lyrics: Option<String>,
    pub has_artwork: bool,
}

/// Reads the `VORBIS_COMMENT`/`PICTURE` blocks from raw FLAC bytes.
pub fn read_tags(bytes: &[u8]) -> Result<FlacTags> {
    let parsed = parse(bytes)?;
    let mut tags = FlacTags::default();
    for block in &parsed.blocks {
        match block.block_type {
            block_type::VORBIS_COMMENT => {
                let comments = VorbisComments::parse(&block.body)?;
                tags.title = comments.get("TITLE").unwrap_or_default().to_string();
                tags.artist = comments.get("ARTIST").unwrap_or_default().to_string();
                tags.album = comments.get("ALBUM").unwrap_or_default().to_string();
                tags.track_number = comments.get("TRACKNUMBER").unwrap_or_default().to_string();
                tags.disc_number = comments.get("DISCNUMBER").unwrap_or_default().to_string();
                tags.year = comments
                    .get("DATE")
                    .or_else(|| comments.get("YEAR"))
                    .unwrap_or_default()
                    .to_string();
                tags.genre = crate::genre::normalize_genre(comments.get("GENRE").unwrap_or_default());
                tags.lyrics = comments.lyrics();
            }
            block_type::PICTURE => tags.has_artwork = true,
            _ => {}
        }
    }
    Ok(tags)
}

/// New field values plus optional new artwork for [`write_tags`]. `artwork_jpeg` must already be
/// a Baseline JPEG (see `artwork::encode_baseline_jpeg`) — this writer does not re-encode images.
#[derive(Debug, Clone, Default)]
pub struct FlacTagsWrite<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    pub album: &'a str,
    pub track_number: &'a str,
    pub disc_number: &'a str,
    pub year: &'a str,
    pub genre: &'a str,
    pub lyrics: Option<&'a str>,
    /// `(jpeg_bytes, width, height)`. `None` leaves any existing `PICTURE` block untouched;
    /// `Some` replaces every existing `PICTURE` block with this one.
    pub artwork: Option<(&'a [u8], u32, u32)>,
}

/// Rebuilds `bytes`' metadata-block chain with the given fields written into `VORBIS_COMMENT`
/// (creating one if the file had none) and, if `artwork` is set, its `PICTURE` block(s) replaced.
/// Every other block (`STREAMINFO`, `SEEKTABLE`, `APPLICATION`/`SMFM`, `PADDING`, ...) and the
/// audio stream are copied through unchanged.
pub fn write_tags(bytes: &[u8], fields: &FlacTagsWrite<'_>) -> Result<Vec<u8>> {
    let mut parsed = parse(bytes)?;

    let mut comments = parsed
        .blocks
        .iter()
        .find(|b| b.block_type == block_type::VORBIS_COMMENT)
        .map(|b| VorbisComments::parse(&b.body))
        .transpose()?
        .unwrap_or_else(|| VorbisComments {
            vendor: "walkman-studio".to_string(),
            fields: Vec::new(),
        });

    comments.set("TITLE", fields.title);
    comments.set("ARTIST", fields.artist);
    comments.set("ALBUM", fields.album);
    comments.set("TRACKNUMBER", fields.track_number);
    comments.set("DISCNUMBER", fields.disc_number);
    comments.set("DATE", fields.year);
    comments.set("GENRE", fields.genre);
    if let Some(lyrics) = fields.lyrics {
        // Clear every lyrics-carrying field, then write the one canonical one, matching what
        // `read_tags` prefers to read back (SYNCEDLYRICS first).
        for lyrics_field in LYRICS_FIELDS {
            comments.set(lyrics_field, "");
        }
        comments.set("LYRICS", lyrics);
    }

    let new_comment_block = RawBlock {
        block_type: block_type::VORBIS_COMMENT,
        body: comments.encode(),
    };

    let mut new_blocks = Vec::with_capacity(parsed.blocks.len() + 1);
    let mut comment_written = false;
    let mut picture_written = false;
    for block in parsed.blocks.drain(..) {
        match block.block_type {
            block_type::VORBIS_COMMENT => {
                if !comment_written {
                    new_blocks.push(RawBlock {
                        block_type: new_comment_block.block_type,
                        body: new_comment_block.body.clone(),
                    });
                    comment_written = true;
                }
            }
            block_type::PICTURE => {
                if let Some((jpeg, w, h)) = fields.artwork {
                    if !picture_written {
                        new_blocks.push(RawBlock {
                            block_type: block_type::PICTURE,
                            body: encode_picture_block(jpeg, w, h),
                        });
                        picture_written = true;
                    }
                    // Drop any further original PICTURE blocks: we only ever want one.
                } else {
                    new_blocks.push(block); // leave existing artwork exactly as-is.
                }
            }
            _ => new_blocks.push(block),
        }
    }
    if !comment_written {
        new_blocks.push(new_comment_block);
    }
    if !picture_written {
        if let Some((jpeg, w, h)) = fields.artwork {
            new_blocks.push(RawBlock {
                block_type: block_type::PICTURE,
                body: encode_picture_block(jpeg, w, h),
            });
        }
    }

    // STREAMINFO must stay the very first block per the FLAC spec; everything else keeps its
    // relative order from the loop above, which already preserves it.
    if new_blocks.first().map(|b| b.block_type) != Some(block_type::STREAMINFO) {
        return Err(AppError::Tag("FLAC file is missing a leading STREAMINFO block".into()));
    }

    Ok(serialize(&new_blocks, &parsed.audio_data))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal valid FLAC: `STREAMINFO` + a tiny `VORBIS_COMMENT`, no audio frames (sufficient to
    /// exercise the metadata-block rewrite path this module is responsible for).
    fn minimal_flac(extra_vorbis_fields: &[(&str, &str)]) -> Vec<u8> {
        let mut streaminfo = vec![0u8; 34]; // content doesn't matter for these tests
        let mut out = Vec::new();
        out.extend_from_slice(FLAC_MAGIC);

        let is_last_streaminfo = extra_vorbis_fields.is_empty();
        out.push(if is_last_streaminfo { 0x80 } else { 0x00 });
        out.extend_from_slice(&(streaminfo.len() as u32).to_be_bytes()[1..]);
        out.append(&mut streaminfo);

        if !is_last_streaminfo {
            let comments = VorbisComments {
                vendor: "test".into(),
                fields: extra_vorbis_fields
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            };
            let body = comments.encode();
            out.push(0x84); // last block, type 4 = VORBIS_COMMENT
            out.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
            out.extend_from_slice(&body);
        }
        out.extend_from_slice(b"FAKE_AUDIO_FRAMES");
        out
    }

    #[test]
    fn reads_tags_from_an_existing_vorbis_comment_block() {
        let bytes = minimal_flac(&[("TITLE", "Dawn"), ("ARTIST", "Nova"), ("GENRE", "17")]);
        let tags = read_tags(&bytes).unwrap();
        assert_eq!(tags.title, "Dawn");
        assert_eq!(tags.artist, "Nova");
        assert_eq!(tags.genre, "Rock"); // normalized from the bare ID3v1 index
        assert!(!tags.has_artwork);
    }

    #[test]
    fn write_then_read_round_trips_fields_and_preserves_audio_bytes() {
        let original = minimal_flac(&[]);
        let written = write_tags(
            &original,
            &FlacTagsWrite {
                title: "New Title",
                artist: "New Artist",
                album: "New Album",
                track_number: "3",
                disc_number: "1",
                year: "2024",
                genre: "Synthwave",
                lyrics: Some("la la la"),
                artwork: None,
            },
        )
        .unwrap();

        assert!(written.ends_with(b"FAKE_AUDIO_FRAMES"), "audio stream must survive untouched");

        let tags = read_tags(&written).unwrap();
        assert_eq!(tags.title, "New Title");
        assert_eq!(tags.artist, "New Artist");
        assert_eq!(tags.album, "New Album");
        assert_eq!(tags.track_number, "3");
        assert_eq!(tags.disc_number, "1");
        assert_eq!(tags.year, "2024");
        assert_eq!(tags.genre, "Synthwave");
        assert_eq!(tags.lyrics, Some("la la la".to_string()));
        assert!(!tags.has_artwork);
    }

    #[test]
    fn write_adds_a_picture_block_and_preserves_other_blocks() {
        let original = minimal_flac(&[("TITLE", "Old")]);
        let jpeg = vec![0xFF, 0xD8, 0xFF, 0xD9]; // not a real JPEG; writer doesn't validate it
        let written = write_tags(
            &original,
            &FlacTagsWrite {
                title: "Old",
                artwork: Some((&jpeg, 800, 800)),
                ..Default::default()
            },
        )
        .unwrap();

        let tags = read_tags(&written).unwrap();
        assert!(tags.has_artwork);

        let parsed = parse(&written).unwrap();
        let streaminfo_first = parsed.blocks.first().unwrap().block_type == block_type::STREAMINFO;
        assert!(streaminfo_first, "STREAMINFO must remain the first block");
        let picture_blocks = parsed.blocks.iter().filter(|b| b.block_type == block_type::PICTURE).count();
        assert_eq!(picture_blocks, 1);
    }

    #[test]
    fn rewriting_twice_replaces_rather_than_duplicates_the_picture_block() {
        let original = minimal_flac(&[]);
        let jpeg_a = vec![1, 2, 3];
        let jpeg_b = vec![4, 5, 6, 7];

        let once = write_tags(
            &original,
            &FlacTagsWrite {
                artwork: Some((&jpeg_a, 100, 100)),
                ..Default::default()
            },
        )
        .unwrap();
        let twice = write_tags(
            &once,
            &FlacTagsWrite {
                artwork: Some((&jpeg_b, 200, 200)),
                ..Default::default()
            },
        )
        .unwrap();

        let parsed = parse(&twice).unwrap();
        let pictures: Vec<_> = parsed.blocks.iter().filter(|b| b.block_type == block_type::PICTURE).collect();
        assert_eq!(pictures.len(), 1);
        // The picture body's final bytes are the JPEG payload; confirm it's jpeg_b, not jpeg_a.
        assert!(pictures[0].body.ends_with(&jpeg_b));
    }

    #[test]
    fn rejects_bytes_without_the_flac_magic() {
        assert!(read_tags(b"not flac").is_err());
    }
}
