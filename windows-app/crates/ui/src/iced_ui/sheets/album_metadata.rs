//! Album Metadata editor sheet for the iced shell, ported from
//! `crate::sheets::album_metadata` (`AlbumMetadataEditorSheet.swift`). Reuses that
//! module's pure types/logic unchanged — `AlbumEditorFields`, `AlbumEditorTrackRow`,
//! `RenumberStyle`, `renumber`, `move_row_up`/`move_row_down`, `AlbumEditorCommand`,
//! [`AlbumEditorState`] — and only rewrites `render` as this module's `view`/`update`.
//! The artwork row reuses the same placeholder-tile pattern as
//! `iced_ui::sheets::song_metadata`'s Album Artwork tab.

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length};

use crate::iced_ui::sheets::text_field;
use crate::iced_ui::theme::{color, font};
use crate::sheets::album_metadata::{move_row_down, move_row_up, renumber, AlbumEditorCommand, AlbumEditorState, RenumberStyle};

#[derive(Debug, Clone)]
pub enum Message {
    TitleChanged(String),
    ArtistChanged(String),
    YearChanged(String),
    GenreChanged(String),
    ImportArtworkClicked,
    RenumberZeroPaddedClicked,
    RenumberOneToNClicked,
    MoveRowUpClicked(usize),
    MoveRowDownClicked(usize),
    SaveClicked,
    CancelClicked,
}

pub fn update(state: &mut AlbumEditorState, message: Message) {
    match message {
        Message::TitleChanged(v) => state.fields.title = v,
        Message::ArtistChanged(v) => state.fields.artist = v,
        Message::YearChanged(v) => state.fields.year = v,
        Message::GenreChanged(v) => state.fields.genre = v,
        Message::ImportArtworkClicked => state.commands.push(AlbumEditorCommand::ImportArtworkFromFile),
        Message::RenumberZeroPaddedClicked => renumber(&mut state.tracks, RenumberStyle::ZeroPaddedToN),
        Message::RenumberOneToNClicked => renumber(&mut state.tracks, RenumberStyle::OneToN),
        Message::MoveRowUpClicked(index) => move_row_up(&mut state.tracks, index),
        Message::MoveRowDownClicked(index) => move_row_down(&mut state.tracks, index),
        Message::SaveClicked => {
            state.commands.push(AlbumEditorCommand::Save {
                track_ids: state.tracks.iter().map(|r| r.track_id.clone()).collect(),
                track_numbers: state.tracks.iter().map(|r| r.track_number.clone()).collect(),
                fields: state.fields.clone(),
            });
            state.close();
        }
        Message::CancelClicked => state.close(),
    }
}

pub fn view(state: &AlbumEditorState) -> Option<Element<'_, Message>> {
    if !state.open {
        return None;
    }

    let fields = column![
        text_field("Album Title", &state.fields.title, Message::TitleChanged),
        text_field("Album Artist", &state.fields.artist, Message::ArtistChanged),
        row![
            text_field("Year", &state.fields.year, Message::YearChanged),
            text_field("Genre", &state.fields.genre, Message::GenreChanged),
        ]
        .spacing(12),
    ]
    .spacing(8);

    let body = column![fields, artwork_row(state), tracklist(state)].spacing(14);

    let footer = row![
        button(text("Save").size(font::CALLOUT.size)).padding([8, 16]).on_press(Message::SaveClicked).style(|_theme, s| prominent_style(s)),
        button(text("Cancel").size(font::CALLOUT.size)).padding([8, 16]).on_press(Message::CancelClicked).style(|_theme, s| chip_style(s)),
    ]
    .spacing(8);

    let content = column![
        text("Edit Album Metadata").size(font::TITLE2.size).color(color::SILVER_LIGHT),
        scrollable(body).height(Length::Fixed(400.0)),
        footer,
    ]
    .spacing(10);

    Some(super::modal(content, 720.0, 640.0))
}

