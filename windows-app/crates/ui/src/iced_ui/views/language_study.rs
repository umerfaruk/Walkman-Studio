//! Language Study view for the iced shell, ported from `crate::views::language_study`
//! (`LanguageStudyView.swift`). Reuses that module's pure types/logic unchanged —
//! `LanguageDisplayMode`, `LanguageSortOption`, `LanguageStudyCommand`,
//! [`LanguageStudyViewState`], `language_study_tracks`, `sort_tracks`, `group_by_folder`
//! — plus `albums::group_into_albums`/`artists::group_into_artists` for the
//! Albums("Course")/Artists("Speaker") modes — and only rewrites `render` as this
//! module's `view`/`update`.
//!
//! Same documented simplifications as the other ported views.

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length};

use walkman_domain::models::Track;

use crate::iced_ui::theme::{color, font, sizing};
use crate::iced_ui::widgets;
use crate::views::language_study::{
    group_by_folder, language_study_tracks, sort_tracks, LanguageDisplayMode, LanguageSortOption, LanguageStudyCommand,
    LanguageStudyViewState,
};

#[derive(Debug, Clone)]
pub enum Message {
    ModeSelected(LanguageDisplayMode),
    SortSelected(LanguageSortOption),
    TrackClicked(String),
    FindLyricsClicked,
}

pub fn update(state: &mut LanguageStudyViewState, message: Message) {
    match message {
        Message::ModeSelected(mode) => state.display_mode = mode,
        Message::SortSelected(sort) => state.sort = sort,
        Message::TrackClicked(path) => {
            state.selected.clear();
            state.selected.push(path);
        }
        Message::FindLyricsClicked => {
            state.commands.push(LanguageStudyCommand::BatchLyrics(state.selected.clone()));
        }
    }
}

pub fn view<'a>(all_tracks: &[Track], state: &'a LanguageStudyViewState) -> Element<'a, Message> {
    let tracks = language_study_tracks(all_tracks);

    let mode_row = LanguageDisplayMode::ALL.iter().fold(row![].spacing(6), |r, &mode| {
        let active = state.display_mode == mode;
        r.push(
            button(text(mode.title()).size(font::CALLOUT.size))
                .padding([6, 12])
                .on_press(Message::ModeSelected(mode))
                .style(move |_theme, status| chip_style(active, status)),
        )
    });

    let mut toolbar = column![mode_row].spacing(8);
    if state.display_mode != LanguageDisplayMode::Folders {
        let sort_row = LanguageSortOption::ALL.iter().fold(row![].spacing(4), |r, &option| {
            let active = state.sort == option;
            r.push(
                button(text(option.title()).size(font::CAPTION.size))
                    .padding([4, 8])
                    .on_press(Message::SortSelected(option))
                    .style(move |_theme, status| chip_style(active, status)),
            )
        });
        toolbar = toolbar.push(sort_row);
    }

    let has_selection = !state.selected.is_empty();
    let mut find_lyrics = button(text("Find Lyrics\u{2026}").size(font::CALLOUT.size)).padding([6, 12]);
    if has_selection {
        find_lyrics = find_lyrics.on_press(Message::FindLyricsClicked);
    }
    toolbar = toolbar.push(find_lyrics.style(|_theme, status| chip_style(false, status)));

    let body: Element<'a, Message> = match state.display_mode {
        LanguageDisplayMode::Tracks => track_table(tracks, state.sort, state),
        LanguageDisplayMode::RecentlyAdded => {
            let mut sorted = tracks;
            sort_tracks(&mut sorted, LanguageSortOption::RecentlyAdded);
            track_table(sorted, LanguageSortOption::RecentlyAdded, state)
        }
        LanguageDisplayMode::Albums => course_table(&tracks),
        LanguageDisplayMode::Artists => speaker_table(&tracks),
        LanguageDisplayMode::Folders => folder_table(&tracks),
    };

    column![toolbar, body].spacing(12).padding(16).into()
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

fn track_table<'a>(mut tracks: Vec<Track>, sort: LanguageSortOption, state: &LanguageStudyViewState) -> Element<'a, Message> {
    sort_tracks(&mut tracks, sort);
    let list = tracks.into_iter().fold(column![].spacing(4), |col, track| {
        let selected = state.selected.contains(&track.full_path);
        col.push(track_row(track, selected))
    });

    scrollable(list.padding(iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: sizing::TABLE_BOTTOM_INSET,
        left: 0.0,
    }))
    .height(Length::Fill)
    .into()
}

fn track_row<'a>(track: Track, selected: bool) -> Element<'a, Message> {
    let path = track.full_path.clone();
    let duration = walkman_services::walkman_engine::format_duration(track.duration_seconds);

    let content = row![
        text(track.title).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
        text(track.artist).size(font::BODY.size).color(color::SILVER_MUTED),
        text(duration).size(font::MONO.size).color(color::SILVER_MUTED),
    ]
    .spacing(12)
    .align_y(iced::alignment::Vertical::Center);

    button(content)
        .width(Length::Fill)
        .padding(10)
        .on_press(Message::TrackClicked(path))
        .style(move |_theme, status| row_style(selected, status))
        .into()
}

fn course_table<'a>(tracks: &[Track]) -> Element<'a, Message> {
    let albums = crate::views::albums::group_into_albums(tracks);
    let list = albums.into_iter().fold(column![].spacing(4), |col, album| {
        col.push(simple_row(album.name, format!("{} \u{2022} {} lessons", album.artist, album.track_count)))
    });
    scrollable(list.padding(iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: sizing::TABLE_BOTTOM_INSET,
        left: 0.0,
    }))
    .height(Length::Fill)
    .into()
}

fn speaker_table<'a>(tracks: &[Track]) -> Element<'a, Message> {
    let artists = crate::views::artists::group_into_artists(tracks);
    let list = artists.into_iter().fold(column![].spacing(4), |col, artist| {
        col.push(simple_row(artist.name, format!("{} tracks", artist.track_count)))
    });
    scrollable(list.padding(iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: sizing::TABLE_BOTTOM_INSET,
        left: 0.0,
    }))
    .height(Length::Fill)
    .into()
}

fn folder_table<'a>(tracks: &[Track]) -> Element<'a, Message> {
    let folders = group_by_folder(tracks);
    let list = folders.into_iter().fold(column![].spacing(4), |col, folder| {
        col.push(simple_row(folder.folder_name, format!("{} tracks", folder.track_count)))
    });
    scrollable(list.padding(iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: sizing::TABLE_BOTTOM_INSET,
        left: 0.0,
    }))
    .height(Length::Fill)
    .into()
}

fn simple_row<'a>(title: String, subtitle: String) -> Element<'a, Message> {
    container(
        row![
            text(title).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
            text(subtitle).size(font::CAPTION.size).color(color::SILVER_MUTED),
        ]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .padding(10)
    .style(|_theme| widgets::glass::liquid_glass(false))
    .into()
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
