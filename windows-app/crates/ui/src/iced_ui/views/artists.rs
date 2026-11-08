//! Artists view for the iced shell, ported from `crate::views::artists`
//! (`ArtistsView.swift`). Reuses that module's pure types/logic unchanged —
//! `ArtistDrillDown`, `ArtistsCommand`, [`ArtistsViewState`], `group_into_artists`,
//! `filter_artists` — and only rewrites `render` as this module's `view`/`update`. The
//! egui version's hand-painted avatar circle (`Painter::circle_filled` + a text galley)
//! becomes a plain circular container (border radius = size/2) with centered text —
//! same visual, declarative instead of painted.
//!
//! Same documented simplification as the other ported views: the context menus
//! ("Open Artist"/"Transfer All Tracks…" on a tile, "Play"/"Edit Metadata…"/"Trim &
//! Cut…"/"Delete" on a track row) are dropped pending an iced context-menu equivalent;
//! the tile/row click still drives the artist → albums → tracks drill-down itself.

use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Element, Length};

use walkman_domain::models::{ArtistItem, Track};

use crate::iced_ui::theme::{color, font, sizing};
use crate::views::artists::{filter_artists, group_into_artists, ArtistDrillDown, ArtistsViewState};

const AVATAR_SIZE: f32 = 96.0;

#[derive(Debug, Clone)]
pub enum Message {
    SearchChanged(String),
    ArtistOpened(String),
    AlbumOpened { artist: String, album_id: String },
    BackToGrid,
    BackToAlbums(String),
}

pub fn update(state: &mut ArtistsViewState, message: Message) {
    match message {
        Message::SearchChanged(query) => state.search_query = query,
        Message::ArtistOpened(artist) => state.drill_down = ArtistDrillDown::Albums { artist },
        Message::AlbumOpened { artist, album_id } => {
            state.drill_down = ArtistDrillDown::Tracks { artist, album_id }
        }
        Message::BackToGrid => state.drill_down = ArtistDrillDown::Grid,
        Message::BackToAlbums(artist) => state.drill_down = ArtistDrillDown::Albums { artist },
    }
}

pub fn view<'a>(tracks: &[Track], state: &'a ArtistsViewState) -> Element<'a, Message> {
    let artists = group_into_artists(tracks);

    match &state.drill_down {
        ArtistDrillDown::Grid => grid(artists, state),
        ArtistDrillDown::Albums { artist } => match artists.into_iter().find(|a| &a.name == artist) {
            Some(found) => albums(found),
            None => grid(group_into_artists(tracks), state),
        },
        ArtistDrillDown::Tracks { artist, album_id } => {
            let found = artists.into_iter().find(|a| &a.name == artist).and_then(|a| {
                let album = a.albums.iter().find(|alb| &alb.id() == album_id)?.clone();
                Some((a.name, album))
            });
            match found {
                Some((artist, album)) => tracks_list(artist, album),
                None => grid(group_into_artists(tracks), state),
            }
        }
    }
}

/// Artist tiles in rows of 4 (plain `column`-of-`row`s instead of iced's wrapping row
/// layout, whose builder shape didn't fit this fold-based construction cleanly).
const TILES_PER_ROW: usize = 4;

fn grid(artists: Vec<ArtistItem>, state: &ArtistsViewState) -> Element<'_, Message> {
    let search = text_input("Search Artists", &state.search_query)
        .on_input(Message::SearchChanged)
        .padding(8)
        .size(font::BODY.size)
        .width(Length::Fixed(220.0));

    let filtered: Vec<ArtistItem> = filter_artists(&artists, &state.search_query).into_iter().cloned().collect();
    let tiles = filtered.chunks(TILES_PER_ROW).fold(column![].spacing(16), |col, chunk| {
        let r = chunk.iter().cloned().fold(row![].spacing(16), |r, artist| r.push(avatar_tile(artist)));
        col.push(r)
    });

    let body = scrollable(
        container(tiles).padding(iced::Padding {
            top: 0.0,
            right: 0.0,
            bottom: sizing::TABLE_BOTTOM_INSET,
            left: 0.0,
        }),
    )
    .height(Length::Fill);

    column![search, body].spacing(12).padding(16).into()
}

