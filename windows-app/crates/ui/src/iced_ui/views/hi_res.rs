//! Hi-Res Audio view for the iced shell, ported from `crate::views::hi_res`
//! (`HiResAudioView.swift`). Reuses that module's pure types/logic unchanged —
//! `HiResViewMode`, `TrackSortOption`, `QualityFilter`, `HiResCommand`,
//! [`HiResViewState`], `hi_res_tracks`, `sort_tracks`, `filter_tracks`,
//! `distinct_formats` — plus `crate::views::albums`/`crate::views::artists`'s grouping
//! functions for the Albums/Artists modes — and only rewrites `render` as this module's
//! `view`/`update`.
//!
//! Same documented simplifications as the other ported views: single-select only, no
//! context menu, sort/filter pickers are button rows instead of dropdowns.

use iced::widget::{button, column, row, scrollable, text, text_input};
use iced::{Element, Length};

use walkman_domain::models::Track;

use crate::iced_ui::theme::{color, font, sizing};
use crate::iced_ui::widgets;
use crate::views::albums::AlbumSortOption;
use crate::views::hi_res::{
    distinct_formats, filter_tracks, hi_res_tracks, sort_tracks, HiResCommand, HiResViewMode, HiResViewState, QualityFilter,
    TrackSortOption,
};

#[derive(Debug, Clone)]
pub enum Message {
    ModeSelected(HiResViewMode),
    QualityFilterSelected(QualityFilter),
    TrackSortSelected(TrackSortOption),
    AlbumSortSelected(AlbumSortOption),
    SearchChanged(String),
    TrackClicked(String),
    FindLyricsClicked,
}

pub fn update(state: &mut HiResViewState, message: Message) {
    match message {
        Message::ModeSelected(mode) => state.mode = mode,
        Message::QualityFilterSelected(filter) => state.quality_filter = filter,
        Message::TrackSortSelected(sort) => state.track_sort = sort,
        Message::AlbumSortSelected(sort) => state.album_sort = sort,
        Message::SearchChanged(query) => state.search_query = query,
        Message::TrackClicked(path) => {
            state.selected.clear();
            state.selected.push(path);
        }
        Message::FindLyricsClicked => {
            state.commands.push(HiResCommand::BatchLyrics(state.selected.clone()));
        }
    }
}

pub fn view<'a>(all_tracks: &[Track], state: &'a HiResViewState) -> Element<'a, Message> {
    let tracks = hi_res_tracks(all_tracks);

    let body: Element<'a, Message> = match state.mode {
        HiResViewMode::Tracks => tracks_list(&tracks, state),
        HiResViewMode::Albums => albums_list(&tracks, state.album_sort),
        HiResViewMode::Artists => artists_list(&tracks),
    };

    column![toolbar(&tracks, state), body].spacing(12).padding(16).into()
}

