//! Core data models, ported from `Sources/WalkmanStudio/Models.swift`.
//!
//! Spec task 5.1. Full field parity with the Swift structs; derived properties are
//! exposed as methods. Timestamps use `chrono::DateTime<Utc>` in place of `Date`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Which storage partition a drive represents. Ported from Swift `DriveType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DriveType {
    Internal,
    Microsd,
    Local,
}

/// Transfer destination root. Ported from Swift `TransferTargetMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransferTargetMode {
    Music,
    Learning,
}

impl TransferTargetMode {
    pub const ALL: [TransferTargetMode; 2] = [TransferTargetMode::Music, TransferTargetMode::Learning];

    pub fn title(self) -> &'static str {
        match self {
            TransferTargetMode::Music => "Music Library",
            TransferTargetMode::Learning => "Language Study & Audiobooks",
        }
    }

    /// Root folder name on the device.
    pub fn root_folder_name(self) -> &'static str {
        match self {
            TransferTargetMode::Music => "MUSIC",
            TransferTargetMode::Learning => "LEARNING",
        }
    }

    /// SF-Symbol name in the macOS app; mapped to a bundled glyph in the UI layer.
    pub fn icon(self) -> &'static str {
        match self {
            TransferTargetMode::Music => "music.note",
            TransferTargetMode::Learning => "character.book.closed.fill",
        }
    }
}

/// A single audio track. Ported from Swift `Track`. `id` is the full path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub filename: String,
    pub relative_path: String,
    pub full_path: String,
    /// "FLAC", "MP3", "M4A", etc.
    pub format: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track_num: String,
    pub year: String,
    pub genre: String,
    pub duration_seconds: f64,
    pub bitrate_kbps: i32,
    pub sample_rate: i32,
    pub bits_per_sample: i32,
    pub has_artwork: bool,
    pub size_bytes: i64,
    pub date_added: DateTime<Utc>,
    pub is_senseme_analyzed: bool,
    pub senseme_bpm: Option<f64>,
    pub senseme_valence: Option<f64>,
    pub senseme_arousal: Option<f64>,
    pub senseme_channels: Vec<String>,
    pub senseme_highlight_sec: Option<f64>,
}

impl Track {
    /// Stable identity (full path), matching Swift `Track.id`.
    pub fn id(&self) -> &str {
        &self.full_path
    }

    /// Hi-Res = exceeds standard 16-bit / 44.1-48kHz CD quality.
    pub fn is_hi_res(&self) -> bool {
        self.bits_per_sample > 16 || self.sample_rate > 48_000
    }

    /// Belongs to Language Study (under /LEARNING/ or a language/audiobook/podcast genre).
    pub fn is_language_study(&self) -> bool {
        if self.full_path.contains("/LEARNING/") || self.full_path.contains("\\LEARNING\\") {
            return true;
        }
        if self.genre.is_empty() {
            return false;
        }
        let g = self.genre.to_lowercase();
        g.contains("language") || g.contains("audiobook") || g.contains("podcast") || g.contains("learning")
    }

    /// Primary artist (featured/guest artists stripped). Delegates to `artist_parse`.
    pub fn primary_artist(&self) -> String {
        crate::artist_parse::parse_primary_and_featured(&self.artist).primary
    }

    /// Featured/guest artists if present. Delegates to `artist_parse`.
    pub fn featured_artist(&self) -> Option<String> {
        crate::artist_parse::parse_primary_and_featured(&self.artist).featured
    }
}

/// A playlist (`.m3u8`). Ported from Swift `Playlist`. `id` is the full path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    pub name: String,
    pub filename: String,
    pub track_count: usize,
    /// Relative POSIX track paths.
    pub tracks: Vec<String>,
    pub full_path: String,
}

impl Playlist {
    pub fn is_language_study(&self) -> bool {
        self.full_path.contains("/LEARNING/") || self.full_path.contains("\\LEARNING\\")
    }
}

/// Status of a queued transfer item. Ported from Swift `TransferStatus`.
#[derive(Debug, Clone, PartialEq)]
pub enum TransferStatus {
    Pending,
    Optimizing,
    Converting,
    Success,
    Skipped(String),
    Failed(String),
}

