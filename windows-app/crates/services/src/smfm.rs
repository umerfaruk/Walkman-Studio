//! SMFM (SenseMe 12-Tone) payload injection into FLAC/MP3 (spec task 8.4, R6.3/R15.4).
//!
//! `domain::senseme` defines the 3,270-byte payload's chunk layout
//! ([`walkman_domain::senseme::SMFM_CHUNK_LAYOUT`]) but not the exact bytes of the `STBF`/`STHF`/
//! `STMM` chunks — those are device-template data the design doc says is "copied from template"
//! from a real macOS-app-produced file, and no such fixture is committed to this repo yet (see
//! `tasks.md`'s task 8.5 note on golden-byte fixtures). [`build_payload`] therefore writes the
//! chunk framing and the fields we *can* derive (`GBPM`, `STSA`, `STMO`) correctly, but
//! zero-fills the three template chunks rather than guessing proprietary byte content — a real
//! on-device SMFM blob would need those bytes to be meaningful. [`inject_into_flac`]/
//! [`inject_into_mp3`] are the real, spec-correct container mechanics (a FLAC `APPLICATION`
//! block / an ID3v2.3 `GEOB` frame) and work correctly today regardless of that gap: once a real
//! template fixture lands, only `build_payload`'s three placeholder fills need to change.

use walkman_domain::senseme::{SMFM_CHUNK_HEADER_LEN, SMFM_CHUNK_LAYOUT, SMFM_MAGIC, SMFM_PAYLOAD_LEN};

use crate::error::{AppError, Result};

/// FLAC `APPLICATION` block registered application ID this port uses for the SenseMe payload
/// (ASCII `"SMFM"`, matching the magic the payload itself starts with).
const FLAC_APPLICATION_ID: [u8; 4] = SMFM_MAGIC;

fn make_chunk_header(tag: &str, body_len: u32) -> [u8; SMFM_CHUNK_HEADER_LEN] {
    let mut header = [0u8; SMFM_CHUNK_HEADER_LEN];
    header[0..4].copy_from_slice(tag.as_bytes());
    header[4..8].copy_from_slice(b"STAE");
    header[8..12].copy_from_slice(b"MMLW");
    // bytes[12..16] = flag word, left zero.
    header[16..20].copy_from_slice(&body_len.to_be_bytes());
    header
}

/// Builds the 3,270-byte SMFM payload for `bpm`/`highlight` (milliseconds)/`mood_scores`. See
/// the module doc for which chunks are real vs. zero-filled placeholders.
pub fn build_payload(bpm: f32, highlight_ms: (u32, u32), mood_scores: [i32; 10]) -> Vec<u8> {
    let mut out = Vec::with_capacity(SMFM_PAYLOAD_LEN);
    out.extend_from_slice(&SMFM_MAGIC);

    for (tag, body_len) in SMFM_CHUNK_LAYOUT {
        out.extend_from_slice(&make_chunk_header(tag, body_len as u32));
        let body_start = out.len();
        match tag {
            "GBPM" => out.extend_from_slice(&bpm.to_be_bytes()),
            "STSA" => {
                out.extend_from_slice(&highlight_ms.0.to_be_bytes());
                out.extend_from_slice(&highlight_ms.1.to_be_bytes());
            }
            "STMO" => {
                for score in mood_scores {
                    for _quartile in 0..4 {
                        out.extend_from_slice(&score.to_be_bytes());
                    }
                }
            }
            // "STBF" / "STHF" / "STMM": real device template data, not yet available as a
            // fixture (see module doc) — zero-filled placeholders of the correct length.
            _ => out.resize(out.len() + body_len, 0),
        }
        debug_assert_eq!(out.len() - body_start, body_len, "chunk {tag} body length mismatch");
    }

    debug_assert_eq!(out.len(), SMFM_PAYLOAD_LEN);
    out
}