fn toolbar<'a>(tracks: &[Track], state: &'a HiResViewState) -> Element<'a, Message> {
    let mode_row = HiResViewMode::ALL.iter().fold(row![].spacing(6), |r, &mode| {
        let active = state.mode == mode;
        r.push(
            button(text(mode.title()).size(font::CALLOUT.size))
                .padding([6, 12])
                .on_press(Message::ModeSelected(mode))
                .style(move |_theme, status| chip_style(active, status)),
        )
    });

    let mut rows = column![mode_row].spacing(8);

    if state.mode == HiResViewMode::Tracks {
        let mut filter_row =
            row![format_chip("ALL", state.quality_filter == QualityFilter::All, Message::QualityFilterSelected(QualityFilter::All))]
                .spacing(6);
        for format in distinct_formats(tracks) {
            let active = state.quality_filter == QualityFilter::Format(format.clone());
            filter_row = filter_row.push(format_chip(
                format.clone(),
                active,
                Message::QualityFilterSelected(QualityFilter::Format(format)),
            ));
        }
        rows = rows.push(filter_row);

        let sort_row = TrackSortOption::ALL.iter().fold(row![].spacing(4), |r, &option| {
            let active = state.track_sort == option;
            r.push(
                button(text(option.title()).size(font::CAPTION.size))
                    .padding([4, 8])
                    .on_press(Message::TrackSortSelected(option))
                    .style(move |_theme, status| chip_style(active, status)),
            )
        });
        rows = rows.push(sort_row);
    } else if state.mode == HiResViewMode::Albums {
        let sort_row = AlbumSortOption::ALL.iter().fold(row![].spacing(4), |r, &option| {
            let active = state.album_sort == option;
            r.push(
                button(text(option.title()).size(font::CAPTION.size))
                    .padding([4, 8])
                    .on_press(Message::AlbumSortSelected(option))
                    .style(move |_theme, status| chip_style(active, status)),
            )
        });
        rows = rows.push(sort_row);
    }

    let search = text_input("Search Hi-Res Audio", &state.search_query)
        .on_input(Message::SearchChanged)
        .padding(8)
        .size(font::BODY.size)
        .width(Length::Fixed(200.0));

    let has_selection = !state.selected.is_empty();
    let mut find_lyrics = button(text("Find Lyrics\u{2026}").size(font::CALLOUT.size)).padding([6, 12]);
    if has_selection {
        find_lyrics = find_lyrics.on_press(Message::FindLyricsClicked);
    }
    let find_lyrics = find_lyrics.style(|_theme, status| chip_style(false, status));

    rows = rows.push(row![search, find_lyrics].spacing(12));
    rows.into()
}

fn format_chip<'a>(label: impl Into<String>, active: bool, message: Message) -> Element<'a, Message> {
    button(text(label.into()).size(font::CALLOUT.size))
        .padding([6, 12])
        .on_press(message)
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

fn tracks_list<'a>(tracks: &[Track], state: &HiResViewState) -> Element<'a, Message> {
    let filtered = filter_tracks(tracks, &state.quality_filter, &state.search_query);
    let mut owned: Vec<Track> = filtered.into_iter().cloned().collect();
    sort_tracks(&mut owned, state.track_sort);

    let list = owned.into_iter().fold(column![].spacing(4), |col, track| {
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
    let quality = format!("{}bit/{}kHz", track.bits_per_sample, track.sample_rate / 1000);

    let content = row![
        text(track.title).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
        text(track.artist).size(font::BODY.size).color(color::SILVER_MUTED),
        iced::widget::container(text("HR").size(font::BADGE.size)).padding([2, 6]).style(|_theme| widgets::pill::hi_res_badge()),
        text(quality).size(font::MONO_SMALL.size).color(color::SILVER_MUTED),
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

fn albums_list<'a>(tracks: &[Track], sort: AlbumSortOption) -> Element<'a, Message> {
    use crate::views::albums::{group_into_albums, sort_albums};
    let mut grouped = group_into_albums(tracks);
    sort_albums(&mut grouped, sort);

    let list = grouped.into_iter().fold(column![].spacing(4), |col, album| {
        col.push(
            iced::widget::container(
                row![
                    text(album.name).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
                    text(album.artist).size(font::CAPTION.size).color(color::SILVER_MUTED),
                    iced::widget::container(text("HR").size(font::BADGE.size))
                        .padding([2, 6])
                        .style(|_theme| widgets::pill::hi_res_badge()),
                ]
                .spacing(8)
                .align_y(iced::alignment::Vertical::Center),
            )
            .width(Length::Fill)
            .padding(10)
            .style(|_theme| widgets::glass::liquid_glass(false)),
        )
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

fn artists_list<'a>(tracks: &[Track]) -> Element<'a, Message> {
    let grouped = crate::views::artists::group_into_artists(tracks);

    let list = grouped.into_iter().fold(column![].spacing(4), |col, artist| {
        col.push(
            iced::widget::container(text(artist.name).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT))
                .width(Length::Fill)
                .padding(10)
                .style(|_theme| widgets::glass::liquid_glass(false)),
        )
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

fn row_style(selected: bool, status: button::Status) -> button::Style {
    let glass = widgets::glass::liquid_glass(selected || matches!(status, button::Status::Hovered));
    button::Style {
        background: glass.background,
        text_color: color::SILVER_LIGHT,
        border: glass.border,
        shadow: glass.shadow,
    }
}