/// A user-registered local music folder. Ported from Swift `LocalMusicFolder`.
///
/// `url`/`exists_on_disk` are intentionally omitted — they require filesystem access,
/// which lives in the platform/services layers, not the pure domain crate.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocalMusicFolder {
    pub id: String,
    pub path: String,
    pub display_name: String,
    pub date_added: DateTime<Utc>,
    pub last_scanned: DateTime<Utc>,
    pub track_count: usize,
    pub total_bytes: i64,
    pub is_missing: bool,
}

/// A single mounted storage partition on a Walkman. Ported from Swift `WalkmanDrive`.
/// `id` is the mount point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WalkmanDrive {
    pub name: String,
    pub mount_point: String,
    #[serde(rename = "type")]
    pub drive_type: DriveType,
    pub total_bytes: i64,
    pub used_bytes: i64,
    pub free_bytes: i64,
    pub percent_used: f64,
    pub has_cfw: bool,
    pub music_dir: String,
}

impl WalkmanDrive {
    /// Stable identity (mount point), matching Swift `WalkmanDrive.id`.
    pub fn id(&self) -> &str {
        &self.mount_point
    }

    /// Absolute path of the `LEARNING` folder on this drive. Ported from Swift
    /// `WalkmanDrive.learningDir`. Joins using the drive's own path separator so the
    /// layout is correct on both Windows (`\`) and POSIX (`/`) mount points.
    pub fn learning_dir(&self) -> String {
        join_path(&self.mount_point, "LEARNING")
    }
}

/// Immutable hardware/firmware profile of a Walkman. Ported from Swift `WalkmanDeviceInfo`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WalkmanDeviceInfo {
    /// e.g. "NW-A50Series"
    pub model_raw: String,
    /// e.g. "WALKMAN NW-A50Series"
    pub marketing_name: String,
    /// e.g. "Sony Walkman NW-A55L"
    pub display_name: String,
    /// e.g. "NW-A50 Series"
    pub series_name: String,
    /// e.g. "NW-A55L"
    pub specific_model: String,
    /// e.g. "Sony Corporation"
    pub vendor: String,
    /// e.g. "1.02"
    pub firmware_version: String,
    pub is_non_android: bool,
    pub has_cfw: bool,
    /// e.g. "800x480"
    pub display_resolution: String,
    pub internal_capacity_gb: f64,
    pub supported_audio_formats: Vec<String>,
}

impl WalkmanDeviceInfo {
    /// Android-based Walkman (W-series Android OS) vs. a Sony Linux DAP.
    pub fn is_android(&self) -> bool {
        !self.is_non_android
    }

    /// Human-readable OS family label. Ported from Swift `WalkmanDeviceInfo.osFamily`.
    pub fn os_family(&self) -> &'static str {
        if self.is_non_android {
            "Sony OS (Linux DAP)"
        } else {
            "Android Walkman OS"
        }
    }
}

/// A detected Walkman device and its mounted drives. Ported from Swift `WalkmanDevice`.
/// `id` is the primary mount point. Capacity totals aggregate across all drives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WalkmanDevice {
    pub device_info: WalkmanDeviceInfo,
    pub drives: Vec<WalkmanDrive>,
}

impl WalkmanDevice {
    pub fn new(device_info: WalkmanDeviceInfo, drives: Vec<WalkmanDrive>) -> Self {
        Self { device_info, drives }
    }

    /// Internal storage partition, if mounted.
    pub fn internal_drive(&self) -> Option<&WalkmanDrive> {
        self.drives.iter().find(|d| d.drive_type == DriveType::Internal)
    }

    /// microSD card partition, if present.
    pub fn microsd_drive(&self) -> Option<&WalkmanDrive> {
        self.drives.iter().find(|d| d.drive_type == DriveType::Microsd)
    }

    /// Primary mount point: internal, else microSD, else the model string.
    /// Also serves as the stable identity (Swift `WalkmanDevice.id`).
    pub fn primary_mount_point(&self) -> String {
        if let Some(d) = self.internal_drive() {
            d.mount_point.clone()
        } else if let Some(d) = self.microsd_drive() {
            d.mount_point.clone()
        } else {
            self.device_info.specific_model.clone()
        }
    }

    /// Stable identity (primary mount point).
    pub fn id(&self) -> String {
        self.primary_mount_point()
    }

