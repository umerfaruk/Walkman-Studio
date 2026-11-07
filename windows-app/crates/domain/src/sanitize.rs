//! Walkman-safe text & filename sanitization and target path layout, ported from
//! `WalkmanEngine.sanitizeWalkmanText` / `sanitizeFilename` / `targetTrackURL`
//! in `Sources/WalkmanStudio/WalkmanEngine.swift`.
//!
//! Spec task 5.3. The Sony Walkman RTOS SQLite media scanner fails on 4-byte
//! UTF-8 sequences (non-BMP, including emoji), so those scalars are stripped,
//! along with control chars, variation selectors, and `[Explicit]` markers.

/// Illegal FAT32/exFAT filename characters.
const ILLEGAL_FILENAME_CHARS: &[char] =
    &['<', '>', ':', '"', '/', '\\', '|', '?', '*', '\0', '\u{001F}'];

/// Sanitizes text for Sony Walkman hardware compatibility.
///
/// 1. Strips any scalar outside the Basic Multilingual Plane (> U+FFFF), such as
///    4-byte UTF-8 emoji, which break the Sony RTOS tag parser.
/// 2. Strips C0/C1 control characters and variation selectors (U+FE00..=U+FE0F).
/// 3. Optionally removes explicit markers like `[Explicit]`, `(Explicit)`, `[E]`, `(E)`.
pub fn sanitize_walkman_text(text: &str, strip_explicit_markers: bool) -> String {
    let mut result = String::with_capacity(text.len());
    for scalar in text.chars() {
        let v = scalar as u32;
        // Strip anything beyond the BMP.
        if v > 0xFFFF {
            continue;
        }
        // Strip C0 (< 0x20) and C1 (0x7F..=0x9F) control characters.
        if v < 0x20 || (0x7F..=0x9F).contains(&v) {
            continue;
        }
        // Strip variation selectors.
        if (0xFE00..=0xFE0F).contains(&v) {
            continue;
        }
        result.push(scalar);
    }

    let clean = result.trim().to_string();

    let clean = if strip_explicit_markers {
        strip_explicit(&clean)
    } else {
        clean
    };

    clean.trim().to_string()
}

/// Removes trailing/standalone explicit markers: `[Explicit]`, `(Explicit)`,
/// `[Explicit Content]`, `[E]`, `(E)` — case-insensitive. Mirrors the Swift regex
/// `\s*[\(\[]\s*(explicit(\s*content)?|e)\s*[\)\]]`.
fn strip_explicit(input: &str) -> String {
    let mut out = input.to_string();
    loop {
        let lower = out.to_lowercase();
        let mut replaced = false;

        // Find a bracketed segment and test its inner token.
        for (open, close) in [('(', ')'), ('[', ']')] {
            let mut search_from = 0usize;
            while let Some(rel_open) = lower[search_from..].find(open) {
                let open_idx = search_from + rel_open;
                if let Some(rel_close) = lower[open_idx..].find(close) {
                    let close_idx = open_idx + rel_close;
                    let inner = lower[open_idx + 1..close_idx].trim();
                    if inner == "explicit" || inner == "explicit content" || inner == "e" {
                        // Also consume one leading whitespace run before the bracket.
                        let mut start = open_idx;
                        while start > 0 && out.as_bytes()[start - 1].is_ascii_whitespace() {
                            start -= 1;
                        }
                        let end = close_idx + close.len_utf8();
                        out.replace_range(start..end, "");
                        replaced = true;
                        break;
                    }
                    search_from = close_idx + 1;
                } else {
                    break;
                }
            }
            if replaced {
                break;
            }
        }

        if !replaced {
            break;
        }
    }
    out
}

/// Sanitizes a filename: strips non-BMP/explicit markers, removes illegal FAT32/
/// exFAT characters, trims trailing dots/spaces, falls back to "Unknown", and caps
/// length. Ported from `WalkmanEngine.sanitizeFilename`.
pub fn sanitize_filename(name: &str, max_length: usize) -> String {
    let mut clean = sanitize_walkman_text(name, true);
    clean.retain(|c| !ILLEGAL_FILENAME_CHARS.contains(&c));
    clean = clean.trim_matches(|c: char| c == '.' || c == ' ').to_string();
    if clean.is_empty() {
        clean = "Unknown".to_string();
    }
    if clean.chars().count() > max_length {
        clean = clean.chars().take(max_length).collect::<String>();
        clean = clean.trim_matches(|c: char| c == '.' || c == ' ').to_string();
    }
    clean
}

/// Default max filename length used by the Swift app.
pub const DEFAULT_MAX_FILENAME_LEN: usize = 120;

/// Builds the relative track path `Artist/Album/NN - Title.ext` with a zero-padded
/// two-digit track number. Ported from `WalkmanEngine.targetTrackURL` (path parts
/// only; the caller joins this under the music root).
pub fn target_track_relative_path(
    artist: &str,
    album: &str,
    track_num: &str,
    title: &str,
    ext: &str,
) -> String {
    let clean_artist = sanitize_filename(artist, DEFAULT_MAX_FILENAME_LEN);
    let clean_album = sanitize_filename(album, DEFAULT_MAX_FILENAME_LEN);
    let clean_title = sanitize_filename(title, DEFAULT_MAX_FILENAME_LEN);

    let t_int: i64 = track_num
        .split('/')
        .next()
        .unwrap_or("1")
        .trim()
        .parse()
        .unwrap_or(1);
    let track_prefix = format!("{:02}", t_int);
    let clean_ext = ext.trim_matches('.').to_lowercase();

    let filename = format!("{} - {}.{}", track_prefix, clean_title, clean_ext);
    format!("{}/{}/{}", clean_artist, clean_album, filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_emoji_and_non_bmp() {
        // 🅴 is U+1F174 (non-BMP) and must be removed.
        assert_eq!(sanitize_walkman_text("Song \u{1F174}", true), "Song");
        assert_eq!(sanitize_walkman_text("Hello \u{1F600}", true), "Hello");
    }

    #[test]
    fn strips_explicit_markers() {
        assert_eq!(sanitize_walkman_text("Song [Explicit]", true), "Song");
        assert_eq!(sanitize_walkman_text("Song (Explicit Content)", true), "Song");
        assert_eq!(sanitize_walkman_text("Track (E)", true), "Track");
        // Without the flag, markers are kept.
        assert_eq!(sanitize_walkman_text("Song [Explicit]", false), "Song [Explicit]");
    }

    #[test]
    fn filename_strips_illegal_chars() {
        assert_eq!(sanitize_filename("a/b:c*d?", DEFAULT_MAX_FILENAME_LEN), "abcd");
        assert_eq!(sanitize_filename("...", DEFAULT_MAX_FILENAME_LEN), "Unknown");
        assert_eq!(sanitize_filename("name.  ", DEFAULT_MAX_FILENAME_LEN), "name");
    }

    #[test]
    fn target_path_layout() {
        assert_eq!(
            target_track_relative_path("Daft Punk", "Discovery", "4", "Harder Better", "FLAC"),
            "Daft Punk/Discovery/04 - Harder Better.flac"
        );
        // Track number with total "4/14" uses only the first part.
        assert_eq!(
            target_track_relative_path("A", "B", "4/14", "T", ".mp3"),
            "A/B/04 - T.mp3"
        );
    }
}
