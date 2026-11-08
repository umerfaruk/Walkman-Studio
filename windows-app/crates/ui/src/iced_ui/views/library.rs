//! Library ("All Songs") view for the iced shell, ported from `crate::views::library`
//! (`LibraryView.swift`). Reuses that module's pure types/logic unchanged — `Column`,
//! `SortDirection`, `LibraryCommand`, [`LibraryViewState`], `sort_tracks`,
//! `filter_tracks` — and only rewrites `render` as this module's `view`/`update`.
//!
//! Same documented simplifications as `local_library`/`recently_added`: multi-select
//! via ctrl/shift-click collapses to single-select, and the per-row context menu
//! (Play/Edit Metadata/Trim & Cut/Transfer/Delete) is dropped pending an iced
//! context-menu equivalent.

use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Element, Length};

use walkman_domain::models::Track;

use crate::iced_ui::theme::{color, font, sizing};
use crate::iced_ui::widgets;
use crate::views::library::{filter_tracks, sort_tracks, Column, LibraryCommand, LibraryViewState, SortDirection};

#[derive(Debug, Clone)]
pub enum Message {
    SearchChanged(String),
    ColumnClicked(Column),
    TrackClicked(String),
    FindLyricsClicked,
}

pub fn update(state: &mut LibraryViewState, message: Message) {
    match message {
        Message::SearchChanged(query) => state.search_query = query,
        Message::ColumnClicked(column) => {
            if state.sort_column == column {
                state.sort_direction = match state.sort_direction {
                    SortDirection::Ascending => SortDirection::Descending,
                    SortDirection::Descending => SortDirection::Ascending,
                };
            } else {
                state.sort_column = column;
                state.sort_direction = SortDirection::Ascending;
            }
        }
        Message::TrackClicked(path) => {
            state.selected.clear();
            state.selected.push(path);
        }
        Message::FindLyricsClicked => {
            state.commands.push(LibraryCommand::BatchLyrics(state.selected.clone()));
        }
    }
}

pub fn view<'a>(tracks: &'a [Track], state: &'a LibraryViewState) -> Element<'a, Message> {
    let search = text_input("Search All Songs", &state.search_query)
        .on_input(Message::SearchChanged)
        .padding(8)
        .size(font::BODY.size)
        .width(Length::Fixed(240.0));

    let has_selection = !state.selected.is_empty();
    let mut find_lyrics = button(text("Find Lyrics\u{2026}").size(font::CALLOUT.size)).padding([6, 12]);
    if has_selection {
        find_lyrics = find_lyrics.on_press(Message::FindLyricsClicked);
    }
    let find_lyrics = find_lyrics.style(|_theme, status| tab_style(false, status));

    let toolbar = row![search, find_lyrics].spacing(12).align_y(iced::alignment::Vertical::Center);

    let header = Column::ALL.iter().fold(row![].spacing(24), |r, &col| {
        let label = if state.sort_column == col {
            format!("{} {}", col.title(), arrow(state.sort_direction))
        } else {
            col.title().to_string()
        };
        r.push(
            button(text(label).size(font::SECTION_HEADER.size).color(color::SILVER_MUTED))
                .padding(0)
                .on_press(Message::ColumnClicked(col))
                .style(|_theme, _status| button::Style {
                    background: None,
                    text_color: color::SILVER_MUTED,
                    ..button::Style::default()
                }),
        )
    });

    let mut filtered: Vec<Track> = filter_tracks(tracks, &state.search_query).into_iter().cloned().collect();
    sort_tracks(&mut filtered, state.sort_column, state.sort_direction);

    let list = filtered.into_iter().fold(column![].spacing(4), |col, track| {
        let selected = state.selected.contains(&track.full_path);
        col.push(track_row(track, selected))
    });

    let body = scrollable(list.padding(iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: sizing::TABLE_BOTTOM_INSET,
        left: 0.0,
    }))
    .height(Length::Fill);

    column![toolbar, header, body].spacing(8).padding(16).into()
}

fn arrow(direction: SortDirection) -> &'static str {
    match direction {
        SortDirection::Ascending => "\u{25B2}",
        SortDirection::Descending => "\u{25BC}",
    }
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

fn track_row<'a>(track: Track, selected: bool) -> Element<'a, Message> {
    let hi_res = track.bits_per_sample > 16 || track.sample_rate > 48_000;
    let path = track.full_path.clone();
    let duration = walkman_services::walkman_engine::format_duration(track.duration_seconds);

    let mut content = row![
        text(track.title).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
        text(track.artist).size(font::BODY.size).color(color::SILVER_MUTED),
        text(track.album).size(font::BODY.size).color(color::SILVER_MUTED),
        text(duration).size(font::MONO.size).color(color::SILVER_MUTED),
    ]
    .spacing(16)
    .align_y(iced::alignment::Vertical::Center);

    if hi_res {
        content = content.push(
            container(text("HR").size(font::BADGE.size))
                .padding([2, 6])
                .style(|_theme| widgets::pill::hi_res_badge()),
        );
    }
    content = content.push(Space::new(Length::Fill, Length::Shrink));

    button(content)
        .width(Length::Fill)
        .padding(10)
        .on_press(Message::TrackClicked(path))
        .style(move |_theme, status| row_style(selected, status))
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
