//! Backup & Restore view for the iced shell, ported from `crate::views::backup_restore`
//! (`BackupRestoreView.swift`). Reuses that module's pure types/logic unchanged —
//! `BackupRestoreMode`, `AlbumFilter`, `TrackFilter`, `scope_label`, `ALL_SCOPES`,
//! `BackupProgressInfo`, `BackupRestoreCommand`, [`BackupRestoreViewState`],
//! `resolve_selected_album_ids`, `resolve_selected_track_ids` — and only rewrites
//! `render` as this module's `view`/`update`. The egui version already documents its own
//! simplification (destination/source is a text field + "Browse…" command rather than a
//! native folder picker — the real `platform::FileDialogs` wiring is a later concern),
//! which this port keeps as-is.
//!
//! Scope/filter pickers are button rows (consistent with the other ported views'
//! dropdown simplification); the album/track checklists are real `checkbox` widgets.

use iced::widget::{button, checkbox, column, progress_bar, row, scrollable, text, text_input};
use iced::{Element, Length};

use walkman_domain::models::Track;
use walkman_services::backup_restore::BackupScope;

use crate::iced_ui::theme::{color, font, sizing};
use crate::views::albums::group_into_albums;
use crate::views::backup_restore::{
    resolve_selected_album_ids, resolve_selected_track_ids, scope_label, AlbumFilter, BackupRestoreCommand, BackupRestoreMode,
    BackupRestoreViewState, TrackFilter, ALL_SCOPES,
};

#[derive(Debug, Clone)]
pub enum Message {
    ModeSelected(BackupRestoreMode),
    ScopeSelected(BackupScope),
    AlbumFilterSelected(AlbumFilter),
    TrackFilterSelected(TrackFilter),
    AlbumToggled(String, bool),
    TrackToggled(String, bool),
    DestinationChanged(String),
    RestoreSourceChanged(String),
    BrowseClicked,
    EncryptToggled(bool),
    PassphraseChanged(String),
    StartBackupClicked,
    StartRestoreClicked,
    CancelClicked,
}

pub fn update(tracks: &[Track], state: &mut BackupRestoreViewState, message: Message) {
    match message {
        Message::ModeSelected(mode) => state.mode = mode,
        Message::ScopeSelected(scope) => state.scope = scope,
        Message::AlbumFilterSelected(filter) => state.album_filter = filter,
        Message::TrackFilterSelected(filter) => state.track_filter = filter,
        Message::AlbumToggled(id, checked) => {
            if checked {
                state.selected_album_ids.push(id);
            } else {
                state.selected_album_ids.retain(|a| a != &id);
            }
        }
        Message::TrackToggled(id, checked) => {
            if checked {
                state.selected_track_ids.push(id);
            } else {
                state.selected_track_ids.retain(|t| t != &id);
            }
        }
        Message::DestinationChanged(value) => state.destination = value,
        Message::RestoreSourceChanged(value) => state.restore_source = value,
        Message::BrowseClicked => {
            let command = if state.mode == BackupRestoreMode::Backup {
                BackupRestoreCommand::PickDestination
            } else {
                BackupRestoreCommand::PickRestoreSource
            };
            state.commands.push(command);
        }
        Message::EncryptToggled(value) => state.encrypt = value,
        Message::PassphraseChanged(value) => state.passphrase = value,
        Message::StartBackupClicked => {
            state.commands.push(BackupRestoreCommand::StartBackup {
                scope: state.scope,
                selected_album_ids: resolve_selected_album_ids(tracks, state.album_filter, &state.selected_album_ids),
                selected_track_ids: resolve_selected_track_ids(tracks, state.track_filter, &state.selected_track_ids),
                destination: state.destination.clone(),
                passphrase: state.encrypt.then(|| state.passphrase.clone()),
            });
        }
        Message::StartRestoreClicked => {
            state.commands.push(BackupRestoreCommand::StartRestore {
                source: state.restore_source.clone(),
                passphrase: state.encrypt.then(|| state.passphrase.clone()),
            });
        }
        Message::CancelClicked => state.commands.push(BackupRestoreCommand::Cancel),
    }
}

pub fn view<'a>(tracks: &'a [Track], state: &'a BackupRestoreViewState) -> Element<'a, Message> {
    let mode_row = row![
        mode_chip(BackupRestoreMode::Backup, "Backup Walkman", state.mode),
        mode_chip(BackupRestoreMode::Restore, "Restore to Walkman", state.mode),
    ]
    .spacing(6);

    let form = match state.mode {
        BackupRestoreMode::Backup => backup_form(tracks, state),
        BackupRestoreMode::Restore => restore_form(state),
    };

    let mut content = column![mode_row, form].spacing(12);
    if let Some(progress) = &state.progress {
        content = content.push(progress_section(progress));
    }

    scrollable(content.padding(16)).height(Length::Fill).into()
}

