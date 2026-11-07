//! Lyrics engine (spec task 12.2, R15.1/R15.3): LRCLIB primary, Lyrics.ovh fallback, with
//! concurrent batch fetching and synced/plain/skipped/not-found accounting.
//!
//! URL-building and JSON-parsing are split out as pure functions so they're testable without a
//! network call; the actual HTTP round trip (`fetch_one`/`fetch_batch_async`) is not exercised
//! against the real APIs in this session — same documented gap as every other network/`ffmpeg`
//! dependent path in this port (no internet access to verify against here, and no mock-HTTP
//! dependency was judged worth adding for it).

use std::sync::Arc;
use std::sync::mpsc::Receiver;

use serde::Deserialize;

/// This app's User-Agent for both APIs (R15.1: "using the same user-agent string" across both).
pub const USER_AGENT: &str = "WalkmanStudio/3.1.0 (+https://github.com/Jhay-EL/Walkman-Studio)";

const LRCLIB_BASE: &str = "https://lrclib.net/api";
const LYRICS_OVH_BASE: &str = "https://api.lyrics.ovh/v1";

/// What a successful lookup found.
#[derive(Debug, Clone, PartialEq)]
pub enum LyricsOutcomeKind {
    /// A synced `.lrc`-format lyrics body (LRCLIB only).
    Synced(String),
    Plain(String),
    /// Already had lyrics; not re-fetched (R15.3's "Skipped" metric).
    Skipped,
    NotFound,
}

