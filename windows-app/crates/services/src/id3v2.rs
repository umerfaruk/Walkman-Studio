//! Native ID3v2.3 metadata reader/writer for MP3 (spec task 8.3).
//!
//! Writes a binary ID3v2.3 tag with every text/lyrics frame in UTF-16LE with a BOM
//! (`encoding=1`, R13.4) and a synchsafe *tag* header size (the only size field ID3v2.3
//! synchsafe-encodes — each frame's own size field is a plain big-endian `u32`, unlike
//! ID3v2.4). The tag is prepended to the file's audio data; any pre-existing ID3v2 tag is
//! discarded and replaced, while everything after it (audio frames, any trailing ID3v1 tag) is
//! copied through byte-for-byte.

use crate::error::{AppError, Result};

const PICTURE_TYPE_FRONT_COVER: u8 = 3;
const ENCODING_UTF16_BOM: u8 = 1;
const BOM_LE: [u8; 2] = [0xFF, 0xFE];

fn synchsafe(n: u32) -> [u8; 4] {
    [
        ((n >> 21) & 0x7F) as u8,
        ((n >> 14) & 0x7F) as u8,
        ((n >> 7) & 0x7F) as u8,
        (n & 0x7F) as u8,
    ]
}

fn unsynchsafe(bytes: [u8; 4]) -> u32 {
    (u32::from(bytes[0]) << 21) | (u32::from(bytes[1]) << 14) | (u32::from(bytes[2]) << 7) | u32::from(bytes[3])
}

fn utf16le_bytes(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
}

/// Encodes one ID3v2.3 "string" field (`encoding=1`): a BOM followed by the UTF-16LE text. Use
/// this for a frame's sole/final string value, where no terminator is required.
fn encode_utf16_string(s: &str) -> Vec<u8> {
    let mut out = BOM_LE.to_vec();
    out.extend(utf16le_bytes(s));
    out
}

/// Same as [`encode_utf16_string`] plus an explicit null terminator, for a string field that
/// other fields follow within the same frame (e.g. `USLT`'s content descriptor, before the
/// lyrics text) — without it, [`find_terminated`] would consume the rest of the frame.
fn encode_utf16_string_terminated(s: &str) -> Vec<u8> {
    let mut out = encode_utf16_string(s);
    out.extend_from_slice(&[0, 0]);
    out
}

struct Id3Frame {
    id: [u8; 4],
    data: Vec<u8>,
}

struct ParsedMp3 {
    frames: Vec<Id3Frame>,
    audio_data: Vec<u8>,
}

fn parse(bytes: &[u8]) -> Result<ParsedMp3> {
    if bytes.len() < 10 || &bytes[0..3] != b"ID3" {
        return Ok(ParsedMp3 {
            frames: Vec::new(),
            audio_data: bytes.to_vec(),
        });
    }

    let flags = bytes[5];
    let tag_size = unsynchsafe([bytes[6], bytes[7], bytes[8], bytes[9]]) as usize;
    let tag_end = 10 + tag_size;
    if tag_end > bytes.len() {
        return Err(AppError::Tag("truncated ID3v2 tag".into()));
    }

    let mut pos = 10usize;
    if flags & 0x40 != 0 {
        // Extended header present (ID3v2.3: a plain 4-byte BE size, excluding itself).
        if pos + 4 > tag_end {
            return Err(AppError::Tag("truncated ID3v2.3 extended header".into()));
        }
        let ext_size = u32::from_be_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]) as usize;
        pos += 4 + ext_size;
    }

    let mut frames = Vec::new();
    while pos + 10 <= tag_end {
        let id = [bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]];
        if id == [0, 0, 0, 0] {
            break; // padding
        }
        let size = u32::from_be_bytes([bytes[pos + 4], bytes[pos + 5], bytes[pos + 6], bytes[pos + 7]]) as usize;
        let data_start = pos + 10;
        if data_start + size > tag_end {
            return Err(AppError::Tag("truncated ID3v2 frame".into()));
        }
        frames.push(Id3Frame {
            id,
            data: bytes[data_start..data_start + size].to_vec(),
        });
        pos = data_start + size;
    }

    Ok(ParsedMp3 {
        frames,
        audio_data: bytes[tag_end..].to_vec(),
    })
}

