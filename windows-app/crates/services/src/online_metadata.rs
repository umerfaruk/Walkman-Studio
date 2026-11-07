//! Online metadata and artist imagery (spec task 12.3, R17.1-17.3): iTunes Search API + Deezer.
//!
//! Same split as `lyrics.rs`: URL-building and JSON-parsing are pure, tested functions; the
//! actual HTTP round trip is not exercised against the live APIs in this session (no internet
//! access to verify against here). `fetch_*` functions never panic on a network failure — they
//! return `None`/an empty `Vec`, per R17.3's "resilient to failure... never block the UI".

use serde::Deserialize;

const ITUNES_SEARCH_URL: &str = "https://itunes.apple.com/search";
const DEEZER_SEARCH_URL: &str = "https://api.deezer.com/search";
const DEEZER_ARTIST_SEARCH_URL: &str = "https://api.deezer.com/search/artist";

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// One online metadata/cover-art candidate (R17.1).
#[derive(Debug, Clone, PartialEq)]
pub struct MetadataCandidate {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub year: String,
    pub genre: String,
    pub cover_art_url: Option<String>,
    pub source: &'static str,
}

pub fn build_itunes_search_url(artist: &str, title: &str, limit: u32) -> String {
    format!("{ITUNES_SEARCH_URL}?term={}&entity=song&limit={limit}", urlencode(&format!("{artist} {title}")))
}

#[derive(Debug, Deserialize)]
struct ItunesResponse {
    results: Vec<ItunesResult>,
}

#[derive(Debug, Deserialize)]
struct ItunesResult {
    #[serde(rename = "trackName")]
    track_name: Option<String>,
    #[serde(rename = "artistName")]
    artist_name: Option<String>,
    #[serde(rename = "collectionName")]
    collection_name: Option<String>,
    #[serde(rename = "releaseDate")]
    release_date: Option<String>,
    #[serde(rename = "primaryGenreName")]
    primary_genre_name: Option<String>,
    #[serde(rename = "artworkUrl100")]
    artwork_url_100: Option<String>,
}

/// Parses an iTunes Search API response. `artworkUrl100` is upgraded to a larger size by
/// replacing the `100x100` suffix with `600x600`, matching the common convention for this API
/// (it has no explicit "give me the big one" parameter).
pub fn parse_itunes_response(body: &str) -> Vec<MetadataCandidate> {
    let Ok(parsed) = serde_json::from_str::<ItunesResponse>(body) else {
        return Vec::new();
    };
    parsed
        .results
        .into_iter()
        .map(|r| MetadataCandidate {
            title: r.track_name.unwrap_or_default(),
            artist: r.artist_name.unwrap_or_default(),
            album: r.collection_name.unwrap_or_default(),
            year: r.release_date.as_deref().and_then(|d| d.get(0..4)).unwrap_or_default().to_string(),
            genre: r.primary_genre_name.unwrap_or_default(),
            cover_art_url: r.artwork_url_100.map(|u| u.replace("100x100", "600x600")),
            source: "iTunes",
        })
        .collect()
}

