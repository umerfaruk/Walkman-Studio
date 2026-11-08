//! Albums view for the iced shell, ported from `crate::views::albums`
//! (`AlbumsView.swift`). Reuses that module's pure types/logic unchanged —
//! `AlbumSortOption`, `FormatFilter`, `AlbumsCommand`, [`AlbumsViewState`],
//! `group_into_albums`, `sort_albums`, `filter_albums`, `distinct_formats` — and only
//! rewrites `render` as this module's `view`/`update`. Cover art isn't wired up on the
//! egui side either (needs `cover_art_cache.rs` bridged to a texture, a separate
//! concern), so both versions show the same placeholder initial-monogram tile.
//!
//! Same documented simplifications as the other ported views: multi-select collapses to
//! single-select, the tile/track context menus are dropped (iced has no built-in
//! context-menu widget — double-click still opens an album's detail), the sort picker
//! is a row of buttons instead of a dropdown (consistent with `local_library`'s), and
//! the expanding search bar's animation is just an instant show/hide.

use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Element, Length};

use walkman_domain::models::{AlbumItem, Track};

use crate::iced_ui::theme::{color, font, sizing};
use crate::iced_ui::widgets;
use crate::views::albums::{
    distinct_formats, filter_albums, group_into_albums, sort_albums, AlbumSortOption, AlbumsViewState, FormatFilter,
};

const TILE_SIZE: f32 = 160.0;
const TILES_PER_ROW: usize = 6;

#[derive(Debug, Clone)]
pub enum Message {
    SortSelected(AlbumSortOption),
    FormatSelected(FormatFilter),
    HiResToggled,
    SearchExpandToggled,
    SearchChanged(String),
    FindLyricsClicked,
    AlbumOpened(String),
    BackToGrid,
}

pub fn update(state: &mut AlbumsViewState, message: Message) {
    match message {
        Message::SortSelected(sort) => state.sort = sort,
        Message::FormatSelected(format) => state.format_filter = format,
        Message::HiResToggled => state.hi_res_only = !state.hi_res_only,
        Message::SearchExpandToggled => {
            state.search_expanded = !state.search_expanded;
            if !state.search_expanded {
                state.search_query.clear();
            }
        }
        Message::SearchChanged(query) => state.search_query = query,
        Message::FindLyricsClicked => {
            use crate::views::albums::AlbumsCommand;
            state.commands.push(AlbumsCommand::BatchLyrics(state.selected.clone()));
        }
        Message::AlbumOpened(id) => state.drilled_into = Some(id),
        Message::BackToGrid => state.drilled_into = None,
    }
}

pub fn view<'a>(tracks: &[Track], state: &'a AlbumsViewState) -> Element<'a, Message> {
    let mut albums = group_into_albums(tracks);
    sort_albums(&mut albums, state.sort);

    if let Some(album_id) = &state.drilled_into {
        if let Some(album) = albums.iter().find(|a| &a.id() == album_id).cloned() {
            return album_detail(album);
        }
        // The album vanished (e.g. deleted) — fall through to the grid below.
    }

    let filtered: Vec<AlbumItem> =
        filter_albums(&albums, &state.format_filter, state.hi_res_only, &state.search_query).into_iter().cloned().collect();

    column![toolbar(&albums, state), grid(filtered)].spacing(12).padding(16).into()
}

fn toolbar<'a>(albums: &[AlbumItem], state: &'a AlbumsViewState) -> Element<'a, Message> {
    let mut chips = row![format_chip("ALL", state.format_filter == FormatFilter::All, Message::FormatSelected(FormatFilter::All))].spacing(6);
    for format in distinct_formats(albums) {
        let active = state.format_filter == FormatFilter::Format(format.clone());
        chips = chips.push(format_chip(format.clone(), active, Message::FormatSelected(FormatFilter::Format(format))));
    }

    let hi_res = button(text("Hi-Res").size(font::CALLOUT.size))
        .padding([6, 12])
        .on_press(Message::HiResToggled)
        .style(move |_theme, status| chip_style(state.hi_res_only, status));

    let sort_row = AlbumSortOption::ALL.iter().fold(row![].spacing(4), |r, &option| {
        let active = state.sort == option;
        r.push(
            button(text(option.title()).size(font::CAPTION.size))
                .padding([4, 8])
                .on_press(Message::SortSelected(option))
                .style(move |_theme, status| chip_style(active, status)),
        )
    });

    let search: Element<'a, Message> = if state.search_expanded {
        row![
            text_input("Search Albums", &state.search_query)
                .on_input(Message::SearchChanged)
                .padding(8)
                .size(font::BODY.size)
                .width(Length::Fixed(200.0)),
            button(text("\u{2715}").size(font::CALLOUT.size))
                .padding([6, 10])
                .on_press(Message::SearchExpandToggled)
                .style(|_theme, status| chip_style(false, status)),
        ]
        .spacing(6)
        .into()
    } else {
        button(text("Search\u{2026}").size(font::CALLOUT.size))
            .padding([6, 12])
            .on_press(Message::SearchExpandToggled)
            .style(|_theme, status| chip_style(false, status))
            .into()
    };

    let has_selection = !state.selected.is_empty();
    let mut find_lyrics = button(text("Find Lyrics\u{2026}").size(font::CALLOUT.size)).padding([6, 12]);
    if has_selection {
        find_lyrics = find_lyrics.on_press(Message::FindLyricsClicked);
    }
    let find_lyrics = find_lyrics.style(|_theme, status| chip_style(false, status));

    column![row![chips, hi_res].spacing(6), sort_row, row![search, find_lyrics].spacing(12)].spacing(8).into()
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