/// Finds the end of a string encoded with `encoding` starting at `data[start]`: 1 byte for
/// Latin-1 (0)/UTF-8 (3), 2 bytes (aligned) for UTF-16 (1/2). Returns the string's byte range
/// (excluding the terminator) and the position right after the terminator.
fn find_terminated(data: &[u8], start: usize, encoding: u8) -> (std::ops::Range<usize>, usize) {
    if encoding == ENCODING_UTF16_BOM || encoding == 2 {
        let mut i = start;
        while i + 1 < data.len() {
            if data[i] == 0 && data[i + 1] == 0 {
                return (start..i, i + 2);
            }
            i += 2;
        }
        (start..data.len(), data.len())
    } else {
        let end = data[start..].iter().position(|&b| b == 0).map(|p| start + p).unwrap_or(data.len());
        (start..end, (end + 1).min(data.len()).max(end))
    }
}

fn decode_text(encoding: u8, bytes: &[u8]) -> String {
    match encoding {
        ENCODING_UTF16_BOM => decode_utf16_with_bom(bytes),
        2 => decode_utf16_be(bytes),
        3 => String::from_utf8_lossy(bytes).into_owned(),
        _ => bytes.iter().map(|&b| b as char).collect(), // Latin-1: 1 byte <-> 1 code point
    }
}

fn decode_utf16_with_bom(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        decode_utf16_le(&bytes[2..])
    } else if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        decode_utf16_be(&bytes[2..])
    } else {
        decode_utf16_le(bytes)
    }
}

fn decode_utf16_le(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    String::from_utf16_lossy(&units)
}

fn decode_utf16_be(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
    String::from_utf16_lossy(&units)
}

fn text_frame(id: &[u8; 4], text: &str) -> Id3Frame {
    let mut data = vec![ENCODING_UTF16_BOM];
    data.extend(encode_utf16_string(text));
    Id3Frame { id: *id, data }
}

/// Fields read out of an ID3v2 tag (any text encoding), plus whether an `APIC` frame is present.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Id3Tags {
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

fn frame_text(frame: &Id3Frame) -> String {
    if frame.data.is_empty() {
        return String::new();
    }
    decode_text(frame.data[0], &frame.data[1..])
}

fn parse_uslt(frame: &Id3Frame) -> Option<String> {
    if frame.data.len() < 4 {
        return None;
    }
    let encoding = frame.data[0];
    // data[1..4] = 3-byte language code (ignored for extraction purposes).
    let (_desc_range, lyrics_start) = find_terminated(&frame.data, 4, encoding);
    Some(decode_text(encoding, &frame.data[lyrics_start..]))
}

/// Reads every recognized frame from raw ID3v2 bytes.
pub fn read_tags(bytes: &[u8]) -> Result<Id3Tags> {
    let parsed = parse(bytes)?;
    let mut tags = Id3Tags::default();
    for frame in &parsed.frames {
        match &frame.id {
            b"TIT2" => tags.title = frame_text(frame),
            b"TPE1" => tags.artist = frame_text(frame),
            b"TALB" => tags.album = frame_text(frame),
            b"TRCK" => tags.track_number = frame_text(frame),
            b"TPOS" => tags.disc_number = frame_text(frame),
            b"TYER" => tags.year = frame_text(frame),
            b"TCON" => tags.genre = crate::genre::normalize_genre(&frame_text(frame)),
            b"USLT" => tags.lyrics = parse_uslt(frame),
            b"APIC" => tags.has_artwork = true,
            _ => {}
        }
    }
    Ok(tags)
}

