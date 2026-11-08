//! Playlists view for the iced shell, ported from `crate::views::playlists`
//! (`PlaylistsView.swift`). Reuses that module's pure types/logic unchanged —
//! `AddTracksMode`, `tracks_for_add_mode`, `PlaylistsCommand`,
//! [`PlaylistsViewState`], `sync_active_tracks`, `move_selected_to_top`/
//! `move_selected_to_bottom`/`shift_selected`/`remove_selected`, `row_ids` — and only
//! rewrites `render` as this module's `view`/`update`.
//!
//! Documented simplifications: multi-select (ctrl/shift-click) collapses to
//! single-select for the track rows, same as the other ported views; the "New…"/
//! "Rename…"/"Delete" playlist sheets and the "Add Tracks" sheet aren't rendered yet
//! (Milestone 6) — their trigger buttons set `state.sheet` exactly like the egui
//! version. Unlike the egui version (where Rename/Delete live only in a right-click
//! context menu, which iced has no built-in equivalent for), this port puts small
//! Rename/Delete buttons directly on each playlist row so those actions stay reachable.

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length};

use walkman_domain::models::Playlist;

use crate::iced_ui::theme::{color, font, sizing};
use crate::iced_ui::widgets;
use crate::views::playlists::{
    move_selected_to_bottom, move_selected_to_top, remove_selected, shift_selected, sync_active_tracks, AddTracksMode, PlaylistsCommand,
    PlaylistsSheet, PlaylistsViewState,
};

#[derive(Debug, Clone)]
pub enum Message {
    NewPlaylistClicked,
    PlaylistSelected(String),
    RenameClicked(String, String),
    DeleteClicked(String),
    AddTracksClicked,
    SelectAllToggled,
    RowClicked(usize),
    MoveToTopClicked,
    MoveUpClicked,
    MoveDownClicked,
    MoveToBottomClicked,
    RemoveClicked,
}

pub fn update(playlists: &[Playlist], state: &mut PlaylistsViewState, message: Message) {
    sync_active_tracks(playlists, state);

    match message {
        Message::NewPlaylistClicked => state.sheet = PlaylistsSheet::CreatePlaylist { name: String::new() },
        Message::PlaylistSelected(id) => {
            state.selected_playlist_id = Some(id);
            sync_active_tracks(playlists, state);
        }
        Message::RenameClicked(playlist_id, name) => state.sheet = PlaylistsSheet::RenamePlaylist { playlist_id, name },
        Message::DeleteClicked(playlist_id) => state.sheet = PlaylistsSheet::DeleteConfirm { playlist_id },
        Message::AddTracksClicked => {
            state.sheet = PlaylistsSheet::AddTracks { mode: AddTracksMode::default(), search: String::new(), selected: Vec::new() };
        }
        Message::SelectAllToggled => {
            let ids = row_ids(state.active_tracks.len());
            crate::selection::toggle_select_all(&mut state.selected_rows, &ids);
        }
        Message::RowClicked(index) => {
            state.selected_rows.clear();
            state.selected_rows.push(index.to_string());
        }
        Message::MoveToTopClicked => apply_reorder(state, move_selected_to_top),
        Message::MoveUpClicked => apply_reorder(state, |tracks, selected| shift_selected(tracks, selected, -1)),
        Message::MoveDownClicked => apply_reorder(state, |tracks, selected| shift_selected(tracks, selected, 1)),
        Message::MoveToBottomClicked => apply_reorder(state, move_selected_to_bottom),
        Message::RemoveClicked => apply_reorder(state, remove_selected),
    }
}

fn row_ids(len: usize) -> Vec<String> {
    (0..len).map(|i| i.to_string()).collect()
}

fn apply_reorder(state: &mut PlaylistsViewState, f: impl FnOnce(&mut Vec<String>, &[usize])) {
    let Some(playlist_id) = state.selected_playlist_id.clone() else { return };
    let mut selected_indices: Vec<usize> = state.selected_rows.iter().filter_map(|s| s.parse().ok()).collect();
    selected_indices.sort_unstable();
    f(&mut state.active_tracks, &selected_indices);
    state.selected_rows.clear();
    state.selection_anchor = None;
    state.commands.push(PlaylistsCommand::Reorder { playlist_id, relative_paths: state.active_tracks.clone() });
}

pub fn view<'a>(playlists: &'a [Playlist], state: &'a PlaylistsViewState) -> Element<'a, Message> {
    row![playlist_list(playlists, state), track_list(playlists, state)].spacing(16).padding(16).into()
}

fn playlist_list<'a>(playlists: &'a [Playlist], state: &'a PlaylistsViewState) -> Element<'a, Message> {
    let header = row![
        text("Playlists").size(font::TITLE3.size).color(color::SILVER_LIGHT),
        iced::widget::Space::new(Length::Fill, Length::Shrink),
        button(text("New\u{2026}").size(font::CALLOUT.size))
            .padding([6, 12])
            .on_press(Message::NewPlaylistClicked)
            .style(|_theme, status| prominent_style(status)),
    ]
    .align_y(iced::alignment::Vertical::Center);

    let list = playlists.iter().fold(column![].spacing(4), |col, playlist| {
        let selected = state.selected_playlist_id.as_deref() == Some(playlist.full_path.as_str());
        col.push(playlist_row(playlist, selected))
    });

    column![header, scrollable(list).height(Length::Fill)].spacing(8).width(Length::FillPortion(1)).into()
}

