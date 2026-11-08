//! `BackupRestoreView`, ported from `Sources/WalkmanStudio/Views/BackupRestoreView.swift`.
//!
//! Spec task **17.4** (requirements 12.1, 12.2, 12.3, 12.5): `Backup Walkman`/`Restore to
//! Walkman` modes, the 5 backup scopes with Album/Track filters, a destination picker,
//! an optional passphrase field, and progress + cancel — over the real
//! `walkman_services::backup_restore::{BackupScope, BackupManifest}` built in task 11.2.
//!
//! ## Honest simplification
//! The destination/restore-source picker ("Local PC storage... connected external USB/SD
//! drives, and custom folders", R12.3) is a text field plus a "Browse…" button that emits a
//! command; the real native folder-picker dialog is `platform::FileDialogs` (task 6.6),
//! wired in by task 18. This view only displays whatever path it's told was picked.

use eframe::egui;

use walkman_domain::models::Track;
use walkman_services::backup_restore::BackupScope;

use crate::views::albums::group_into_albums;
use crate::theme::{button, color, font};

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrackFilter {
    #[default]
    AllTracks,
    Selected,
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

pub fn render(ui: &mut egui::Ui, tracks: &[Track], state: &mut BackupRestoreViewState) {
    render_mode_toggle(ui, state);
    ui.add_space(12.0);

    match state.mode {
        BackupRestoreMode::Backup => render_backup_form(ui, tracks, state),
        BackupRestoreMode::Restore => render_restore_form(ui, state),
    }

    if let Some(progress) = state.progress.clone() {
        ui.add_space(16.0);
        render_progress(ui, &progress, state);
    }
}

fn render_mode_toggle(ui: &mut egui::Ui, state: &mut BackupRestoreViewState) {
    ui.horizontal(|ui| {
        for (mode, label) in [(BackupRestoreMode::Backup, "Backup Walkman"), (BackupRestoreMode::Restore, "Restore to Walkman")] {
            let variant = if state.mode == mode { button::Variant::Capsule } else { button::Variant::Glass };
            if ui.add(button::Button::new(label, variant)).clicked() {
                state.mode = mode;
            }
        }
    });
}

fn render_backup_form(ui: &mut egui::Ui, tracks: &[Track], state: &mut BackupRestoreViewState) {
    ui.label(egui::RichText::new("Scope").font(font::caption_bold()).color(color::SILVER_MUTED));
    egui::ComboBox::from_id_salt("backup_scope")
        .selected_text(scope_label(state.scope))
        .show_ui(ui, |ui| {
            for scope in ALL_SCOPES {
                ui.selectable_value(&mut state.scope, scope, scope_label(scope));
            }
        });
    ui.add_space(10.0);

    match state.scope {
        BackupScope::SelectedAlbums => render_album_filter(ui, tracks, state),
        BackupScope::SelectedTracks => render_track_filter(ui, tracks, state),
        _ => {}
    }

    ui.add_space(10.0);
    render_destination_row(ui, "Destination", true, state);

    ui.add_space(10.0);
    render_passphrase_row(ui, state);

    ui.add_space(16.0);
    let selection_required_but_empty = (state.scope == BackupScope::SelectedAlbums && state.album_filter == AlbumFilter::Selected && state.selected_album_ids.is_empty())
        || (state.scope == BackupScope::SelectedTracks && state.track_filter == TrackFilter::Selected && state.selected_track_ids.is_empty());
    let can_start = !state.destination.trim().is_empty() && !selection_required_but_empty;
    if ui.add(button::Button::new("Start Backup", button::Variant::Prominent).enabled(can_start)).clicked() {
        state.commands.push(BackupRestoreCommand::StartBackup {
            scope: state.scope,
            selected_album_ids: state.selected_album_ids.clone(),
            selected_track_ids: state.selected_track_ids.clone(),
            destination: state.destination.clone(),
            passphrase: state.encrypt.then(|| state.passphrase.clone()),
        });
    }
}

fn render_album_filter(ui: &mut egui::Ui, tracks: &[Track], state: &mut BackupRestoreViewState) {
    ui.horizontal(|ui| {
        for (filter, label) in [(AlbumFilter::AllAlbums, "All Albums"), (AlbumFilter::Selected, "Selected")] {
            let variant = if state.album_filter == filter { button::Variant::Capsule } else { button::Variant::Glass };
            if ui.add(button::Button::new(label, variant)).clicked() {
                state.album_filter = filter;
            }
        }
    });
    if state.album_filter == AlbumFilter::Selected {
        ui.add_space(6.0);
        egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
            for album in group_into_albums(tracks) {
                let id = album.id();
                let mut checked = state.selected_album_ids.contains(&id);
                if ui.checkbox(&mut checked, format!("{} \u{2014} {}", album.name, album.artist)).changed() {
                    if checked {
                        state.selected_album_ids.push(id);
                    } else {
                        state.selected_album_ids.retain(|a| a != &id);
                    }
                }
            }
        });
    }
}

