//! Integration test for spec task 7.4: scans a fixture music tree of real (hand-built, minimal)
//! FLAC files into the SQLite cache and asserts the album/artist/hi-res/recent/language
//! aggregation counts `precompute` produces from a real end-to-end scan — not just from
//! manually-constructed `Track` values.

use std::path::Path;
use std::sync::Arc;

use walkman_services::library_scanner::LibraryScanner;
use walkman_services::local_library::{precompute, LocalLibrary};

/// Builds a minimal, spec-valid FLAC file: a `STREAMINFO` block declaring `sample_rate`/
/// `bits_per_sample` (so `lofty` reports real audio properties and `Track::is_hi_res()`
/// classifies correctly) plus a `VORBIS_COMMENT` block carrying TITLE/ARTIST/ALBUM/DATE, and no
/// audio frames. `lofty` reads properties straight from `STREAMINFO`, so a frameless stream is
/// enough to exercise the real tag + properties read path this test is checking.
fn write_minimal_flac(
    path: &Path,
    sample_rate: u32,
    bits_per_sample: u8,
    title: &str,
    artist: &str,
    album: &str,
    year: &str,
) {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"fLaC");

    // STREAMINFO (not the last metadata block: high bit clear).
    let streaminfo = build_streaminfo(sample_rate, bits_per_sample);
    bytes.push(0x00); // type 0 = STREAMINFO, last-block flag clear
    bytes.extend_from_slice(&(streaminfo.len() as u32).to_be_bytes()[1..]); // 24-bit BE length
    bytes.extend_from_slice(&streaminfo);

    // VORBIS_COMMENT (last metadata block: high bit set).
    let vorbis_comment = build_vorbis_comment(title, artist, album, year);
    bytes.push(0x80 | 0x04); // type 4 = VORBIS_COMMENT, last-block flag set
    bytes.extend_from_slice(&(vorbis_comment.len() as u32).to_be_bytes()[1..]);
    bytes.extend_from_slice(&vorbis_comment);

    std::fs::write(path, bytes).unwrap();
}

fn build_streaminfo(sample_rate: u32, bits_per_sample: u8) -> Vec<u8> {
    let mut bits = BitWriter::default();
    bits.push_bits(4096, 16); // min_blocksize
    bits.push_bits(4096, 16); // max_blocksize
    bits.push_bits(0, 24); // min_framesize (unknown)
    bits.push_bits(0, 24); // max_framesize (unknown)
    bits.push_bits(sample_rate as u64, 20);
    bits.push_bits(0, 3); // channels - 1 (=> 1 channel)
    bits.push_bits((bits_per_sample - 1) as u64, 5);
    bits.push_bits(0, 36); // total_samples (0 = unknown/none)
    let mut out = bits.into_bytes();
    out.extend_from_slice(&[0u8; 16]); // MD5 (unused/zero)
    out
}

fn build_vorbis_comment(title: &str, artist: &str, album: &str, year: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let vendor = b"walkman-studio-test";
    out.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    out.extend_from_slice(vendor);

    let comments = [
        format!("TITLE={title}"),
        format!("ARTIST={artist}"),
        format!("ALBUM={album}"),
        format!("DATE={year}"),
    ];
    out.extend_from_slice(&(comments.len() as u32).to_le_bytes());
    for comment in comments {
        out.extend_from_slice(&(comment.len() as u32).to_le_bytes());
        out.extend_from_slice(comment.as_bytes());
    }
    out
}

/// Big-endian-within-byte bit packer, matching FLAC's bitstream packing for `STREAMINFO`.
#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    bit_pos: u8, // number of bits already filled in the last byte (0..8)
}

impl BitWriter {
    fn push_bits(&mut self, value: u64, n_bits: u8) {
        for i in (0..n_bits).rev() {
            let bit = ((value >> i) & 1) as u8;
            if self.bit_pos == 0 {
                self.bytes.push(0);
            }
            let last = self.bytes.last_mut().unwrap();
            *last |= bit << (7 - self.bit_pos);
            self.bit_pos = (self.bit_pos + 1) % 8;
        }
    }

    fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

#[test]
fn scans_a_fixture_tree_and_aggregates_correctly() {
    let root = tempfile::tempdir().unwrap();

    // Album "Horizon" by "Nova", two tracks, one hi-res.
    std::fs::create_dir_all(root.path().join("Nova/Horizon")).unwrap();
    write_minimal_flac(
        &root.path().join("Nova/Horizon/01 Dawn.flac"),
        44_100,
        16,
        "Dawn",
        "Nova",
        "Horizon",
        "2023",
    );
    write_minimal_flac(
        &root.path().join("Nova/Horizon/02 Dusk.flac"),
        96_000,
        24,
        "Dusk",
        "Nova",
        "Horizon",
        "2023",
    );

    // A second artist/album.
    std::fs::create_dir_all(root.path().join("Echo/Reverb")).unwrap();
    write_minimal_flac(
        &root.path().join("Echo/Reverb/01 Signal.flac"),
        44_100,
        16,
        "Signal",
        "Echo",
        "Reverb",
        "2021",
    );

    // A language-study track under /LEARNING/.
    std::fs::create_dir_all(root.path().join("LEARNING/French 101")).unwrap();
    write_minimal_flac(
        &root.path().join("LEARNING/French 101/01 Greetings.flac"),
        44_100,
        16,
        "Greetings",
        "Tutor",
        "French 101",
        "2024",
    );

    // A non-audio file that must be ignored by the scanner.
    std::fs::write(root.path().join("cover.jpg"), b"not audio").unwrap();

    let scanner = Arc::new(LibraryScanner::open_in_memory().unwrap());
    let library = LocalLibrary::new(scanner);
    let folder = library.add_folder(root.path()).expect("add_folder should scan successfully");
    assert_eq!(folder.track_count, 4, "expected 4 real audio files, cover.jpg excluded");

    let tracks = library.tracks().unwrap();
    assert_eq!(tracks.len(), 4);

    let caches = precompute(tracks);
    assert_eq!(caches.stats.track_count, 4);
    assert_eq!(caches.stats.album_count, 3, "Horizon, Reverb, French 101");
    assert_eq!(caches.stats.artist_count, 3, "Nova, Echo, Tutor");
    assert_eq!(caches.stats.hi_res_count, 1, "only Dusk is >16bit/>48kHz");
    assert_eq!(caches.stats.language_study_count, 1, "only the /LEARNING/ track");

    let horizon = caches.albums.iter().find(|a| a.name == "Horizon").unwrap();
    assert_eq!(horizon.track_count, 2);
    assert!(horizon.is_hi_res);
    assert_eq!(horizon.artist, "Nova");

    let nova = caches.artists.iter().find(|a| a.name == "Nova").unwrap();
    assert_eq!(nova.track_count, 2);
    assert_eq!(nova.album_count, 1);
    assert!(nova.is_hi_res);

    assert_eq!(caches.language_study[0].title, "Greetings");
}