fn playlist_row<'a>(playlist: &'a Playlist, selected: bool) -> Element<'a, Message> {
    let path = playlist.full_path.clone();
    let name = playlist.name.clone();

    let info = button(
        column![
            text(playlist.name.as_str()).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
            text(format!("{} track{}", playlist.track_count, if playlist.track_count == 1 { "" } else { "s" }))
                .size(font::CAPTION.size)
                .color(color::SILVER_MUTED),
        ]
        .spacing(2),
    )
    .width(Length::Fill)
    .padding(10)
    .on_press(Message::PlaylistSelected(path.clone()))
    .style(move |_theme, status| row_style(selected, status));

    let actions = row![
        button(text("Rename").size(font::CAPTION.size))
            .padding([2, 6])
            .on_press(Message::RenameClicked(path.clone(), name))
            .style(|_theme, status| chip_style(status)),
        button(text("Delete").size(font::CAPTION.size))
            .padding([2, 6])
            .on_press(Message::DeleteClicked(path))
            .style(|_theme, status| chip_style(status)),
    ]
    .spacing(4);

    container(column![info, actions].spacing(4)).into()
}

fn track_list<'a>(playlists: &'a [Playlist], state: &'a PlaylistsViewState) -> Element<'a, Message> {
    let Some(playlist_id) = &state.selected_playlist_id else {
        return container(text("Select a playlist").size(font::CALLOUT.size).color(color::SILVER_MUTED))
            .width(Length::FillPortion(2))
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into();
    };

    let name = playlists.iter().find(|p| &p.full_path == playlist_id).map(|p| p.name.clone()).unwrap_or_default();

    let header = row![
        text(name).size(font::TITLE3.size).color(color::SILVER_LIGHT),
        iced::widget::Space::new(Length::Fill, Length::Shrink),
        button(text("Add Tracks\u{2026}").size(font::CALLOUT.size))
            .padding([6, 12])
            .on_press(Message::AddTracksClicked)
            .style(|_theme, status| prominent_style(status)),
        button(text("Select").size(font::CALLOUT.size))
            .padding([6, 12])
            .on_press(Message::SelectAllToggled)
            .style(|_theme, status| chip_style(status)),
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center);

    let list = state.active_tracks.iter().enumerate().fold(column![].spacing(4), |col, (index, relative_path)| {
        let selected = state.selected_rows.contains(&index.to_string());
        col.push(track_row(index, relative_path, selected))
    });

    let mut content = column![
        header,
        scrollable(list.padding(iced::Padding {
            top: 0.0,
            right: 0.0,
            bottom: sizing::TABLE_BOTTOM_INSET,
            left: 0.0,
        }))
        .height(Length::Fill),
    ]
    .spacing(8);

    if !state.selected_rows.is_empty() {
        content = content.push(batch_action_bar(state.selected_rows.len()));
    }

    content.width(Length::FillPortion(2)).into()
}

fn track_row<'a>(index: usize, relative_path: &'a str, selected: bool) -> Element<'a, Message> {
    button(text(relative_path).size(font::BODY.size).color(color::SILVER_LIGHT))
        .width(Length::Fill)
        .padding(10)
        .on_press(Message::RowClicked(index))
        .style(move |_theme, status| row_style(selected, status))
        .into()
}

fn batch_action_bar<'a>(count: usize) -> Element<'a, Message> {
    container(
        row![
            text(format!("{count} selected")).size(font::CAPTION_BOLD.size).color(color::SILVER_MUTED),
            action_button("Move to Top", Message::MoveToTopClicked),
            action_button("\u{2191}", Message::MoveUpClicked),
            action_button("\u{2193}", Message::MoveDownClicked),
            action_button("Move to Bottom", Message::MoveToBottomClicked),
            button(text("Remove").size(font::CALLOUT.size)).padding([6, 12]).on_press(Message::RemoveClicked).style(|_theme, status| destructive_style(status)),
        ]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center),
    )
    .padding(10)
    .width(Length::Fill)
    .style(|_theme| widgets::glass::liquid_glass(false))
    .into()
}

fn action_button<'a>(label: &'static str, message: Message) -> Element<'a, Message> {
    button(text(label).size(font::CALLOUT.size))
        .padding([6, 12])
        .on_press(message)
        .style(|_theme, status| chip_style(status))
        .into()
}

fn chip_style(status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(iced::Background::Color(if hovered {
            color::with_alpha(iced::Color::WHITE, 0.06)
        } else {
            color::with_alpha(color::CARD, 0.55)
        })),
        text_color: color::SILVER_MUTED,
        border: iced::Border {
            color: color::BORDER,
            width: 1.0,
            radius: sizing::radius::CAPSULE.into(),
        },
        ..button::Style::default()
    }
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

fn row_style(selected: bool, status: button::Status) -> button::Style {
    let glass = widgets::glass::liquid_glass(selected || matches!(status, button::Status::Hovered));
    button::Style {
        background: glass.background,
        text_color: color::SILVER_LIGHT,
        border: glass.border,
        shadow: glass.shadow,
    }
}