impl LyricsOutcomeKind {
    pub fn metric_label(&self) -> &'static str {
        match self {
            LyricsOutcomeKind::Synced(_) => "Synced .LRC",
            LyricsOutcomeKind::Plain(_) => "Plain Text",
            LyricsOutcomeKind::Skipped => "Skipped",
            LyricsOutcomeKind::NotFound => "Not Found",
        }
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Builds the LRCLIB `/get` URL (R15.1: "matched by title/artist/album/millisecond duration" —
/// LRCLIB's API takes whole seconds, rounded here).
pub fn build_lrclib_get_url(title: &str, artist: &str, album: &str, duration_seconds: f64) -> String {
    format!(
        "{LRCLIB_BASE}/get?track_name={}&artist_name={}&album_name={}&duration={}",
        urlencode(title),
        urlencode(artist),
        urlencode(album),
        duration_seconds.round() as i64
    )
}

/// Builds the LRCLIB `/search` URL, used when `/get`'s exact match 404s.
pub fn build_lrclib_search_url(title: &str, artist: &str) -> String {
    format!("{LRCLIB_BASE}/search?track_name={}&artist_name={}", urlencode(title), urlencode(artist))
}

pub fn build_lyrics_ovh_url(title: &str, artist: &str) -> String {
    format!("{LYRICS_OVH_BASE}/{}/{}", urlencode(artist), urlencode(title))
}

#[derive(Debug, Deserialize)]
struct LrclibEntry {
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
    #[serde(rename = "plainLyrics")]
    plain_lyrics: Option<String>,
    duration: Option<f64>,
}

fn lrclib_entry_to_outcome(entry: &LrclibEntry) -> Option<LyricsOutcomeKind> {
    if let Some(synced) = entry.synced_lyrics.as_deref().filter(|s| !s.trim().is_empty()) {
        return Some(LyricsOutcomeKind::Synced(synced.to_string()));
    }
    if let Some(plain) = entry.plain_lyrics.as_deref().filter(|s| !s.trim().is_empty()) {
        return Some(LyricsOutcomeKind::Plain(plain.to_string()));
    }
    None
}

/// Parses an LRCLIB `/get` response body (a single object).
pub fn parse_lrclib_get_response(body: &str) -> Option<LyricsOutcomeKind> {
    let entry: LrclibEntry = serde_json::from_str(body).ok()?;
    lrclib_entry_to_outcome(&entry)
}

/// Parses an LRCLIB `/search` response body (an array), picking the entry whose duration is
/// closest to `expected_duration_seconds` (R15.1: "duration-matched").
pub fn parse_lrclib_search_response(body: &str, expected_duration_seconds: f64) -> Option<LyricsOutcomeKind> {
    let entries: Vec<LrclibEntry> = serde_json::from_str(body).ok()?;
    let best = entries.iter().min_by(|a, b| {
        let da = a.duration.map(|d| (d - expected_duration_seconds).abs()).unwrap_or(f64::MAX);
        let db = b.duration.map(|d| (d - expected_duration_seconds).abs()).unwrap_or(f64::MAX);
        da.total_cmp(&db)
    })?;
    lrclib_entry_to_outcome(best)
}

#[derive(Debug, Deserialize)]
struct LyricsOvhResponse {
    lyrics: Option<String>,
}

/// Parses a Lyrics.ovh response body.
pub fn parse_lyrics_ovh_response(body: &str) -> Option<String> {
    let parsed: LyricsOvhResponse = serde_json::from_str(body).ok()?;
    parsed.lyrics.filter(|s| !s.trim().is_empty())
}

/// One track to look up lyrics for.
#[derive(Debug, Clone)]
pub struct LyricsRequest {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_seconds: f64,
    /// `true` if the track already has lyrics; reported as `Skipped` without a network call.
    pub already_has_lyrics: bool,
}

#[derive(Debug, Clone)]
pub struct LyricsOutcome {
    pub id: String,
    pub kind: LyricsOutcomeKind,
}

/// Progress emitted by [`fetch_batch_async`] (R15.3's live progress + metric pills).
#[derive(Debug, Clone)]
pub struct LyricsBatchProgress {
    pub completed: usize,
    pub total: usize,
    pub last: LyricsOutcome,
}

fn build_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder().user_agent(USER_AGENT).build()
}

async fn fetch_one(client: &reqwest::Client, req: &LyricsRequest) -> LyricsOutcome {
    if req.already_has_lyrics {
        return LyricsOutcome { id: req.id.clone(), kind: LyricsOutcomeKind::Skipped };
    }

    if let Some(kind) = fetch_lrclib(client, req).await {
        return LyricsOutcome { id: req.id.clone(), kind };
    }
    if let Some(text) = fetch_lyrics_ovh(client, req).await {
        return LyricsOutcome { id: req.id.clone(), kind: LyricsOutcomeKind::Plain(text) };
    }
    LyricsOutcome { id: req.id.clone(), kind: LyricsOutcomeKind::NotFound }
}

async fn fetch_lrclib(client: &reqwest::Client, req: &LyricsRequest) -> Option<LyricsOutcomeKind> {
    let get_url = build_lrclib_get_url(&req.title, &req.artist, &req.album, req.duration_seconds);
    if let Ok(resp) = client.get(&get_url).send().await {
        if resp.status().is_success() {
            if let Ok(body) = resp.text().await {
                if let Some(kind) = parse_lrclib_get_response(&body) {
                    return Some(kind);
                }
            }
        }
    }
    let search_url = build_lrclib_search_url(&req.title, &req.artist);
    let resp = client.get(&search_url).send().await.ok()?;
    let body = resp.text().await.ok()?;
    parse_lrclib_search_response(&body, req.duration_seconds)
}

async fn fetch_lyrics_ovh(client: &reqwest::Client, req: &LyricsRequest) -> Option<String> {
    let url = build_lyrics_ovh_url(&req.title, &req.artist);
    let resp = client.get(&url).send().await.ok()?;
    let body = resp.text().await.ok()?;
    parse_lyrics_ovh_response(&body)
}

/// Runs lookups for every request in `requests` concurrently (bounded by `concurrency`,
/// R15.1's "concurrent multi-worker behavior") on a background thread with its own Tokio
/// runtime, publishing a [`LyricsBatchProgress`] per completed item over the returned channel —
/// same off-thread + channel pattern used throughout this crate (e.g.
/// `transfer_queue::transfer_batch_async`).
pub fn fetch_batch_async(requests: Vec<LyricsRequest>, concurrency: usize) -> Receiver<LyricsBatchProgress> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let Ok(runtime) = tokio::runtime::Runtime::new() else {
            return;
        };
        runtime.block_on(async move {
            let Ok(client) = build_client() else { return };
            let total = requests.len();
            let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency.max(1)));
            let mut set = tokio::task::JoinSet::new();

            for req in requests {
                let client = client.clone();
                let semaphore = Arc::clone(&semaphore);
                set.spawn(async move {
                    let _permit = semaphore.acquire().await;
                    fetch_one(&client, &req).await
                });
            }

            let mut completed = 0usize;
            while let Some(result) = set.join_next().await {
                let Ok(outcome) = result else { continue };
                completed += 1;
                if tx.send(LyricsBatchProgress { completed, total, last: outcome }).is_err() {
                    return; // receiver dropped
                }
            }
        });
    });
    rx
}

/// Tallies R15.3's four metric pills across a finished batch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LyricsBatchCounts {
    pub synced: usize,
    pub plain: usize,
    pub skipped: usize,
    pub not_found: usize,
}