    pub fn display_name(&self) -> &str {
        &self.device_info.display_name
    }

    pub fn specific_model(&self) -> &str {
        &self.device_info.specific_model
    }

    pub fn series_name(&self) -> &str {
        &self.device_info.series_name
    }

    /// Sum of every drive's total capacity.
    pub fn total_bytes(&self) -> i64 {
        self.drives.iter().map(|d| d.total_bytes).sum()
    }

    /// Sum of every drive's used space.
    pub fn used_bytes(&self) -> i64 {
        self.drives.iter().map(|d| d.used_bytes).sum()
    }

    /// Sum of every drive's free space.
    pub fn free_bytes(&self) -> i64 {
        self.drives.iter().map(|d| d.free_bytes).sum()
    }

    /// Used percentage across the whole device (0.0 when empty).
    pub fn percent_used(&self) -> f64 {
        let total = self.total_bytes();
        if total > 0 {
            (self.used_bytes() as f64 / total as f64) * 100.0
        } else {
            0.0
        }
    }
}

/// An album grouping of tracks. Ported from Swift `AlbumItem`.
/// `id` is `"{name}|{artist}"` lowercased. Derived flags are computed from the member
/// tracks at construction time (see [`AlbumItem::new`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlbumItem {
    pub name: String,
    pub artist: String,
    pub year: String,
    pub track_count: usize,
    pub tracks: Vec<Track>,
    pub latest_date_added: DateTime<Utc>,
    pub is_hi_res: bool,
    pub is_language_study: bool,
}

impl AlbumItem {
    /// Build an album, deriving `latest_date_added`, `is_hi_res`, and `is_language_study`
    /// from the member tracks (matching the Swift initializer's defaults).
    pub fn new(name: String, artist: String, year: String, track_count: usize, tracks: Vec<Track>) -> Self {
        let latest_date_added = tracks
            .iter()
            .map(|t| t.date_added)
            .max()
            .unwrap_or_else(distant_past);
        let is_hi_res = tracks.iter().any(|t| t.bits_per_sample > 16 || t.sample_rate > 48_000);
        let is_language_study = tracks.iter().any(|t| t.is_language_study());
        Self {
            name,
            artist,
            year,
            track_count,
            tracks,
            latest_date_added,
            is_hi_res,
            is_language_study,
        }
    }

    /// Stable identity, matching Swift `AlbumItem.id`.
    pub fn id(&self) -> String {
        format!("{}|{}", self.name.to_lowercase(), self.artist.to_lowercase())
    }

    pub fn representative_track(&self) -> Option<&Track> {
        self.tracks.first()
    }

    /// Format of the first track, defaulting to "FLAC".
    pub fn primary_format(&self) -> String {
        self.tracks.first().map(|t| t.format.clone()).unwrap_or_else(|| "FLAC".into())
    }
}

/// An artist grouping of albums and tracks. Ported from Swift `ArtistItem`.
/// `id` is the artist name. Derived flags are computed at construction time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtistItem {
    pub name: String,
    pub album_count: usize,
    pub track_count: usize,
    pub albums: Vec<AlbumItem>,
    pub tracks: Vec<Track>,
    pub representative_track: Option<Track>,
    pub is_language_study: bool,
    pub is_hi_res: bool,
}

impl ArtistItem {
    /// Build an artist, deriving the representative track (first with artwork, else first)
    /// and the `is_language_study` / `is_hi_res` flags from the member tracks.
    pub fn new(
        name: String,
        album_count: usize,
        track_count: usize,
        albums: Vec<AlbumItem>,
        tracks: Vec<Track>,
    ) -> Self {
        let representative_track = tracks
            .iter()
            .find(|t| t.has_artwork)
            .or_else(|| tracks.first())
            .cloned();
        let is_language_study = tracks.iter().any(|t| t.is_language_study());
        let is_hi_res = tracks.iter().any(|t| t.bits_per_sample > 16 || t.sample_rate > 48_000);
        Self {
            name,
            album_count,
            track_count,
            albums,
            tracks,
            representative_track,
            is_language_study,
            is_hi_res,
        }
    }

    /// Stable identity (artist name), matching Swift `ArtistItem.id`.
    pub fn id(&self) -> &str {
        &self.name
    }
}