/// New field values plus optional new artwork for [`write_tags`]. `artwork_jpeg` must already be
/// a Baseline JPEG (see `artwork::encode_baseline_jpeg`).
#[derive(Debug, Clone, Default)]
pub struct Id3TagsWrite<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    pub album: &'a str,
    pub track_number: &'a str,
    pub disc_number: &'a str,
    pub year: &'a str,
    pub genre: &'a str,
    pub lyrics: Option<&'a str>,
    pub artwork_jpeg: Option<&'a [u8]>,
}

/// Replaces `bytes`' leading ID3v2 tag (if any) with a freshly-built ID3v2.3 tag carrying
/// `fields`, and returns the new file bytes. The audio stream (and any trailing ID3v1 tag) is
/// copied through unchanged.
pub fn write_tags(bytes: &[u8], fields: &Id3TagsWrite<'_>) -> Result<Vec<u8>> {
    let parsed = parse(bytes)?;

    let mut frames = vec![
        text_frame(b"TIT2", fields.title),
        text_frame(b"TPE1", fields.artist),
        text_frame(b"TALB", fields.album),
        text_frame(b"TRCK", fields.track_number),
        text_frame(b"TPOS", fields.disc_number),
        text_frame(b"TYER", fields.year),
        text_frame(b"TCON", fields.genre),
    ];

    if let Some(lyrics) = fields.lyrics {
        let mut data = vec![ENCODING_UTF16_BOM];
        data.extend_from_slice(b"eng");
        data.extend(encode_utf16_string_terminated("")); // empty content descriptor
        data.extend(encode_utf16_string(lyrics));
        frames.push(Id3Frame { id: *b"USLT", data });
    }

    if let Some(jpeg) = fields.artwork_jpeg {
        let mut data = vec![0u8]; // description encoding: Latin-1
        data.extend_from_slice(b"image/jpeg\0");
        data.push(PICTURE_TYPE_FRONT_COVER);
        data.push(0); // empty description, Latin-1 terminator
        data.extend_from_slice(jpeg);
        frames.push(Id3Frame { id: *b"APIC", data });
    }

    Ok(serialize_tag(&frames, &parsed.audio_data))
}

fn serialize_tag(frames: &[Id3Frame], audio_data: &[u8]) -> Vec<u8> {
    let mut frames_bytes = Vec::new();
    for frame in frames {
        frames_bytes.extend_from_slice(&frame.id);
        frames_bytes.extend_from_slice(&(frame.data.len() as u32).to_be_bytes());
        frames_bytes.extend_from_slice(&[0, 0]); // frame flags
        frames_bytes.extend_from_slice(&frame.data);
    }

    let mut out = Vec::with_capacity(10 + frames_bytes.len() + audio_data.len());
    out.extend_from_slice(b"ID3");
    out.extend_from_slice(&[3, 0]); // version 2.3.0
    out.push(0); // flags: no unsynchronisation, no extended header, no experimental
    out.extend_from_slice(&synchsafe(frames_bytes.len() as u32));
    out.extend_from_slice(&frames_bytes);
    out.extend_from_slice(audio_data);
    out
}

