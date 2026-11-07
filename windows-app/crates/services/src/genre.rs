//! Genre normalization for the multi-tier tag reader (spec task 8.2).
//!
//! Legacy ID3v1-style genre tags store a genre as a bare numeric code, or an ID3v2 `TCON` frame
//! referencing one via a parenthesized index (optionally followed by free text, e.g.
//! `"(17)Rock"` or chained references like `"(4)(6)Eurodisco"`). [`normalize_genre`] resolves
//! those back to the actual genre name so the UI never shows a bare index to the user.

/// The standard 80 ID3v1 genres (Winamp extensions 80+ are intentionally out of scope — they
/// were never standardized and are rare in real files), indexed by their ID3v1 byte value.
const ID3V1_GENRES: &[&str] = &[
    "Blues", "Classic Rock", "Country", "Dance", "Disco", "Funk", "Grunge", "Hip-Hop", "Jazz",
    "Metal", "New Age", "Oldies", "Other", "Pop", "R&B", "Rap", "Reggae", "Rock", "Techno",
    "Industrial", "Alternative", "Ska", "Death Metal", "Pranks", "Soundtrack", "Euro-Techno",
    "Ambient", "Trip-Hop", "Vocal", "Jazz+Funk", "Fusion", "Trance", "Classical", "Instrumental",
    "Acid", "House", "Game", "Sound Clip", "Gospel", "Noise", "Alternative Rock", "Bass", "Soul",
    "Punk", "Space", "Meditative", "Instrumental Pop", "Instrumental Rock", "Ethnic", "Gothic",
    "Darkwave", "Techno-Industrial", "Electronic", "Pop-Folk", "Eurodance", "Dream",
    "Southern Rock", "Comedy", "Cult", "Gangsta", "Top 40", "Christian Rap", "Pop/Funk", "Jungle",
    "Native American", "Cabaret", "New Wave", "Psychedelic", "Rave", "Showtunes", "Trailer",
    "Lo-Fi", "Tribal", "Acid Punk", "Acid Jazz", "Polka", "Retro", "Musical", "Rock & Roll",
    "Hard Rock",
];

/// Resolves `raw` to a genre name:
/// - a bare integer (`"17"`) or a parenthesized one (`"(17)"`) maps through [`ID3V1_GENRES`];
/// - `"(N)free text"` keeps the free text if present, otherwise resolves `N`;
/// - anything else (already-free-text genres, unknown indices) is trimmed and returned as-is.
pub fn normalize_genre(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if let Some(rest) = trimmed.strip_prefix('(') {
        if let Some(close) = rest.find(')') {
            let (index_str, after) = (&rest[..close], &rest[close + 1..]);
            if let Ok(index) = index_str.parse::<usize>() {
                let after_trimmed = after.trim();
                if !after_trimmed.is_empty() {
                    return after_trimmed.to_string();
                }
                if let Some(name) = ID3V1_GENRES.get(index) {
                    return name.to_string();
                }
            }
            // Not a numeric reference after all (e.g. a genre that just starts with '(') —
            // fall through and return the trimmed original.
        }
    } else if let Ok(index) = trimmed.parse::<usize>() {
        if let Some(name) = ID3V1_GENRES.get(index) {
            return name.to_string();
        }
    }

    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_a_bare_numeric_code() {
        assert_eq!(normalize_genre("17"), "Rock");
    }

    #[test]
    fn resolves_a_parenthesized_code_with_no_trailing_text() {
        assert_eq!(normalize_genre("(17)"), "Rock");
    }

    #[test]
    fn prefers_trailing_free_text_over_the_index() {
        assert_eq!(normalize_genre("(4)Eurodisco"), "Eurodisco");
    }

    #[test]
    fn passes_through_free_text_genres_unchanged() {
        assert_eq!(normalize_genre("Shoegaze"), "Shoegaze");
    }

    #[test]
    fn passes_through_an_out_of_range_index_as_is() {
        assert_eq!(normalize_genre("(999)"), "(999)");
    }

    #[test]
    fn trims_whitespace_and_handles_empty_input() {
        assert_eq!(normalize_genre("  Rock  "), "Rock");
        assert_eq!(normalize_genre(""), "");
        assert_eq!(normalize_genre("   "), "");
    }

    #[test]
    fn genre_table_has_the_standard_80_entries() {
        assert_eq!(ID3V1_GENRES.len(), 80);
        assert_eq!(ID3V1_GENRES[0], "Blues");
        assert_eq!(ID3V1_GENRES[79], "Hard Rock");
    }
}
