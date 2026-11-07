//! Artist-name parsing, ported 1:1 from `Track.parsePrimaryAndFeaturedArtist`
//! (and helpers) in `Sources/WalkmanStudio/Models.swift`.
//!
//! Spec task 5.2. Behavior-critical: splits an artist string into a primary
//! artist and an optional featured/guest string, while NEVER splitting inside
//! parentheses/brackets and honoring a hardcoded known-exceptions list of band
//! names that legitimately contain commas, ampersands, or slashes.

/// Result of parsing an artist string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedArtist {
    pub primary: String,
    pub featured: Option<String>,
}

/// Band/artist names that contain commas, ampersands, or slashes as part of
/// their official name and must not be split. Stored lowercased for matching.
/// Ported verbatim from the Swift `knownExceptions` array.
const KNOWN_EXCEPTIONS: &[&str] = &[
    "3d (danao, dancel, dumas)",
    "tyler, the creator",
    "earth, wind & fire",
    "crosby, stills, nash & young",
    "crosby, stills & nash",
    "emerson, lake & palmer",
    "blood, sweat & tears",
    "peter, paul and mary",
    "peter, paul & mary",
    "simon & garfunkel",
    "hall & oates",
    "daryl hall & john oates",
    "brooks & dunn",
    "bell biv devoe",
    "kool & the gang",
    "kc & the sunshine band",
    "sly & the family stone",
    "sly and the family stone",
    "huey lewis & the news",
    "huey lewis and the news",
    "tom petty and the heartbreakers",
    "tom petty & the heartbreakers",
    "bob marley & the wailers",
    "bob marley and the wailers",
    "florence + the machine",
    "florence and the machine",
    "of monsters and men",
    "captain & tennille",
    "katrina and the waves",
    "katrina & the waves",
    "joan jett & the blackhearts",
    "orange & lemons",
    "she & him",
    "ben&ben",
    "ac/dc",
];

/// Explicit "featuring" keyword markers (with surrounding spaces), case-insensitive.
const KEYWORDS: &[&str] = &[
    " feat. ", " feat ", " ft. ", " ft ", " featuring ", " with ", " w/ ",
];

/// Prefixes stripped from the start of a featured string.
const FEAT_PREFIXES: &[&str] = &[
    "feat. ", "feat ", "ft. ", "ft ", "featuring ", "with ", "w/ ",
];

/// Delimiters considered at bracket-nesting depth 0, in priority order.
const DELIMITERS: &[&str] = &["; ", " // ", " / ", ", "];