fn avatar_tile<'a>(artist: ArtistItem) -> Element<'a, Message> {
    let initial = artist.name.chars().next().unwrap_or('?').to_uppercase().to_string();

    let avatar = container(text(initial).size(font::TITLE1.size).color(color::SILVER_MUTED))
        .width(Length::Fixed(AVATAR_SIZE))
        .height(Length::Fixed(AVATAR_SIZE))
        .center_x(Length::Fixed(AVATAR_SIZE))
        .center_y(Length::Fixed(AVATAR_SIZE))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(color::INPUT)),
            border: iced::Border {
                radius: (AVATAR_SIZE / 2.0).into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });

    let name_row = if artist.is_hi_res {
        row![
            text(artist.name.clone()).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
            container(text("HR").size(font::BADGE.size))
                .padding([2, 6])
                .style(|_theme| crate::iced_ui::widgets::pill::hi_res_badge()),
        ]
        .spacing(6)
    } else {
        row![text(artist.name.clone()).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT)]
    };

    let content = column![
        avatar,
        name_row,
        text(format!("{} albums", artist.album_count)).size(font::CAPTION.size).color(color::SILVER_MUTED),
    ]
    .spacing(4)
    .align_x(iced::alignment::Horizontal::Center)
    .width(Length::Fixed(AVATAR_SIZE + 16.0));

    button(content)
        .on_press(Message::ArtistOpened(artist.name.clone()))
        .style(|_theme, _status| button::Style {
            background: None,
            text_color: color::SILVER_LIGHT,
            ..button::Style::default()
        })
        .into()
}

fn back_button<'a>(label: &'static str, message: Message) -> Element<'a, Message> {
    button(text(label).size(font::CALLOUT.size))
        .padding([6, 12])
        .on_press(message)
        .style(|_theme, status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(iced::Background::Color(if hovered {
                    color::with_alpha(iced::Color::WHITE, 0.06)
                } else {
                    color::with_alpha(color::CARD, 0.55)
                })),
                text_color: color::SILVER_LIGHT,
                border: iced::Border {
                    color: color::BORDER,
                    width: 1.0,
                    radius: sizing::radius::CAPSULE.into(),
                },
                ..button::Style::default()
            }
        })
        .into()
}

fn albums<'a>(artist: ArtistItem) -> Element<'a, Message> {
    let back = back_button("\u{2190} Back to Artists", Message::BackToGrid);
    let heading = text(artist.name.clone()).size(font::TITLE2.size).color(color::SILVER_LIGHT);

    let list = artist.albums.into_iter().fold(column![].spacing(4), |col, album| {
        let message = Message::AlbumOpened { artist: artist.name.clone(), album_id: album.id() };
        col.push(
            button(
                row![
                    text(album.name.clone()).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
                    Space::new(Length::Fill, Length::Shrink),
                    text(format!("{} tracks", album.track_count)).size(font::CAPTION.size).color(color::SILVER_MUTED),
                ]
                .align_y(iced::alignment::Vertical::Center),
            )
            .width(Length::Fill)
            .padding(10)
            .on_press(message)
            .style(|_theme, status| row_style(matches!(status, button::Status::Hovered))),
        )
    });

    let body = scrollable(list.padding(iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: sizing::TABLE_BOTTOM_INSET,
        left: 0.0,
    }))
    .height(Length::Fill);

    column![back, heading, body].spacing(8).padding(16).into()
}

fn tracks_list<'a>(artist: String, album: walkman_domain::models::AlbumItem) -> Element<'a, Message> {
    let back = back_button("\u{2190} Back to Albums", Message::BackToAlbums(artist));
    let heading = text(album.name).size(font::TITLE2.size).color(color::SILVER_LIGHT);

    let list = album.tracks.into_iter().fold(column![].spacing(4), |col, track| {
        col.push(
            container(text(track.title).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT))
                .width(Length::Fill)
                .padding(10)
                .style(|_theme| crate::iced_ui::widgets::glass::liquid_glass(false)),
        )
    });

    let body = scrollable(list.padding(iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: sizing::TABLE_BOTTOM_INSET,
        left: 0.0,
    }))
    .height(Length::Fill);

    column![back, heading, body].spacing(8).padding(16).into()
}

fn row_style(hovered: bool) -> button::Style {
    let glass = crate::iced_ui::widgets::glass::liquid_glass(hovered);
    button::Style {
        background: glass.background,
        text_color: color::SILVER_LIGHT,
        border: glass.border,
        shadow: glass.shadow,
    }
}