fn render_track_filter(ui: &mut egui::Ui, tracks: &[Track], state: &mut BackupRestoreViewState) {
    ui.horizontal(|ui| {
        for (filter, label) in [(TrackFilter::AllTracks, "All Tracks"), (TrackFilter::Selected, "Selected")] {
            let variant = if state.track_filter == filter { button::Variant::Capsule } else { button::Variant::Glass };
            if ui.add(button::Button::new(label, variant)).clicked() {
                state.track_filter = filter;
            }
        }
    });
    if state.track_filter == TrackFilter::Selected {
        ui.add_space(6.0);
        egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
            for track in tracks {
                let id = track.id().to_owned();
                let mut checked = state.selected_track_ids.contains(&id);
                if ui.checkbox(&mut checked, format!("{} \u{2014} {}", track.title, track.artist)).changed() {
                    if checked {
                        state.selected_track_ids.push(id);
                    } else {
                        state.selected_track_ids.retain(|t| t != &id);
                    }
                }
            }
        });
    }
}

fn render_restore_form(ui: &mut egui::Ui, state: &mut BackupRestoreViewState) {
    render_destination_row(ui, "Backup Source", false, state);
    ui.add_space(10.0);
    render_passphrase_row(ui, state);

    ui.add_space(16.0);
    let can_start = !state.restore_source.trim().is_empty();
    if ui.add(button::Button::new("Start Restore", button::Variant::Prominent).enabled(can_start)).clicked() {
        state.commands.push(BackupRestoreCommand::StartRestore {
            source: state.restore_source.clone(),
            passphrase: state.encrypt.then(|| state.passphrase.clone()),
        });
    }
}

/// Renders the destination/source text field + "Browse…" button, operating on
/// `state.destination` (backup) or `state.restore_source` (`is_backup = false`).
fn render_destination_row(ui: &mut egui::Ui, label: &str, is_backup: bool, state: &mut BackupRestoreViewState) {
    ui.label(egui::RichText::new(label).font(font::caption_bold()).color(color::SILVER_MUTED));
    ui.horizontal(|ui| {
        let field = if is_backup { &mut state.destination } else { &mut state.restore_source };
        ui.add(egui::TextEdit::singleline(field).desired_width(360.0).hint_text("Local PC, external drive, or custom folder"));
        let clicked = ui.add(button::Button::new("Browse\u{2026}", button::Variant::Glass)).clicked();
        if clicked {
            let command = if is_backup { BackupRestoreCommand::PickDestination } else { BackupRestoreCommand::PickRestoreSource };
            state.commands.push(command);
        }
    });
}

fn render_passphrase_row(ui: &mut egui::Ui, state: &mut BackupRestoreViewState) {
    ui.checkbox(&mut state.encrypt, "Encrypt with a passphrase (AES-256-GCM)");
    if state.encrypt {
        ui.add_space(4.0);
        ui.add(egui::TextEdit::singleline(&mut state.passphrase).password(true).hint_text("Passphrase").desired_width(260.0));
    }
}

fn render_progress(ui: &mut egui::Ui, progress: &BackupProgressInfo, state: &mut BackupRestoreViewState) {
    ui.label(egui::RichText::new(&progress.current_file).font(font::callout()).color(color::SILVER_MUTED));
    let fraction = if progress.total == 0 { 0.0 } else { progress.completed as f32 / progress.total as f32 };
    ui.add(egui::ProgressBar::new(fraction).text(format!("{} / {}", progress.completed, progress.total)));
    ui.add_space(8.0);
    if ui.add(button::Button::new("Cancel", button::Variant::Destructive)).clicked() {
        state.commands.push(BackupRestoreCommand::Cancel);
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

    // ---- Pure labels ----

    #[test]
    fn scope_label_covers_every_scope_distinctly() {
        let mut seen = HashSet::new();
        for scope in ALL_SCOPES {
            assert!(seen.insert(scope_label(scope)), "duplicate label for {scope:?}");
        }
    }

    // ---- Default state ----

    #[test]
    fn default_state_starts_in_backup_mode_unencrypted_scoped_to_full_library() {
        let state = BackupRestoreViewState::default();
        assert_eq!(state.mode, BackupRestoreMode::Backup);
        assert_eq!(state.scope, BackupScope::FullLibrary);
        assert!(!state.encrypt);
        assert!(state.progress.is_none());
    }

    // ---- Headless render smoke test ----

    #[test]
    fn backup_restore_view_renders_in_headless_frame_for_every_state() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);

        let tracks = vec![sample_track("Song A", "Album 1"), sample_track("Song B", "Album 2")];

        let mut backup_full = BackupRestoreViewState::default();
        let mut backup_selected_albums =
            BackupRestoreViewState { scope: BackupScope::SelectedAlbums, album_filter: AlbumFilter::Selected, ..BackupRestoreViewState::default() };
        let mut backup_selected_tracks =
            BackupRestoreViewState { scope: BackupScope::SelectedTracks, track_filter: TrackFilter::Selected, ..BackupRestoreViewState::default() };
        let mut restore = BackupRestoreViewState { mode: BackupRestoreMode::Restore, ..BackupRestoreViewState::default() };
        let mut in_progress = BackupRestoreViewState {
            progress: Some(BackupProgressInfo { completed: 3, total: 10, current_file: "Artist/Album/Song.flac".into() }),
            ..BackupRestoreViewState::default()
        };
        let mut encrypted = BackupRestoreViewState { encrypt: true, ..BackupRestoreViewState::default() };

        for state in [&mut backup_full, &mut backup_selected_albums, &mut backup_selected_tracks, &mut restore, &mut in_progress, &mut encrypted] {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render(ui, &tracks, state);
                });
            });
        }
    }
}