/// Inserts (or replaces) the `SMFM` `APPLICATION` block in a FLAC metadata-block chain.
/// `flac_bytes` must start with the `fLaC` marker; the audio stream after the metadata blocks is
/// copied through unchanged, same as [`crate::flac_tags::write_tags`].
pub fn inject_into_flac(flac_bytes: &[u8], payload: &[u8]) -> Result<Vec<u8>> {
    const STREAMINFO: u8 = 0;
    const APPLICATION: u8 = 2;

    if flac_bytes.len() < 4 || &flac_bytes[0..4] != b"fLaC" {
        return Err(AppError::Tag("not a FLAC file (missing 'fLaC' marker)".into()));
    }

    let mut pos = 4usize;
    let mut blocks: Vec<(u8, Vec<u8>)> = Vec::new();
    loop {
        if pos + 4 > flac_bytes.len() {
            return Err(AppError::Tag("truncated FLAC metadata block header".into()));
        }
        let header = flac_bytes[pos];
        let is_last = header & 0x80 != 0;
        let block_type = header & 0x7F;
        let length =
            u32::from_be_bytes([0, flac_bytes[pos + 1], flac_bytes[pos + 2], flac_bytes[pos + 3]]) as usize;
        pos += 4;
        if pos + length > flac_bytes.len() {
            return Err(AppError::Tag("truncated FLAC metadata block body".into()));
        }
        blocks.push((block_type, flac_bytes[pos..pos + length].to_vec()));
        pos += length;
        if is_last {
            break;
        }
    }
    let audio_data = &flac_bytes[pos..];

    let mut smfm_body = FLAC_APPLICATION_ID.to_vec();
    smfm_body.extend_from_slice(payload);

    let mut replaced = false;
    for block in &mut blocks {
        if block.0 == APPLICATION && block.1.len() >= 4 && block.1[0..4] == FLAC_APPLICATION_ID {
            block.1 = smfm_body.clone();
            replaced = true;
            break;
        }
    }
    if !replaced {
        blocks.push((APPLICATION, smfm_body));
    }
    if blocks.first().map(|(t, _)| *t) != Some(STREAMINFO) {
        return Err(AppError::Tag("FLAC file is missing a leading STREAMINFO block".into()));
    }

    let mut out = Vec::new();
    out.extend_from_slice(b"fLaC");
    let last_index = blocks.len() - 1;
    for (i, (block_type, body)) in blocks.iter().enumerate() {
        let mut header = block_type & 0x7F;
        if i == last_index {
            header |= 0x80;
        }
        out.push(header);
        out.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
        out.extend_from_slice(body);
    }
    out.extend_from_slice(audio_data);
    Ok(out)
}

/// Inserts an ID3v2.3 `GEOB` (General Encapsulated Object) frame carrying the SMFM payload,
/// using [`crate::id3v2`]'s own frame-level plumbing so the surrounding tag stays consistent
/// with whatever `id3v2::write_tags` already produced. Any existing `GEOB` frame whose MIME type
/// is `application/x-smfm` is replaced; the frame is appended via a second [`crate::id3v2::write_tags`]-style
/// re-serialization is avoided here by operating directly on the already-written tag bytes.
pub fn inject_into_mp3(mp3_bytes: &[u8], payload: &[u8]) -> Result<Vec<u8>> {
    crate::id3v2::inject_geob_frame(mp3_bytes, "application/x-smfm", "SMFM", payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_payload_has_the_documented_total_length() {
        let payload = build_payload(128.0, (1000, 2000), [10; 10]);
        assert_eq!(payload.len(), SMFM_PAYLOAD_LEN);
        assert_eq!(&payload[0..4], SMFM_MAGIC);
    }

    #[test]
    fn gbpm_chunk_round_trips_the_bpm_value() {
        let payload = build_payload(123.4, (0, 0), [0; 10]);
        // magic(4) + header(20) = offset 24 for GBPM's body.
        let bpm_bytes: [u8; 4] = payload[24..28].try_into().unwrap();
        assert_eq!(f32::from_be_bytes(bpm_bytes), 123.4);
    }

    #[test]
    fn injecting_into_flac_adds_exactly_one_smfm_application_block() {
        let mut flac = Vec::new();
        flac.extend_from_slice(b"fLaC");
        flac.push(0x80); // STREAMINFO, last block
        flac.extend_from_slice(&34u32.to_be_bytes()[1..]);
        flac.extend_from_slice(&[0u8; 34]);
        flac.extend_from_slice(b"AUDIO_FRAMES");

        let payload = build_payload(100.0, (500, 1500), [1; 10]);
        let injected = inject_into_flac(&flac, &payload).unwrap();
        assert!(injected.ends_with(b"AUDIO_FRAMES"));

        // Running it twice must replace, not duplicate, the APPLICATION/SMFM block.
        let injected_twice = inject_into_flac(&injected, &payload).unwrap();
        let count = injected_twice
            .windows(4)
            .filter(|w| *w == b"SMFM")
            .count();
        // The magic appears once as the APPLICATION id and once as the payload's own leading
        // magic — both inside a single block, so exactly 2 occurrences means "injected once".
        assert_eq!(count, 2, "expected exactly one SMFM block (id + payload magic)");
    }

    #[test]
    fn rejects_non_flac_bytes() {
        assert!(inject_into_flac(b"not flac", &[]).is_err());
    }
}