pub fn tally(outcomes: &[LyricsOutcome]) -> LyricsBatchCounts {
    let mut counts = LyricsBatchCounts::default();
    for outcome in outcomes {
        match &outcome.kind {
            LyricsOutcomeKind::Synced(_) => counts.synced += 1,
            LyricsOutcomeKind::Plain(_) => counts.plain += 1,
            LyricsOutcomeKind::Skipped => counts.skipped += 1,
            LyricsOutcomeKind::NotFound => counts.not_found += 1,
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_the_lrclib_get_url_with_encoded_params() {
        let url = build_lrclib_get_url("Don't Stop", "AC/DC", "Back in Black", 210.6);
        assert!(url.starts_with("https://lrclib.net/api/get?"));
        assert!(url.contains("track_name=Don%27t%20Stop") || url.contains("track_name=Don%27t"));
        assert!(url.contains("artist_name=AC%2FDC"));
        assert!(url.contains("duration=211")); // rounded
    }

    #[test]
    fn builds_the_lyrics_ovh_url_as_artist_then_title() {
        let url = build_lyrics_ovh_url("Song", "Artist Name");
        assert_eq!(url, "https://api.lyrics.ovh/v1/Artist%20Name/Song");
    }

    #[test]
    fn parses_a_synced_lrclib_response() {
        let body = r#"{"id":1,"syncedLyrics":"[00:01.00]hello","plainLyrics":"hello","duration":200.0}"#;
        assert_eq!(parse_lrclib_get_response(body), Some(LyricsOutcomeKind::Synced("[00:01.00]hello".to_string())));
    }

    #[test]
    fn falls_back_to_plain_when_no_synced_lyrics_present() {
        let body = r#"{"id":1,"syncedLyrics":null,"plainLyrics":"hello world","duration":200.0}"#;
        assert_eq!(parse_lrclib_get_response(body), Some(LyricsOutcomeKind::Plain("hello world".to_string())));
    }

    #[test]
    fn returns_none_when_both_lyrics_fields_are_empty() {
        let body = r#"{"id":1,"syncedLyrics":"","plainLyrics":"","duration":200.0}"#;
        assert_eq!(parse_lrclib_get_response(body), None);
    }

    #[test]
    fn search_response_picks_the_duration_closest_match() {
        let body = r#"[
            {"id":1,"syncedLyrics":null,"plainLyrics":"wrong song","duration":90.0},
            {"id":2,"syncedLyrics":"[00:00.00]right song","plainLyrics":null,"duration":200.0}
        ]"#;
        assert_eq!(
            parse_lrclib_search_response(body, 201.0),
            Some(LyricsOutcomeKind::Synced("[00:00.00]right song".to_string()))
        );
    }

    #[test]
    fn parses_a_lyrics_ovh_response() {
        assert_eq!(parse_lyrics_ovh_response(r#"{"lyrics":"la la la"}"#), Some("la la la".to_string()));
        assert_eq!(parse_lyrics_ovh_response(r#"{"error":"No lyrics found"}"#), None);
    }

    #[test]
    fn tally_counts_each_outcome_kind() {
        let outcomes = vec![
            LyricsOutcome { id: "a".into(), kind: LyricsOutcomeKind::Synced("x".into()) },
            LyricsOutcome { id: "b".into(), kind: LyricsOutcomeKind::Plain("y".into()) },
            LyricsOutcome { id: "c".into(), kind: LyricsOutcomeKind::Skipped },
            LyricsOutcome { id: "d".into(), kind: LyricsOutcomeKind::NotFound },
            LyricsOutcome { id: "e".into(), kind: LyricsOutcomeKind::NotFound },
        ];
        let counts = tally(&outcomes);
        assert_eq!(counts, LyricsBatchCounts { synced: 1, plain: 1, skipped: 1, not_found: 2 });
    }

    #[test]
    fn metric_labels_match_r15_3s_pill_names() {
        assert_eq!(LyricsOutcomeKind::Synced(String::new()).metric_label(), "Synced .LRC");
        assert_eq!(LyricsOutcomeKind::Plain(String::new()).metric_label(), "Plain Text");
        assert_eq!(LyricsOutcomeKind::Skipped.metric_label(), "Skipped");
        assert_eq!(LyricsOutcomeKind::NotFound.metric_label(), "Not Found");
    }

    #[test]
    fn already_has_lyrics_is_skipped_without_a_network_call() {
        // Exercises the real async path end-to-end for the one outcome that never needs the
        // network, so `fetch_batch_async`'s plumbing (runtime, semaphore, channel) is verified
        // without depending on internet access in this environment.
        let requests = vec![LyricsRequest {
            id: "a".into(),
            title: "T".into(),
            artist: "A".into(),
            album: "Al".into(),
            duration_seconds: 100.0,
            already_has_lyrics: true,
        }];
        let rx = fetch_batch_async(requests, 2);
        let progress = rx.recv().unwrap();
        assert_eq!(progress.last.kind, LyricsOutcomeKind::Skipped);
        assert_eq!(progress.completed, 1);
        assert_eq!(progress.total, 1);
    }
}