fn mode_chip<'a>(mode: BackupRestoreMode, label: &'static str, selected: BackupRestoreMode) -> Element<'a, Message> {
    let active = mode == selected;
    button(text(label).size(font::CALLOUT.size))
        .padding([6, 12])
        .on_press(Message::ModeSelected(mode))
        .style(move |_theme, status| chip_style(active, status))
        .into()
}

fn chip_style(active: bool, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let fill = if active {
        color::with_alpha(iced::Color::WHITE, 0.14)
    } else if hovered {
        color::with_alpha(iced::Color::WHITE, 0.06)
    } else {
        color::with_alpha(color::CARD, 0.55)
    };
    button::Style {
        background: Some(iced::Background::Color(fill)),
        text_color: if active { color::SILVER_LIGHT } else { color::SILVER_MUTED },
        border: iced::Border {
            color: color::BORDER,
            width: 1.0,
            radius: sizing::radius::CAPSULE.into(),
        },
        ..button::Style::default()
    }
}

fn section_label<'a>(label: &'static str) -> Element<'a, Message> {
    text(label).size(font::CAPTION_BOLD.size).color(color::SILVER_MUTED).into()
}

fn backup_form<'a>(tracks: &'a [Track], state: &'a BackupRestoreViewState) -> Element<'a, Message> {
    let scope_row = ALL_SCOPES.iter().fold(row![].spacing(4), |r, &scope| {
        let active = state.scope == scope;
        r.push(
            button(text(scope_label(scope)).size(font::CAPTION.size))
                .padding([4, 8])
                .on_press(Message::ScopeSelected(scope))
                .style(move |_theme, status| chip_style(active, status)),
        )
    });

    let mut col = column![section_label("Scope"), scope_row].spacing(6);

    match state.scope {
        BackupScope::SelectedAlbums => col = col.push(album_filter_section(tracks, state)),
        BackupScope::SelectedTracks => col = col.push(track_filter_section(tracks, state)),
        _ => {}
    }

    col = col.push(destination_row("Destination", &state.destination, Message::DestinationChanged));
    col = col.push(passphrase_section(state));

    let selection_required_but_empty = (state.scope == BackupScope::SelectedAlbums
        && state.album_filter == AlbumFilter::Selected
        && state.selected_album_ids.is_empty())
        || (state.scope == BackupScope::SelectedTracks && state.track_filter == TrackFilter::Selected && state.selected_track_ids.is_empty());
    let can_start = !state.destination.trim().is_empty() && !selection_required_but_empty;

    let mut start = button(text("Start Backup").size(font::CALLOUT.size)).padding([8, 16]);
    if can_start {
        start = start.on_press(Message::StartBackupClicked);
    }
    col.push(start.style(|_theme, status| prominent_style(status))).into()
}

fn album_filter_section<'a>(tracks: &'a [Track], state: &'a BackupRestoreViewState) -> Element<'a, Message> {
    let filter_row = row![
        filter_chip("All Albums", state.album_filter == AlbumFilter::AllAlbums, Message::AlbumFilterSelected(AlbumFilter::AllAlbums)),
        filter_chip("Selected", state.album_filter == AlbumFilter::Selected, Message::AlbumFilterSelected(AlbumFilter::Selected)),
        filter_chip("Hi-Res Only", state.album_filter == AlbumFilter::HiRes, Message::AlbumFilterSelected(AlbumFilter::HiRes)),
    ]
    .spacing(6);

    let mut col = column![filter_row].spacing(6);
    if state.album_filter == AlbumFilter::Selected {
        let list = group_into_albums(tracks).into_iter().fold(column![].spacing(4), |col, album| {
            let id = album.id();
            let checked = state.selected_album_ids.contains(&id);
            col.push(checkbox(format!("{} \u{2014} {}", album.name, album.artist), checked).on_toggle(move |v| Message::AlbumToggled(id.clone(), v)))
        });
        col = col.push(scrollable(list).height(Length::Fixed(160.0)));
    }
    col.into()
}

