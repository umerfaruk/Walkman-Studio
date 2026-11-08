//! Pure types/logic behind `BackupRestoreView`, ported from
//! `Sources/WalkmanStudio/Views/BackupRestoreView.swift`. The rendering itself lives in
//! `iced_ui::views::backup_restore` (the egui `render` this module used to carry was
//! removed once the iced migration reached parity — see Milestone 8 of the
//! egui-to-iced migration plan).

use walkman_domain::models::Track;
use walkman_services::backup_restore::BackupScope;

use crate::views::albums::group_into_albums;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackupRestoreMode {
    #[default]
    Backup,
    Restore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlbumFilter {
    #[default]
    AllAlbums,
    Selected,
    /// `BackupRestoreView.swift`'s private `AlbumFilter.hiRes` ("Hi-Res Only") — restricts
    /// the Selected-Albums scope's checklist to Hi-Res albums only.
    HiRes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrackFilter {
    #[default]
    AllTracks,
    Selected,
    /// `BackupRestoreView.swift`'s private `TrackFilter.hiRes` ("Hi-Res Only") — restricts
    /// the Selected-Tracks scope's checklist to Hi-Res tracks only.
    HiRes,
}

/// Display label for a scope, matching R12.2's own wording.
pub fn scope_label(scope: BackupScope) -> &'static str {
    match scope {
        BackupScope::FullLibrary => "Full Library",
        BackupScope::Playlists => "Playlists Scope",
        BackupScope::LanguageStudy => "Language Study (/LEARNING)",
        BackupScope::SelectedAlbums => "Selected Albums",
        BackupScope::SelectedTracks => "Selected Tracks",
    }
}

pub const ALL_SCOPES: [BackupScope; 5] = [
    BackupScope::FullLibrary,
    BackupScope::Playlists,
    BackupScope::LanguageStudy,
    BackupScope::SelectedAlbums,
    BackupScope::SelectedTracks,
];

#[derive(Debug, Clone, PartialEq)]
pub struct BackupProgressInfo {
    pub completed: usize,
    pub total: usize,
    pub current_file: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BackupRestoreCommand {
    PickDestination,
    PickRestoreSource,
    StartBackup {
        scope: BackupScope,
        selected_album_ids: Vec<String>,
        selected_track_ids: Vec<String>,
        destination: String,
        passphrase: Option<String>,
    },
    StartRestore { source: String, passphrase: Option<String> },
    Cancel,
}

#[derive(Debug, Clone)]
pub struct BackupRestoreViewState {
    pub mode: BackupRestoreMode,
    pub scope: BackupScope,
    pub album_filter: AlbumFilter,
    pub track_filter: TrackFilter,
    pub selected_album_ids: Vec<String>,
    pub selected_track_ids: Vec<String>,
    pub destination: String,
    pub encrypt: bool,
    pub passphrase: String,
    pub restore_source: String,
    pub progress: Option<BackupProgressInfo>,
    pub commands: Vec<BackupRestoreCommand>,
}

impl Default for BackupRestoreViewState {
    fn default() -> Self {
        Self {
            mode: BackupRestoreMode::default(),
            scope: BackupScope::FullLibrary,
            album_filter: AlbumFilter::default(),
            track_filter: TrackFilter::default(),
            selected_album_ids: Vec::new(),
            selected_track_ids: Vec::new(),
            destination: String::new(),
            encrypt: false,
            passphrase: String::new(),
            restore_source: String::new(),
            progress: None,
            commands: Vec::new(),
        }
    }
}

/// Resolves `filter` into the album-id list `BackupRestoreCommand::StartBackup` carries.
/// `HiRes` is its own selection mode (every Hi-Res album, no manual checklist), matching
/// `BackupRestoreView.swift`'s `AlbumFilter.hiRes` — resolved here into the same id-list
/// shape `Selected`'s manual checklist already produces, so the command itself doesn't need
/// a second field for it.
pub fn resolve_selected_album_ids(tracks: &[Track], filter: AlbumFilter, manually_selected: &[String]) -> Vec<String> {
    if filter == AlbumFilter::HiRes {
        group_into_albums(tracks).iter().filter(|a| a.is_hi_res).map(|a| a.id()).collect()
    } else {
        manually_selected.to_vec()
    }
}

/// Same as [`resolve_selected_album_ids`] for tracks/`TrackFilter`.
pub fn resolve_selected_track_ids(tracks: &[Track], filter: TrackFilter, manually_selected: &[String]) -> Vec<String> {
    if filter == TrackFilter::HiRes {
        tracks.iter().filter(|t| t.is_hi_res()).map(|t| t.id().to_owned()).collect()
    } else {
        manually_selected.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashSet;

    fn sample_track(title: &str, album: &str) -> Track {
        Track {
            filename: format!("{title}.flac"),
            relative_path: format!("Artist/{album}/{title}.flac"),
            full_path: format!("D:/Music/Artist/{album}/{title}.flac"),
            format: "FLAC".into(),
            title: title.into(),
            artist: "Artist".into(),
            album: album.into(),
            track_num: "1".into(),
            year: "2024".into(),
            genre: String::new(),
            duration_seconds: 200.0,
            bitrate_kbps: 900,
            sample_rate: 44_100,
            bits_per_sample: 16,
            has_artwork: false,
            size_bytes: 1,
            date_added: Utc::now(),
            is_senseme_analyzed: false,
            senseme_bpm: None,
            senseme_valence: None,
            senseme_arousal: None,
            senseme_channels: Vec::new(),
            senseme_highlight_sec: None,
        }
    }

    #[test]
    fn scope_label_covers_every_scope_distinctly() {
        let mut seen = HashSet::new();
        for scope in ALL_SCOPES {
            assert!(seen.insert(scope_label(scope)), "duplicate label for {scope:?}");
        }
    }

    #[test]
    fn resolve_selected_album_ids_uses_every_hi_res_album_when_filter_is_hi_res() {
        let tracks = vec![sample_track("Song A", "Album 1"), sample_track("Hi Song", "Hi Album")];
        let mut hi_res = tracks[1].clone();
        hi_res.bits_per_sample = 24;
        hi_res.sample_rate = 96_000;
        let tracks = vec![tracks[0].clone(), hi_res];

        let ids = resolve_selected_album_ids(&tracks, AlbumFilter::HiRes, &[]);
        assert_eq!(ids.len(), 1);
        assert!(ids[0].to_lowercase().contains("hi album"));
    }

    #[test]
    fn resolve_selected_album_ids_uses_the_manual_list_when_filter_is_selected() {
        let tracks = vec![sample_track("Song A", "Album 1")];
        let manual = vec!["some-album-id".to_string()];
        assert_eq!(resolve_selected_album_ids(&tracks, AlbumFilter::Selected, &manual), manual);
    }

    #[test]
    fn resolve_selected_track_ids_uses_every_hi_res_track_when_filter_is_hi_res() {
        let mut hi_res = sample_track("Hi Song", "Album 1");
        hi_res.bits_per_sample = 24;
        hi_res.sample_rate = 96_000;
        let tracks = vec![sample_track("Song A", "Album 1"), hi_res.clone()];

        let ids = resolve_selected_track_ids(&tracks, TrackFilter::HiRes, &[]);
        assert_eq!(ids, vec![hi_res.id().to_owned()]);
    }

    #[test]
    fn default_state_starts_in_backup_mode_unencrypted_scoped_to_full_library() {
        let state = BackupRestoreViewState::default();
        assert_eq!(state.mode, BackupRestoreMode::Backup);
        assert_eq!(state.scope, BackupScope::FullLibrary);
        assert!(!state.encrypt);
        assert!(state.progress.is_none());
    }
}