fn grid<'a>(albums: Vec<AlbumItem>) -> Element<'a, Message> {
    let tiles = albums.chunks(TILES_PER_ROW).fold(column![].spacing(16), |col, chunk| {
        let r = chunk.iter().cloned().fold(row![].spacing(16), |r, album| r.push(tile(album)));
        col.push(r)
    });

    scrollable(
        container(tiles).padding(iced::Padding {
            top: 0.0,
            right: 0.0,
            bottom: sizing::TABLE_BOTTOM_INSET,
            left: 0.0,
        }),
    )
    .height(Length::Fill)
    .into()
}

fn tile<'a>(album: AlbumItem) -> Element<'a, Message> {
    let initial = album.name.chars().next().unwrap_or('?').to_uppercase().to_string();
    let cover_size = TILE_SIZE - 16.0;

    let cover = container(text(initial).size(font::TITLE1.size).color(color::SILVER_MUTED))
        .width(Length::Fixed(cover_size))
        .height(Length::Fixed(cover_size))
        .center_x(Length::Fixed(cover_size))
        .center_y(Length::Fixed(cover_size))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(color::INPUT)),
            border: iced::Border {
                radius: 6.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });

    let artist_row = if album.is_hi_res {
        row![
            text(album.artist.clone()).size(font::CAPTION.size).color(color::SILVER_MUTED),
            container(text("HR").size(font::BADGE.size)).padding([2, 6]).style(|_theme| widgets::pill::hi_res_badge()),
        ]
        .spacing(6)
    } else {
        row![text(album.artist.clone()).size(font::CAPTION.size).color(color::SILVER_MUTED)]
    };

    let content = column![
        cover,
        text(album.name.clone()).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
        artist_row,
    ]
    .spacing(4)
    .width(Length::Fixed(TILE_SIZE));

    button(content)
        .on_press(Message::AlbumOpened(album.id()))
        .style(|_theme, _status| button::Style {
            background: None,
            text_color: color::SILVER_LIGHT,
            ..button::Style::default()
        })
        .into()
}

fn album_detail<'a>(album: AlbumItem) -> Element<'a, Message> {
    let back = button(text("\u{2190} Back to Albums").size(font::CALLOUT.size))
        .padding([6, 12])
        .on_press(Message::BackToGrid)
        .style(|_theme, status| chip_style(false, status));

    let heading = column![
        text(album.name).size(font::TITLE2.size).color(color::SILVER_LIGHT),
        text(album.artist).size(font::CALLOUT.size).color(color::SILVER_MUTED),
    ]
    .spacing(2);

    let list = album.tracks.into_iter().fold(column![].spacing(4), |col, track| {
        col.push(
            container(
                row![
                    text(track.track_num).size(font::MONO.size).color(color::SILVER_MUTED),
                    text(track.title).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
                ]
                .spacing(8)
                .align_y(iced::alignment::Vertical::Center),
            )
            .width(Length::Fill)
            .padding(10)
            .style(|_theme| widgets::glass::liquid_glass(false)),
        )
    });

    let body = scrollable(list.padding(iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: sizing::TABLE_BOTTOM_INSET,
        left: 0.0,
    }))
    .height(Length::Fill);

    column![back, heading, Space::new(Length::Shrink, Length::Fixed(4.0)), body]
        .spacing(8)
        .padding(16)
        .into()
}