/// A queued transfer item with editable metadata and in-flight UI state.
/// Ported from Swift `TransferItem`. `id` is caller-supplied (the Swift version uses a
/// `UUID`); `source_path` replaces the Swift `sourceURL`, and `artwork_data` holds raw
/// image bytes in place of Swift `Data`.
#[derive(Debug, Clone, PartialEq)]
pub struct TransferItem {
    pub id: String,
    pub source_path: String,
    pub status: TransferStatus,

    // Metadata
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track_number: String,
    pub year: String,
    pub genre: String,
    pub duration_seconds: f64,
    pub bitrate_kbps: i32,
    pub sample_rate: i32,
    pub bits_per_sample: i32,
    pub format: String,
    pub size_bytes: i64,

    // Artwork & optimization
    pub artwork_data: Option<Vec<u8>>,
    pub is_artwork_optimized: bool,

    // Lyrics
    pub lyrics: Option<String>,
    pub is_lyrics_synced: bool,

    // UI & in-flight state
    pub is_fetching_online: bool,
    pub is_optimizing: bool,
    pub is_expanded: bool,
    pub is_duplicate_on_walkman: bool,
    pub duplicate_match_info: Option<String>,
    pub duplicate_matched_track: Option<Track>,
}

impl TransferItem {
    /// File name component of `source_path`, matching Swift `TransferItem.filename`.
    pub fn filename(&self) -> &str {
        last_path_component(&self.source_path)
    }

    /// Converts this item into a preview `Track` compatible with the audio player.
    /// Ported from Swift `TransferItem.toPreviewTrack()`.
    pub fn to_preview_track(&self) -> Track {
        let ext = path_extension(&self.source_path).to_uppercase();
        let format = if self.format.is_empty() { ext.clone() } else { self.format.clone() };
        let filename = self.filename().to_string();
        let title = if self.title.is_empty() {
            file_stem(&self.source_path).to_string()
        } else {
            self.title.clone()
        };
        Track {
            filename: filename.clone(),
            relative_path: filename,
            full_path: self.source_path.clone(),
            format,
            title,
            artist: if self.artist.is_empty() { "Unknown Artist".into() } else { self.artist.clone() },
            album: if self.album.is_empty() { "Unknown Album".into() } else { self.album.clone() },
            track_num: self.track_number.clone(),
            year: self.year.clone(),
            genre: self.genre.clone(),
            duration_seconds: self.duration_seconds,
            bitrate_kbps: self.bitrate_kbps,
            sample_rate: self.sample_rate,
            bits_per_sample: self.bits_per_sample,
            has_artwork: self.artwork_data.is_some(),
            size_bytes: self.size_bytes,
            date_added: Utc::now(),
            is_senseme_analyzed: false,
            senseme_bpm: None,
            senseme_valence: None,
            senseme_arousal: None,
            senseme_channels: vec![],
            senseme_highlight_sec: None,
        }
    }

    /// Whether crucial metadata fields are missing or placeholder.
    /// Ported from Swift `TransferItem.isMissingMetadata`.
    pub fn is_missing_metadata(&self) -> bool {
        is_placeholder(&self.title, &["unknown", "unknown track", "unknown title"])
            || is_placeholder(&self.artist, &["unknown artist", "unknown"])
            || is_placeholder(&self.album, &["unknown album", "unknown"])
            || self.artwork_data.is_none()
    }
}

/// Plain audio-file metadata read from a tag reader. Ported from Swift `AudioFileMetadata`.
/// Fields mirror the Swift defaults (`sample_rate` 44100, `bits_per_sample` 16).
#[derive(Debug, Clone, PartialEq)]
pub struct AudioFileMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track_number: String,
    pub year: String,
    pub genre: String,
    pub duration_seconds: f64,
    pub bitrate_kbps: i32,
    pub sample_rate: i32,
    pub bits_per_sample: i32,
    pub has_artwork: bool,
    pub lyrics: Option<String>,
}

impl Default for AudioFileMetadata {
    fn default() -> Self {
        Self {
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            track_number: String::new(),
            year: String::new(),
            genre: String::new(),
            duration_seconds: 0.0,
            bitrate_kbps: 0,
            sample_rate: 44_100,
            bits_per_sample: 16,
            has_artwork: false,
            lyrics: None,
        }
    }
}

