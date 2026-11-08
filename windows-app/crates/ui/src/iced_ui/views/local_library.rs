//! Local Library view for the iced shell, ported from `crate::views::local_library`
//! (`LocalLibraryView.swift`). Reuses that module's pure types/logic unchanged — display
//! modes, sort options, the sheet/command enums, [`LocalLibraryViewState`] itself, and
//! every filter/sort/group function are plain data/logic with no `egui` dependency — and
//! only rewrites `render` as this module's `view`/`update`.
//!
//! Documented simplifications versus the egui version (both deferred to later
//! milestones, not dropped): multi-select via ctrl/shift-click (iced's `button::on_press`
//! doesn't carry modifier-key state the way egui's `Response`/`Modifiers` does — needs a
//! lower-level event subscription) collapses to single-select here, and the sheets this
//! view can open (`ManageFolders`, `DirectTransfer`, format-warning/removal confirms —
//! Milestone 6's job) aren't rendered yet; opening one just records it on
//! `state.sheet` same as before.

use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Element, Length};

use walkman_domain::models::{LocalMusicFolder, Track};

use crate::iced_ui::theme::{color, font, sizing};
use crate::iced_ui::widgets;
use crate::views::local_library::{
    filter_tracks, group_by_album, group_by_artist, sort_tracks, LocalDisplayMode, LocalLibrarySheet,
    LocalLibraryViewState, LocalSortOption,
};

#[derive(Debug, Clone)]
pub enum Message {
    ModeSelected(LocalDisplayMode),
    SortSelected(LocalSortOption),
    SearchChanged(String),
    TrackClicked(String),
    ManageFoldersClicked,
}

pub fn update(state: &mut LocalLibraryViewState, message: Message) {
    match message {
        Message::ModeSelected(mode) => state.display_mode = mode,
        Message::SortSelected(sort) => state.sort = sort,
        Message::SearchChanged(query) => state.search_query = query,
        Message::TrackClicked(path) => {
            state.selected.clear();
            state.selected.insert(path);
        }
        Message::ManageFoldersClicked => state.sheet = LocalLibrarySheet::ManageFolders,
    }
}

pub fn view<'a>(
    tracks: &'a [Track],
    folders: &'a [LocalMusicFolder],
    state: &'a LocalLibraryViewState,
) -> Element<'a, Message> {
    let filtered = filter_tracks(tracks, &state.search_query);

    let mode_row = LocalDisplayMode::ALL.iter().fold(row![].spacing(6), |r, &mode| {
        let active = state.display_mode == mode;
        r.push(
            button(text(mode.title()).size(font::CALLOUT.size))
                .padding([6, 12])
                .on_press(Message::ModeSelected(mode))
                .style(move |_theme, status| tab_style(active, status)),
        )
    });

    let search = text_input("Search", &state.search_query)
        .on_input(Message::SearchChanged)
        .padding(8)
        .size(font::BODY.size);

    let toolbar_top = row![mode_row, Space::new(Length::Fill, Length::Shrink), container(search).width(Length::Fixed(220.0))]
        .align_y(iced::alignment::Vertical::Center)
        .spacing(12);

    let mut toolbar = column![toolbar_top].spacing(8);
    if state.display_mode != LocalDisplayMode::Folders {
        let sort_row = LocalSortOption::ALL.iter().fold(row![].spacing(4), |r, &option| {
            let active = state.sort == option;
            r.push(
                button(text(option.title()).size(font::CAPTION.size))
                    .padding([4, 8])
                    .on_press(Message::SortSelected(option))
                    .style(move |_theme, status| tab_style(active, status)),
            )
        });
        toolbar = toolbar.push(sort_row);
    }

    let manage_folders = button(text("Manage Local Folders\u{2026}").size(font::CAPTION.size))
        .padding([4, 10])
        .on_press(Message::ManageFoldersClicked)
        .style(|_theme, status| tab_style(false, status));
    toolbar = toolbar.push(manage_folders);

    let body: Element<'a, Message> = match state.display_mode {
        LocalDisplayMode::Folders => folders_list(folders),
        LocalDisplayMode::Albums => albums_list(&filtered),
        LocalDisplayMode::Artists => artists_list(&filtered),
        LocalDisplayMode::RecentlyAdded => {
            let mut sorted: Vec<Track> = filtered.iter().map(|t| (*t).clone()).collect();
            sort_tracks(&mut sorted, LocalSortOption::DateAddedNewest);
            songs_table(sorted, state)
        }
        LocalDisplayMode::Songs => {
            let mut sorted: Vec<Track> = filtered.iter().map(|t| (*t).clone()).collect();
            sort_tracks(&mut sorted, state.sort);
            songs_table(sorted, state)
        }
    };

    column![toolbar, body].spacing(12).padding(16).into()
}