fn track_filter_section<'a>(tracks: &'a [Track], state: &'a BackupRestoreViewState) -> Element<'a, Message> {
    let filter_row = row![
        filter_chip("All Tracks", state.track_filter == TrackFilter::AllTracks, Message::TrackFilterSelected(TrackFilter::AllTracks)),
        filter_chip("Selected", state.track_filter == TrackFilter::Selected, Message::TrackFilterSelected(TrackFilter::Selected)),
        filter_chip("Hi-Res Only", state.track_filter == TrackFilter::HiRes, Message::TrackFilterSelected(TrackFilter::HiRes)),
    ]
    .spacing(6);

    let mut col = column![filter_row].spacing(6);
    if state.track_filter == TrackFilter::Selected {
        let list = tracks.iter().fold(column![].spacing(4), |col, track| {
            let id = track.id().to_owned();
            let checked = state.selected_track_ids.contains(&id);
            col.push(checkbox(format!("{} \u{2014} {}", track.title, track.artist), checked).on_toggle(move |v| Message::TrackToggled(id.clone(), v)))
        });
        col = col.push(scrollable(list).height(Length::Fixed(160.0)));
    }
    col.into()
}

fn filter_chip<'a>(label: &'static str, active: bool, message: Message) -> Element<'a, Message> {
    button(text(label).size(font::CALLOUT.size))
        .padding([6, 12])
        .on_press(message)
        .style(move |_theme, status| chip_style(active, status))
        .into()
}

fn restore_form(state: &BackupRestoreViewState) -> Element<'_, Message> {
    let mut col = column![destination_row("Backup Source", &state.restore_source, Message::RestoreSourceChanged), passphrase_section(state)].spacing(10);

    let can_start = !state.restore_source.trim().is_empty();
    let mut start = button(text("Start Restore").size(font::CALLOUT.size)).padding([8, 16]);
    if can_start {
        start = start.on_press(Message::StartRestoreClicked);
    }
    col = col.push(start.style(|_theme, status| prominent_style(status)));
    col.into()
}

fn destination_row<'a>(label: &'static str, value: &'a str, on_change: impl Fn(String) -> Message + 'a) -> Element<'a, Message> {
    column![
        section_label(label),
        row![
            text_input("Local PC, external drive, or custom folder", value).on_input(on_change).padding(8).size(font::BODY.size).width(Length::Fixed(360.0)),
            button(text("Browse\u{2026}").size(font::CALLOUT.size)).padding([6, 12]).on_press(Message::BrowseClicked).style(|_theme, status| chip_style(false, status)),
        ]
        .spacing(8),
    ]
    .spacing(4)
    .into()
}

fn passphrase_section(state: &BackupRestoreViewState) -> Element<'_, Message> {
    let mut col = column![checkbox("Encrypt with a passphrase (AES-256-GCM)", state.encrypt).on_toggle(Message::EncryptToggled)].spacing(6);
    if state.encrypt {
        col = col.push(
            text_input("Passphrase", &state.passphrase)
                .secure(true)
                .on_input(Message::PassphraseChanged)
                .padding(8)
                .size(font::BODY.size)
                .width(Length::Fixed(260.0)),
        );
    }
    col.into()
}

fn progress_section(progress: &crate::views::backup_restore::BackupProgressInfo) -> Element<'_, Message> {
    let fraction = if progress.total == 0 { 0.0 } else { progress.completed as f32 / progress.total as f32 };
    column![
        text(progress.current_file.as_str()).size(font::CALLOUT.size).color(color::SILVER_MUTED),
        progress_bar(0.0..=1.0, fraction),
        text(format!("{} / {}", progress.completed, progress.total)).size(font::CAPTION.size).color(color::SILVER_MUTED),
        button(text("Cancel").size(font::CALLOUT.size)).padding([6, 12]).on_press(Message::CancelClicked).style(|_theme, status| destructive_style(status)),
    ]
    .spacing(8)
    .into()
}

fn prominent_style(status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(iced::Background::Color(if hovered { color::lighten(color::SILVER, 0.08) } else { color::SILVER })),
        text_color: color::OBSIDIAN,
        border: iced::Border {
            color: color::with_alpha(iced::Color::WHITE, 0.25),
            width: 1.0,
            radius: sizing::radius::BUTTON.into(),
        },
        ..button::Style::default()
    }
}

fn destructive_style(status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(iced::Background::Color(color::with_alpha(color::DESTRUCTIVE, if hovered { 0.95 } else { 0.85 }))),
        text_color: color::SILVER_LIGHT,
        border: iced::Border {
            color: color::with_alpha(color::DESTRUCTIVE_LIGHT, 0.35),
            width: 1.0,
            radius: sizing::radius::BUTTON.into(),
        },
        ..button::Style::default()
    }
}