fn artwork_row(state: &AlbumEditorState) -> Element<'_, Message> {
    let tile = container(text("\u{266A}").size(iced::Pixels(22.0)).color(color::SILVER_MUTED))
        .width(Length::Fixed(72.0))
        .height(Length::Fixed(72.0))
        .center_x(Length::Fixed(72.0))
        .center_y(Length::Fixed(72.0))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(color::INPUT)),
            border: iced::Border {
                color: color::with_alpha(iced::Color::WHITE, 0.15),
                width: 1.0,
                radius: 8.0.into(),
            },
            ..container::Style::default()
        });

    let mut info = column![
        button(text("Import Unified Artwork\u{2026}").size(font::CALLOUT.size)).padding([8, 16]).on_press(Message::ImportArtworkClicked).style(|_theme, s| prominent_style(s)),
        text("Applies to every track plus a companion folder.jpg").size(font::CAPTION.size).color(color::SILVER_MUTED),
    ]
    .spacing(4);

    if let Some(compliance) = &state.artwork_compliance {
        if compliance.is_walkman_ready() {
            info = info.push(text(format!("{}\u{00D7}{} \u{2022} Walkman Ready", compliance.width, compliance.height)).size(font::CAPTION_BOLD.size).color(color::HIRES_GOLD));
        } else {
            for badge in compliance.badges() {
                info = info.push(text(badge).size(font::CAPTION.size).color(color::SILVER_MUTED));
            }
        }
    }

    row![tile, info].spacing(12).align_y(iced::alignment::Vertical::Center).into()
}

fn tracklist(state: &AlbumEditorState) -> Element<'_, Message> {
    let header = row![
        text("Tracklist").size(font::CAPTION_BOLD.size).color(color::SILVER_MUTED),
        iced::widget::Space::new(Length::Fill, Length::Shrink),
        button(text("Renumber 01..N").size(font::CAPTION.size)).padding([4, 8]).on_press(Message::RenumberZeroPaddedClicked).style(|_theme, s| chip_style(s)),
        button(text("Renumber 1..N").size(font::CAPTION.size)).padding([4, 8]).on_press(Message::RenumberOneToNClicked).style(|_theme, s| chip_style(s)),
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center);

    let row_count = state.tracks.len();
    let rows = state.tracks.iter().enumerate().fold(column![].spacing(4), |col, (index, track)| {
        let mut up = button(text("\u{2191}").size(font::CAPTION.size)).padding([2, 8]);
        if index > 0 {
            up = up.on_press(Message::MoveRowUpClicked(index));
        }
        let mut down = button(text("\u{2193}").size(font::CAPTION.size)).padding([2, 8]);
        if index + 1 < row_count {
            down = down.on_press(Message::MoveRowDownClicked(index));
        }

        col.push(
            row![
                text(track.track_number.as_str()).size(font::MONO_SMALL.size).color(color::SILVER_MUTED),
                text(track.title.as_str()).size(font::BODY.size).color(color::SILVER_LIGHT),
                iced::widget::Space::new(Length::Fill, Length::Shrink),
                down.style(|_theme, s| chip_style(s)),
                up.style(|_theme, s| chip_style(s)),
            ]
            .spacing(8)
            .align_y(iced::alignment::Vertical::Center),
        )
    });

    column![header, rows].spacing(6).into()
}

fn chip_style(status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(iced::Background::Color(if hovered { color::with_alpha(iced::Color::WHITE, 0.06) } else { color::with_alpha(color::CARD, 0.55) })),
        text_color: color::SILVER_LIGHT,
        border: iced::Border {
            color: color::BORDER,
            width: 1.0,
            radius: crate::iced_ui::theme::sizing::radius::BUTTON.into(),
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
            radius: crate::iced_ui::theme::sizing::radius::BUTTON.into(),
        },
        ..button::Style::default()
    }
}