/// Inserts (or replaces, matched by MIME type) a `GEOB` (General Encapsulated Object) frame
/// carrying arbitrary binary `payload` — used by [`crate::smfm`] to embed the SenseMe payload.
/// Every other existing frame (and the audio data) is preserved untouched.
pub fn inject_geob_frame(bytes: &[u8], mime: &str, description: &str, payload: &[u8]) -> Result<Vec<u8>> {
    let parsed = parse(bytes)?;
    let mut frames = parsed.frames;

    frames.retain(|f| {
        if &f.id != b"GEOB" || f.data.is_empty() {
            return true;
        }
        let (mime_range, _) = find_terminated(&f.data, 1, 0);
        decode_text(0, &f.data[mime_range]) != mime
    });

    let mut data = vec![0u8]; // encoding = Latin-1 (mime/filename/description are simple ASCII)
    data.extend_from_slice(mime.as_bytes());
    data.push(0);
    data.push(0); // empty filename, Latin-1 terminator
    data.extend_from_slice(description.as_bytes());
    data.push(0);
    data.extend_from_slice(payload);
    frames.push(Id3Frame { id: *b"GEOB", data });

    Ok(serialize_tag(&frames, &parsed.audio_data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_then_read_round_trips_text_and_lyrics_fields() {
        let mp3_body = b"FAKE_MPEG_AUDIO_FRAMES_AND_ID3V1_TAIL";
        let written = write_tags(
            mp3_body,
            &Id3TagsWrite {
                title: "Dawn",
                artist: "Nova",
                album: "Horizon",
                track_number: "1",
                disc_number: "1",
                year: "2023",
                genre: "Synthwave",
                lyrics: Some("hello\nworld"),
                artwork_jpeg: None,
            },
        )
        .unwrap();

        assert!(written.starts_with(b"ID3\x03\x00"));
        assert!(written.ends_with(mp3_body), "original audio bytes must survive untouched");

        let tags = read_tags(&written).unwrap();
        assert_eq!(tags.title, "Dawn");
        assert_eq!(tags.artist, "Nova");
        assert_eq!(tags.album, "Horizon");
        assert_eq!(tags.track_number, "1");
        assert_eq!(tags.disc_number, "1");
        assert_eq!(tags.year, "2023");
        assert_eq!(tags.genre, "Synthwave");
        assert_eq!(tags.lyrics, Some("hello\nworld".to_string()));
        assert!(!tags.has_artwork);
    }

    #[test]
    fn write_embeds_a_front_cover_apic_frame() {
        let jpeg = vec![0xFF, 0xD8, 1, 2, 3, 0xFF, 0xD9];
        let written = write_tags(
            b"audio",
            &Id3TagsWrite {
                title: "T",
                artwork_jpeg: Some(&jpeg),
                ..Default::default()
            },
        )
        .unwrap();

        let tags = read_tags(&written).unwrap();
        assert!(tags.has_artwork);

        let parsed = parse(&written).unwrap();
        let apic = parsed.frames.iter().find(|f| &f.id == b"APIC").unwrap();
        assert!(apic.data.ends_with(&jpeg));
        assert!(apic.data.starts_with(&[0]));
        assert_eq!(apic.data[12], PICTURE_TYPE_FRONT_COVER); // 1 (enc) + "image/jpeg\0".len() (11)
    }

    #[test]
    fn replacing_an_existing_tag_drops_the_old_frames() {
        let first = write_tags(
            b"audio-bytes",
            &Id3TagsWrite { title: "Old Title", artist: "Old Artist", ..Default::default() },
        )
        .unwrap();
        let second = write_tags(
            &first,
            &Id3TagsWrite { title: "New Title", ..Default::default() },
        )
        .unwrap();

        let tags = read_tags(&second).unwrap();
        assert_eq!(tags.title, "New Title");
        assert_eq!(tags.artist, ""); // old TPE1 is gone, not merged
        assert!(second.ends_with(b"audio-bytes"));
    }

    #[test]
    fn header_size_field_is_synchsafe() {
        let written = write_tags(b"x", &Id3TagsWrite { title: "T", ..Default::default() }).unwrap();
        let size_bytes = [written[6], written[7], written[8], written[9]];
        for b in size_bytes {
            assert_eq!(b & 0x80, 0, "synchsafe bytes must never set the high bit");
        }
        let declared_size = unsynchsafe(size_bytes) as usize;
        assert_eq!(10 + declared_size, written.len() - 1); // written.len() - 1 == tag_end ("x" is 1 byte)
    }

    #[test]
    fn reading_a_file_with_no_id3_tag_returns_empty_fields() {
        let tags = read_tags(b"\x00\x01\x02 just raw mpeg frames").unwrap();
        assert_eq!(tags, Id3Tags::default());
    }
}
