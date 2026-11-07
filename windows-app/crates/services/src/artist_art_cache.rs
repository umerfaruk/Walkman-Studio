//! Artist-art cache (spec task 12.3, R5.4/R18.3): the same disk+memory [`ArtCache`]
//! [`crate::cover_art_cache`] defines, keyed by `"artist|album"` instead of a file path — art is
//! typically shared across every track on an album, so caching at that granularity avoids
//! redundant downloads/decodes per track.

use crate::cover_art_cache::ArtCache;

/// Artist-art cache, keyed by `"artist|album"` (lowercased).
pub type ArtistArtCache = ArtCache;

pub fn artist_art_cache_key(artist: &str, album: &str) -> String {
    format!("{}|{}", artist.trim().to_lowercase(), album.trim().to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_is_case_and_whitespace_insensitive() {
        assert_eq!(artist_art_cache_key("Nova", "Horizon"), artist_art_cache_key("  nova  ", "HORIZON"));
    }

    #[test]
    fn different_albums_by_the_same_artist_get_different_keys() {
        assert_ne!(artist_art_cache_key("Nova", "Horizon"), artist_art_cache_key("Nova", "Echoes"));
    }
}