pub fn build_deezer_search_url(artist: &str, title: &str) -> String {
    format!("{DEEZER_SEARCH_URL}?q={}", urlencode(&format!(r#"artist:"{artist}" track:"{title}""#)))
}

#[derive(Debug, Deserialize)]
struct DeezerSearchResponse {
    data: Vec<DeezerTrack>,
}

#[derive(Debug, Deserialize)]
struct DeezerTrack {
    title: Option<String>,
    artist: Option<DeezerArtistRef>,
    album: Option<DeezerAlbumRef>,
}

#[derive(Debug, Deserialize)]
struct DeezerArtistRef {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeezerAlbumRef {
    title: Option<String>,
    cover_big: Option<String>,
}

pub fn parse_deezer_response(body: &str) -> Vec<MetadataCandidate> {
    let Ok(parsed) = serde_json::from_str::<DeezerSearchResponse>(body) else {
        return Vec::new();
    };
    parsed
        .data
        .into_iter()
        .map(|t| MetadataCandidate {
            title: t.title.unwrap_or_default(),
            artist: t.artist.and_then(|a| a.name).unwrap_or_default(),
            album: t.album.as_ref().and_then(|a| a.title.clone()).unwrap_or_default(),
            year: String::new(), // Deezer's track search doesn't return a release year
            genre: String::new(),
            cover_art_url: t.album.and_then(|a| a.cover_big),
            source: "Deezer",
        })
        .collect()
}

pub fn build_deezer_artist_search_url(artist_name: &str) -> String {
    format!("{DEEZER_ARTIST_SEARCH_URL}?q={}", urlencode(artist_name))
}

#[derive(Debug, Deserialize)]
struct DeezerArtistSearchResponse {
    data: Vec<DeezerArtistImage>,
}

#[derive(Debug, Deserialize)]
struct DeezerArtistImage {
    picture_big: Option<String>,
}

/// R17.2: the first result's `picture_big` URL, if any.
pub fn parse_deezer_artist_image(body: &str) -> Option<String> {
    let parsed: DeezerArtistSearchResponse = serde_json::from_str(body).ok()?;
    parsed.data.into_iter().find_map(|a| a.picture_big)
}

/// Queries both iTunes and Deezer and combines their candidates (R17.1). Either source failing
/// (network error, malformed response) simply contributes no candidates rather than failing the
/// whole lookup (R17.3).
pub async fn fetch_metadata_candidates(client: &reqwest::Client, artist: &str, title: &str) -> Vec<MetadataCandidate> {
    let mut candidates = Vec::new();

    if let Ok(resp) = client.get(build_itunes_search_url(artist, title, 5)).send().await {
        if let Ok(body) = resp.text().await {
            candidates.extend(parse_itunes_response(&body));
        }
    }
    if let Ok(resp) = client.get(build_deezer_search_url(artist, title)).send().await {
        if let Ok(body) = resp.text().await {
            candidates.extend(parse_deezer_response(&body));
        }
    }
    candidates
}

/// R17.2: fetches an artist image URL from Deezer's artist search. Returns `None` on any
/// failure or empty result set.
pub async fn fetch_artist_image_url(client: &reqwest::Client, artist_name: &str) -> Option<String> {
    let resp = client.get(build_deezer_artist_search_url(artist_name)).send().await.ok()?;
    let body = resp.text().await.ok()?;
    parse_deezer_artist_image(&body)
}

/// Downloads raw bytes from `url` (e.g. a resolved cover/artist image URL), for the caller to
/// hand to `artwork::encode_baseline_jpeg` and a cache `put`.
pub async fn download_bytes(client: &reqwest::Client, url: &str) -> Option<Vec<u8>> {
    let resp = client.get(url).send().await.ok()?;
    resp.bytes().await.ok().map(|b| b.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_the_itunes_url_with_combined_artist_and_title_term() {
        let url = build_itunes_search_url("AC/DC", "Back in Black", 5);
        assert!(url.starts_with("https://itunes.apple.com/search?term="));
        assert!(url.contains("entity=song"));
        assert!(url.contains("limit=5"));
    }

    #[test]
    fn parses_itunes_results_and_upgrades_artwork_size() {
        let body = r#"{"resultCount":1,"results":[{
            "trackName":"Back in Black","artistName":"AC/DC","collectionName":"Back in Black",
            "releaseDate":"1980-07-25T07:00:00Z","primaryGenreName":"Rock",
            "artworkUrl100":"https://example.com/100x100bb.jpg"
        }]}"#;
        let candidates = parse_itunes_response(body);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].title, "Back in Black");
        assert_eq!(candidates[0].year, "1980");
        assert_eq!(candidates[0].cover_art_url.as_deref(), Some("https://example.com/600x600bb.jpg"));
        assert_eq!(candidates[0].source, "iTunes");
    }

    #[test]
    fn itunes_parse_returns_empty_on_malformed_json() {
        assert!(parse_itunes_response("not json").is_empty());
    }

    #[test]
    fn builds_the_deezer_search_url_with_quoted_artist_and_track() {
        let url = build_deezer_search_url("Nova", "Dawn");
        assert!(url.starts_with("https://api.deezer.com/search?q="));
    }

    #[test]
    fn parses_deezer_track_results() {
        let body = r#"{"data":[{
            "title":"Dawn","artist":{"name":"Nova"},
            "album":{"title":"Horizon","cover_big":"https://example.com/cover.jpg"}
        }]}"#;
        let candidates = parse_deezer_response(body);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].artist, "Nova");
        assert_eq!(candidates[0].album, "Horizon");
        assert_eq!(candidates[0].cover_art_url.as_deref(), Some("https://example.com/cover.jpg"));
        assert_eq!(candidates[0].source, "Deezer");
    }

    #[test]
    fn parses_the_first_deezer_artist_image() {
        let body = r#"{"data":[{"picture_big":"https://example.com/artist.jpg"},{"picture_big":"https://example.com/other.jpg"}]}"#;
        assert_eq!(parse_deezer_artist_image(body), Some("https://example.com/artist.jpg".to_string()));
    }

    #[test]
    fn artist_image_parse_returns_none_on_empty_results() {
        assert_eq!(parse_deezer_artist_image(r#"{"data":[]}"#), None);
    }
}