fn tab_style(active: bool, status: button::Status) -> button::Style {
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

fn songs_table<'a>(tracks: Vec<Track>, state: &LocalLibraryViewState) -> Element<'a, Message> {
    let col = tracks.into_iter().fold(column![].spacing(4), |col, track| {
        let selected = state.selected.contains(&track.full_path);
        col.push(track_row(track, selected))
    });
    scrollable(col.padding(iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: sizing::TABLE_BOTTOM_INSET,
        left: 0.0,
    }))
    .height(Length::Fill)
    .into()
}

fn track_row<'a>(track: Track, selected: bool) -> Element<'a, Message> {
    let hi_res = track.bits_per_sample > 16 || track.sample_rate > 48_000;
    let path = track.full_path.clone();

    let mut content = row![
        text(track.title).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
        text("\u{2014}").color(color::SILVER_MUTED),
        text(track.artist).size(font::BODY.size).color(color::SILVER_MUTED),
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center);

    if hi_res {
        content = content.push(
            container(text("HR").size(font::BADGE.size))
                .padding([2, 6])
                .style(|_theme| widgets::pill::hi_res_badge()),
        );
    }

    button(content)
        .width(Length::Fill)
        .padding(10)
        .on_press(Message::TrackClicked(path))
        .style(move |_theme, status| row_style(selected, status))
        .into()
}

fn albums_list<'a>(tracks: &[&Track]) -> Element<'a, Message> {
    let groups = group_by_album(tracks);
    let col = groups.into_iter().fold(column![].spacing(4), |col, group| {
        col.push(
            container(
                row![
                    text(group.album).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
                    text(group.artist).size(font::BODY.size).color(color::SILVER_MUTED),
                    Space::new(Length::Fill, Length::Shrink),
                    text(format!("{} tracks", group.track_count)).size(font::CAPTION.size).color(color::SILVER_MUTED),
                ]
                .spacing(8)
                .align_y(iced::alignment::Vertical::Center),
            )
            .padding(10)
            .style(|_theme| widgets::glass::liquid_glass(false)),
        )
    });
    scrollable(col).height(Length::Fill).into()
}

fn artists_list<'a>(tracks: &[&Track]) -> Element<'a, Message> {
    let groups = group_by_artist(tracks);
    let col = groups.into_iter().fold(column![].spacing(4), |col, group| {
        col.push(
            container(
                row![
                    text(group.artist).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
                    Space::new(Length::Fill, Length::Shrink),
                    text(format!("{} tracks", group.track_count)).size(font::CAPTION.size).color(color::SILVER_MUTED),
                ]
                .spacing(8)
                .align_y(iced::alignment::Vertical::Center),
            )
            .padding(10)
            .style(|_theme| widgets::glass::liquid_glass(false)),
        )
    });
    scrollable(col).height(Length::Fill).into()
}

fn folders_list<'a>(folders: &'a [LocalMusicFolder]) -> Element<'a, Message> {
    let col = folders.iter().fold(column![].spacing(4), |col, folder| {
        col.push(
            container(
                row![
                    text(folder.display_name.as_str()).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
                    text(folder.path.as_str()).size(font::CAPTION.size).color(color::SILVER_MUTED),
                    Space::new(Length::Fill, Length::Shrink),
                    text(format!("{} tracks", folder.track_count)).size(font::CAPTION.size).color(color::SILVER_MUTED),
                ]
                .spacing(8)
                .align_y(iced::alignment::Vertical::Center),
            )
            .padding(10)
            .style(|_theme| widgets::glass::liquid_glass(false)),
        )
    });
    scrollable(col).height(Length::Fill).into()
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