// --- Path helpers (pure string ops; no filesystem access) -------------------
// The domain crate has no I/O. These mirror the small slices of Foundation's `URL`
// behaviour the ported models rely on, handling both `/` and `\` separators.

/// Join `base` and `component` with the separator already used by `base`
/// (defaults to `/` when `base` has none).
fn join_path(base: &str, component: &str) -> String {
    let sep = if base.contains('\\') && !base.contains('/') { '\\' } else { '/' };
    let trimmed = base.trim_end_matches(['/', '\\']);
    format!("{trimmed}{sep}{component}")
}

/// Final path component of `path` (handles `/` and `\`).
fn last_path_component(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Uppercase-free extension (without the dot) of the final path component.
fn path_extension(path: &str) -> &str {
    let name = last_path_component(path);
    match name.rfind('.') {
        Some(idx) if idx > 0 => &name[idx + 1..],
        _ => "",
    }
}

/// Final path component with its extension removed.
fn file_stem(path: &str) -> &str {
    let name = last_path_component(path);
    match name.rfind('.') {
        Some(idx) if idx > 0 => &name[..idx],
        _ => name,
    }
}

/// Trimmed-and-lowercased emptiness/placeholder check for metadata fields.
fn is_placeholder(value: &str, placeholders: &[&str]) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return true;
    }
    let lower = trimmed.to_lowercase();
    placeholders.contains(&lower.as_str())
}