/// True if parentheses and brackets are balanced. Ported from Swift `isBalanced`.
pub fn is_balanced(text: &str) -> bool {
    let mut p_depth: i32 = 0;
    let mut b_depth: i32 = 0;
    for ch in text.chars() {
        match ch {
            '(' => p_depth += 1,
            ')' => {
                p_depth -= 1;
                if p_depth < 0 {
                    return false;
                }
            }
            '[' => b_depth += 1,
            ']' => {
                b_depth -= 1;
                if b_depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    p_depth == 0 && b_depth == 0
}

/// Trims whitespace and trailing punctuation (`, ; / - &`). Ported from Swift.
pub fn clean_trailing_punctuation(text: &str) -> String {
    let mut s = text.trim().to_string();
    loop {
        match s.chars().last() {
            Some(c) if c == ',' || c == ';' || c == '/' || c == '-' || c == '&' || c.is_whitespace() => {
                s.pop();
                s = s.trim().to_string();
            }
            _ => break,
        }
    }
    s
}

/// Strips leading separator chars and a leading feat-token prefix. Ported from Swift.
pub fn strip_leading_feat_tokens(text: &str) -> String {
    let mut s = text.trim().to_string();
    while s.starts_with([',', ';', '/', '-', '&']) {
        s = s[1..].trim().to_string();
    }
    let lower = s.to_lowercase();
    for prefix in FEAT_PREFIXES {
        if lower.starts_with(prefix) {
            // prefixes are ASCII, so byte length == char length here
            return s[prefix.len()..].trim().to_string();
        }
    }
    s
}

/// Case-insensitive search for the first occurrence of `needle` in `haystack`,
/// returning the byte range (start, end).
fn find_ci(haystack: &str, needle: &str) -> Option<(usize, usize)> {
    let h = haystack.to_lowercase();
    let n = needle.to_lowercase();
    h.find(&n).map(|start| {
        // `to_lowercase` can change byte lengths for non-ASCII; our needles are
        // ASCII so the lowercased haystack indices line up with the original for
        // ASCII matches. Map by counting chars to be safe.
        let char_start = h[..start].chars().count();
        let byte_start = haystack.char_indices().nth(char_start).map(|(i, _)| i).unwrap_or(start);
        let byte_end = haystack
            .char_indices()
            .nth(char_start + needle.chars().count())
            .map(|(i, _)| i)
            .unwrap_or(haystack.len());
        (byte_start, byte_end)
    })
}

/// Splits an artist string into a primary artist and an optional featured string.
/// Ported 1:1 from `Track.parsePrimaryAndFeaturedArtist`.
pub fn parse_primary_and_featured(raw: &str) -> ParsedArtist {
    let trimmed = raw.trim().to_string();
    if trimmed.is_empty() {
        return ParsedArtist {
            primary: "Unknown Artist".to_string(),
            featured: None,
        };
    }

    let lower = trimmed.to_lowercase();

    // Known exceptions: exact match or prefix match.
    for known in KNOWN_EXCEPTIONS {
        if lower == *known {
            return ParsedArtist { primary: trimmed, featured: None };
        }
        if lower.starts_with(known) {
            let known_char_len = known.chars().count();
            let rest: String = trimmed.chars().skip(known_char_len).collect::<String>().trim().to_string();
            let primary: String = trimmed.chars().take(known_char_len).collect();
            if rest.is_empty() {
                return ParsedArtist { primary: trimmed, featured: None };
            }
            let cleaned_feat = strip_leading_feat_tokens(&rest);
            return ParsedArtist {
                primary,
                featured: if cleaned_feat.is_empty() { None } else { Some(cleaned_feat) },
            };
        }
    }

    // 1. Explicit keyword markers.
    for kw in KEYWORDS {
        if let Some((lo, hi)) = find_ci(&trimmed, kw) {
            let primary = clean_trailing_punctuation(&trimmed[..lo]);
            let feat = strip_leading_feat_tokens(&trimmed[hi..]);
            if !primary.is_empty() && is_balanced(&primary) && (feat.is_empty() || is_balanced(&feat)) {
                return ParsedArtist {
                    primary,
                    featured: if feat.is_empty() { None } else { Some(feat) },
                };
            }
        }
    }

    // 2. Parenthesized feat, e.g. "Drake (feat. Rihanna)" / "Drake [ft. Rihanna]".
    for (open, close) in [('(', ')'), ('[', ']')] {
        if trimmed.ends_with(close) {
            if let Some(open_byte) = trimmed.rfind(open) {
                let primary = clean_trailing_punctuation(&trimmed[..open_byte]);
                // inside = between the brackets, trimmed.
                let inside_full = &trimmed[open_byte..];
                let inside: String = inside_full
                    .chars()
                    .skip(1) // drop open bracket
                    .collect::<String>();
                let inside = inside.strip_suffix(close).unwrap_or(&inside).trim().to_string();
                let lower_inside = inside.to_lowercase();
                for prefix in FEAT_PREFIXES {
                    if lower_inside.starts_with(prefix) {
                        let feat = strip_leading_feat_tokens(&inside[prefix.len()..]);
                        if !primary.is_empty() && is_balanced(&primary) && (feat.is_empty() || is_balanced(&feat)) {
                            return ParsedArtist {
                                primary,
                                featured: if feat.is_empty() { None } else { Some(feat) },
                            };
                        }
                    }
                }
            }
        }
    }

    // 3. Delimiters at bracket-nesting depth 0. Never split inside () or [].
    let mut p_depth: i32 = 0;
    let mut b_depth: i32 = 0;
    let mut earliest: Option<(usize, usize)> = None;
    let chars: Vec<(usize, char)> = trimmed.char_indices().collect();
    let mut idx = 0usize;
    while idx < chars.len() {
        let (byte_pos, ch) = chars[idx];
        match ch {
            '(' => p_depth += 1,
            ')' => p_depth = (p_depth - 1).max(0),
            '[' => b_depth += 1,
            ']' => b_depth = (b_depth - 1).max(0),
            _ if p_depth == 0 && b_depth == 0 => {
                let remaining = &trimmed[byte_pos..];
                for delim in DELIMITERS {
                    if remaining.starts_with(delim) {
                        earliest = Some((byte_pos, byte_pos + delim.len()));
                        break;
                    }
                }
            }
            _ => {}
        }
        if earliest.is_some() {
            break;
        }
        idx += 1;
    }

    if let Some((lo, hi)) = earliest {
        let primary = clean_trailing_punctuation(&trimmed[..lo]);
        let feat = strip_leading_feat_tokens(&trimmed[hi..]);
        if !primary.is_empty() && !feat.is_empty() && is_balanced(&primary) && is_balanced(&feat) {
            return ParsedArtist { primary, featured: Some(feat) };
        }
    }

    ParsedArtist { primary: trimmed, featured: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> (String, Option<String>) {
        let p = parse_primary_and_featured(s);
        (p.primary, p.featured)
    }

    #[test]
    fn empty_is_unknown() {
        assert_eq!(parse("   "), ("Unknown Artist".into(), None));
    }

    #[test]
    fn known_exception_not_split() {
        assert_eq!(parse("Earth, Wind & Fire"), ("Earth, Wind & Fire".into(), None));
        assert_eq!(parse("Tyler, the Creator"), ("Tyler, the Creator".into(), None));
        assert_eq!(parse("AC/DC"), ("AC/DC".into(), None));
        assert_eq!(parse("Ben&Ben"), ("Ben&Ben".into(), None));
    }

    #[test]
    fn keyword_feat_split() {
        assert_eq!(parse("Drake feat. Rihanna"), ("Drake".into(), Some("Rihanna".into())));
        assert_eq!(parse("Calvin Harris ft. Dua Lipa"), ("Calvin Harris".into(), Some("Dua Lipa".into())));
    }

    #[test]
    fn parenthesized_feat_split() {
        assert_eq!(parse("Drake (feat. Rihanna)"), ("Drake".into(), Some("Rihanna".into())));
        assert_eq!(parse("Eminem [ft. Dido]"), ("Eminem".into(), Some("Dido".into())));
    }

    #[test]
    fn comma_delimiter_split_outside_brackets() {
        assert_eq!(parse("6CycleMind, Ebe Dancel"), ("6CycleMind".into(), Some("Ebe Dancel".into())));
    }

    #[test]
    fn no_split_inside_parens() {
        // Not in exceptions list but has an internal comma within parens.
        assert_eq!(parse("Band (Alice, Bob)"), ("Band (Alice, Bob)".into(), None));
    }

    #[test]
    fn plain_single_artist() {
        assert_eq!(parse("Daft Punk"), ("Daft Punk".into(), None));
    }
}