/// The epoch used when a collection has no dated tracks (Swift `Date.distantPast`).
fn distant_past() -> DateTime<Utc> {
    DateTime::<Utc>::MIN_UTC
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample(path: &str, bits: i32, rate: i32) -> Track {
        Track {
            filename: "x".into(),
            relative_path: "x".into(),
            full_path: path.into(),
            format: "FLAC".into(),
            title: "t".into(),
            artist: "a".into(),
            album: "al".into(),
            track_num: "1".into(),
            year: "2020".into(),
            genre: String::new(),
            duration_seconds: 10.0,
            bitrate_kbps: 900,
            sample_rate: rate,
            bits_per_sample: bits,
            has_artwork: false,
            size_bytes: 1,
            date_added: Utc::now(),
            is_senseme_analyzed: false,
            senseme_bpm: None,
            senseme_valence: None,
            senseme_arousal: None,
            senseme_channels: vec![],
            senseme_highlight_sec: None,
        }
    }

    #[test]
    fn hi_res_detection() {
        assert!(sample("/a.flac", 24, 96_000).is_hi_res());
        assert!(sample("/a.flac", 16, 96_000).is_hi_res());
        assert!(!sample("/a.flac", 16, 44_100).is_hi_res());
    }

    #[test]
    fn language_study_by_path_and_genre() {
        assert!(sample("/Volumes/WALKMAN/LEARNING/x.mp3", 16, 44_100).is_language_study());
        let mut t = sample("/Music/x.mp3", 16, 44_100);
        t.genre = "Audiobook".into();
        assert!(t.is_language_study());
    }

    #[test]
    fn target_mode_roots() {
        assert_eq!(TransferTargetMode::Music.root_folder_name(), "MUSIC");
        assert_eq!(TransferTargetMode::Learning.root_folder_name(), "LEARNING");
    }

    fn drive(ty: DriveType, total: i64, used: i64, free: i64) -> WalkmanDrive {
        WalkmanDrive {
            name: "WALKMAN".into(),
            mount_point: "/Volumes/WALKMAN".into(),
            drive_type: ty,
            total_bytes: total,
            used_bytes: used,
            free_bytes: free,
            percent_used: if total > 0 { used as f64 / total as f64 * 100.0 } else { 0.0 },
            has_cfw: false,
            music_dir: "/Volumes/WALKMAN/MUSIC".into(),
        }
    }

    fn device_info() -> WalkmanDeviceInfo {
        WalkmanDeviceInfo {
            model_raw: "NW-A50Series".into(),
            marketing_name: "WALKMAN NW-A50Series".into(),
            display_name: "Sony Walkman NW-A55L".into(),
            series_name: "NW-A50 Series".into(),
            specific_model: "NW-A55L".into(),
            vendor: "Sony Corporation".into(),
            firmware_version: "1.02".into(),
            is_non_android: true,
            has_cfw: false,
            display_resolution: "800x480".into(),
            internal_capacity_gb: 16.0,
            supported_audio_formats: vec!["FLAC".into(), "MP3".into()],
        }
    }

    #[test]
    fn learning_dir_joins_on_posix_and_windows() {
        let mut d = drive(DriveType::Internal, 0, 0, 0);
        d.mount_point = "/Volumes/WALKMAN".into();
        assert_eq!(d.learning_dir(), "/Volumes/WALKMAN/LEARNING");

        let mut w = drive(DriveType::Internal, 0, 0, 0);
        w.mount_point = "E:\\".into();
        assert_eq!(w.learning_dir(), "E:\\LEARNING");

        let mut w2 = drive(DriveType::Internal, 0, 0, 0);
        w2.mount_point = "E:\\WALKMAN".into();
        assert_eq!(w2.learning_dir(), "E:\\WALKMAN\\LEARNING");
    }

    #[test]
    fn device_os_family_and_android_flag() {
        let mut info = device_info();
        info.is_non_android = true;
        assert!(!info.is_android());
        assert_eq!(info.os_family(), "Sony OS (Linux DAP)");

        info.is_non_android = false;
        assert!(info.is_android());
        assert_eq!(info.os_family(), "Android Walkman OS");
    }

    #[test]
    fn device_totals_aggregate_across_drives() {
        let dev = WalkmanDevice::new(
            device_info(),
            vec![
                drive(DriveType::Internal, 16_000, 6_000, 10_000),
                drive(DriveType::Microsd, 64_000, 4_000, 60_000),
            ],
        );
        assert_eq!(dev.total_bytes(), 80_000);
        assert_eq!(dev.used_bytes(), 10_000);
        assert_eq!(dev.free_bytes(), 70_000);
        assert!((dev.percent_used() - 12.5).abs() < 1e-9);
        assert_eq!(dev.internal_drive().unwrap().drive_type, DriveType::Internal);
        assert_eq!(dev.microsd_drive().unwrap().drive_type, DriveType::Microsd);
    }

    #[test]
    fn device_percent_used_zero_when_empty() {
        let dev = WalkmanDevice::new(device_info(), vec![]);
        assert_eq!(dev.total_bytes(), 0);
        assert_eq!(dev.percent_used(), 0.0);
    }

    #[test]
    fn device_primary_mount_point_prefers_internal_then_microsd_then_model() {
        let internal = drive(DriveType::Internal, 1, 0, 1);
        let mut sd = drive(DriveType::Microsd, 1, 0, 1);
        sd.mount_point = "/Volumes/SD".into();

        let dev = WalkmanDevice::new(device_info(), vec![sd.clone(), internal.clone()]);
        assert_eq!(dev.primary_mount_point(), internal.mount_point);

        let dev_sd = WalkmanDevice::new(device_info(), vec![sd.clone()]);
        assert_eq!(dev_sd.primary_mount_point(), "/Volumes/SD");

        let dev_none = WalkmanDevice::new(device_info(), vec![]);
        assert_eq!(dev_none.primary_mount_point(), "NW-A55L");
    }

    #[test]
    fn album_derives_hi_res_and_language_study_and_latest_date() {
        let early = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        let late = Utc.with_ymd_and_hms(2023, 6, 1, 0, 0, 0).unwrap();

        let mut cd = sample("/Music/x.flac", 16, 44_100);
        cd.date_added = early;
        let mut hires = sample("/Music/y.flac", 24, 96_000);
        hires.date_added = late;

        let album = AlbumItem::new("Album".into(), "Artist".into(), "2020".into(), 2, vec![cd, hires]);
        assert!(album.is_hi_res);
        assert!(!album.is_language_study);
        assert_eq!(album.latest_date_added, late);
        assert_eq!(album.id(), "album|artist");
        assert_eq!(album.primary_format(), "FLAC");

        let lang = {
            let mut t = sample("/LEARNING/lesson.mp3", 16, 44_100);
            t.genre = "Language".into();
            AlbumItem::new("Course".into(), "Tutor".into(), "2021".into(), 1, vec![t])
        };
        assert!(!lang.is_hi_res);
        assert!(lang.is_language_study);
    }

    #[test]
    fn album_latest_date_is_distant_past_when_empty() {
        let album = AlbumItem::new("Empty".into(), "Nobody".into(), "".into(), 0, vec![]);
        assert_eq!(album.latest_date_added, DateTime::<Utc>::MIN_UTC);
        assert!(!album.is_hi_res);
        assert!(album.representative_track().is_none());
        assert_eq!(album.primary_format(), "FLAC");
    }

    #[test]
    fn artist_picks_representative_track_with_artwork() {
        let no_art = sample("/Music/a.flac", 16, 44_100);
        let mut with_art = sample("/Music/b.flac", 24, 96_000);
        with_art.has_artwork = true;

        let artist = ArtistItem::new(
            "Artist".into(),
            1,
            2,
            vec![],
            vec![no_art, with_art.clone()],
        );
        assert_eq!(artist.representative_track.as_ref().unwrap().full_path, with_art.full_path);
        assert!(artist.is_hi_res);
        assert!(!artist.is_language_study);
        assert_eq!(artist.id(), "Artist");
    }

    #[test]
    fn artist_falls_back_to_first_track_without_artwork() {
        let first = sample("/Music/a.flac", 16, 44_100);
        let second = sample("/Music/b.flac", 16, 44_100);
        let artist = ArtistItem::new("Artist".into(), 1, 2, vec![], vec![first.clone(), second]);
        assert_eq!(artist.representative_track.unwrap().full_path, first.full_path);
    }

    #[test]
    fn transfer_item_preview_track_fills_placeholders() {
        let mut item = base_transfer("/src/My Song.flac");
        item.title = String::new();
        item.artist = String::new();
        item.album = String::new();
        item.format = String::new();

        let track = item.to_preview_track();
        assert_eq!(track.title, "My Song");
        assert_eq!(track.artist, "Unknown Artist");
        assert_eq!(track.album, "Unknown Album");
        assert_eq!(track.format, "FLAC");
        assert!(!track.has_artwork);
        assert_eq!(item.filename(), "My Song.flac");
    }

    #[test]
    fn transfer_item_preview_track_keeps_windows_paths() {
        let mut item = base_transfer("C:\\Users\\me\\Music\\Track.mp3");
        item.format = String::new();
        assert_eq!(item.filename(), "Track.mp3");
        let track = item.to_preview_track();
        assert_eq!(track.format, "MP3");
    }

    #[test]
    fn transfer_item_missing_metadata_detection() {
        let mut item = base_transfer("/src/song.flac");
        item.title = "Song".into();
        item.artist = "Artist".into();
        item.album = "Album".into();
        item.artwork_data = Some(vec![1, 2, 3]);
        assert!(!item.is_missing_metadata());

        item.artist = "Unknown Artist".into();
        assert!(item.is_missing_metadata());

        item.artist = "Artist".into();
        item.artwork_data = None;
        assert!(item.is_missing_metadata());

        item.artwork_data = Some(vec![0]);
        item.title = "   ".into();
        assert!(item.is_missing_metadata());
    }

    #[test]
    fn audio_file_metadata_defaults() {
        let m = AudioFileMetadata::default();
        assert_eq!(m.sample_rate, 44_100);
        assert_eq!(m.bits_per_sample, 16);
        assert_eq!(m.bitrate_kbps, 0);
        assert!(m.title.is_empty());
        assert!(m.lyrics.is_none());
    }

    fn base_transfer(source_path: &str) -> TransferItem {
        TransferItem {
            id: "id-1".into(),
            source_path: source_path.into(),
            status: TransferStatus::Pending,
            title: "Title".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            track_number: "1".into(),
            year: "2020".into(),
            genre: String::new(),
            duration_seconds: 10.0,
            bitrate_kbps: 900,
            sample_rate: 44_100,
            bits_per_sample: 16,
            format: "FLAC".into(),
            size_bytes: 123,
            artwork_data: None,
            is_artwork_optimized: false,
            lyrics: None,
            is_lyrics_synced: false,
            is_fetching_online: false,
            is_optimizing: false,
            is_expanded: false,
            is_duplicate_on_walkman: false,
            duplicate_match_info: None,
            duplicate_matched_track: None,
        }
    }
}
