//! Transfer Queue Item editor sheet for the iced shell, ported from
//! `crate::sheets::transfer_item` (`TransferQueueItemEditorSheet.swift`). Reuses that
//! module's pure types/logic unchanged — `status_label`, `TransferItemEditorCommand`,
//! [`TransferItemEditorState`] — plus `SongEditorFields` (its working-copy fields are
//! identical to the Song Metadata editor's, not a parallel field set) — and only
//! rewrites `render` as this module's `view`/`update`.
//!
//! Same lyrics-field `text_editor::Content` persistence note as
//! `iced_ui::sheets::song_metadata`/`lyrics_editor` — `shell.rs` owns a separate
//! `Content` for this sheet alongside its state.

use iced::widget::{button, column, row, scrollable, text, text_editor};
use iced::{Element, Length};

use crate::iced_ui::sheets::text_field;
use crate::iced_ui::theme::{color, font};
use crate::sheets::transfer_item::{status_label, TransferItemEditorCommand, TransferItemEditorState};

#[derive(Debug, Clone)]
pub enum Message {
    TitleChanged(String),
    ArtistChanged(String),
    AlbumChanged(String),
    TrackNumberChanged(String),
    DiscNumberChanged(String),
    YearChanged(String),
    GenreChanged(String),
    LyricsEdited(text_editor::Action),
    SearchOnlineClicked,
    FindLyricsClicked,
    ApplyCandidateClicked(usize),
    ApplyLyricsMatchClicked,
    SaveClicked,
    CancelClicked,
}

pub fn update(state: &mut TransferItemEditorState, lyrics_content: &mut text_editor::Content, message: Message) {
    match message {
        Message::TitleChanged(v) => state.fields.title = v,
        Message::ArtistChanged(v) => state.fields.artist = v,
        Message::AlbumChanged(v) => state.fields.album = v,
        Message::TrackNumberChanged(v) => state.fields.track_number = v,
        Message::DiscNumberChanged(v) => state.fields.disc_number = v,
        Message::YearChanged(v) => state.fields.year = v,
        Message::GenreChanged(v) => state.fields.genre = v,
        Message::LyricsEdited(action) => {
            lyrics_content.perform(action);
            state.fields.lyrics = lyrics_content.text();
        }
        Message::SearchOnlineClicked => {
            state.commands.push(TransferItemEditorCommand::SearchOnlineMetadata {
                item_id: state.item_id.clone(),
                artist: state.fields.artist.clone(),
                title: state.fields.title.clone(),
            });
        }
        Message::FindLyricsClicked => {
            state.commands.push(TransferItemEditorCommand::SearchLyrics {
                item_id: state.item_id.clone(),
                artist: state.fields.artist.clone(),
                title: state.fields.title.clone(),
                album: state.fields.album.clone(),
                duration_seconds: state.duration_seconds,
            });
        }
        Message::ApplyCandidateClicked(index) => {
            if let Some(candidate) = state.online_candidates.get(index).cloned() {
                state.fields.apply_candidate(&candidate);
            }
        }
        Message::ApplyLyricsMatchClicked => {
            if let Some(lyrics) = state.lyrics_candidate.clone() {
                state.fields.lyrics = lyrics;
                *lyrics_content = text_editor::Content::with_text(&state.fields.lyrics);
            }
        }
        Message::SaveClicked => {
            state.commands.push(TransferItemEditorCommand::Save { item_id: state.item_id.clone(), fields: state.fields.clone() });
            state.close();
        }
        Message::CancelClicked => state.close(),
    }
}

pub fn view<'a>(state: &'a TransferItemEditorState, lyrics_content: &'a text_editor::Content) -> Option<Element<'a, Message>> {
    if !state.open {
        return None;
    }

    let mut header = row![text("Edit Transfer Item").size(font::TITLE2.size).color(color::SILVER_LIGHT)].spacing(10);
    if let Some(status) = &state.status {
        header = header.push(text(status_label(status)).size(font::CAPTION_BOLD.size).color(color::SILVER_MUTED));
    }

    let mut body = column![
        text_field("Title", &state.fields.title, Message::TitleChanged),
        text_field("Artist", &state.fields.artist, Message::ArtistChanged),
        text_field("Album", &state.fields.album, Message::AlbumChanged),
        row![
            text_field("Track Number", &state.fields.track_number, Message::TrackNumberChanged),
            text_field("Disc Number", &state.fields.disc_number, Message::DiscNumberChanged),
        ]
        .spacing(12),
        row![
            text_field("Year", &state.fields.year, Message::YearChanged),
            text_field("Genre", &state.fields.genre, Message::GenreChanged),
        ]
        .spacing(12),
        text("Lyrics").size(font::CAPTION.size).color(color::SILVER_MUTED),
        text_editor(lyrics_content).height(Length::Fixed(100.0)).on_action(Message::LyricsEdited),
        row![
            button(text("Search Online Metadata\u{2026}").size(font::CALLOUT.size)).padding([6, 12]).on_press(Message::SearchOnlineClicked).style(|_theme, s| chip_style(s)),
            button(text("Find Lyrics\u{2026}").size(font::CALLOUT.size)).padding([6, 12]).on_press(Message::FindLyricsClicked).style(|_theme, s| chip_style(s)),
        ]
        .spacing(8),
    ]
    .spacing(8);

    if !state.online_candidates.is_empty() {
        let candidates = state.online_candidates.iter().enumerate().fold(column![].spacing(4), |col, (index, candidate)| {
            col.push(
                row![
                    text(format!("{} \u{2014} {} ({})", candidate.title, candidate.artist, candidate.album)).size(font::CALLOUT.size).color(color::SILVER_LIGHT),
                    iced::widget::Space::new(Length::Fill, Length::Shrink),
                    button(text("Apply").size(font::CAPTION.size)).padding([2, 8]).on_press(Message::ApplyCandidateClicked(index)).style(|_theme, s| chip_style(s)),
                ]
                .spacing(8)
                .align_y(iced::alignment::Vertical::Center),
            )
        });
        body = body.push(text("Online Matches").size(font::CAPTION_BOLD.size).color(color::SILVER_MUTED)).push(candidates);
    }

    if let Some(lyrics) = &state.lyrics_candidate {
        body = body.push(
            column![
                text("Lyrics Match").size(font::CAPTION_BOLD.size).color(color::SILVER_MUTED),
                text(lyrics.as_str()).size(font::MONO_SMALL.size).color(color::SILVER_LIGHT),
                button(text("Apply to Lyrics Field").size(font::CAPTION.size)).padding([4, 10]).on_press(Message::ApplyLyricsMatchClicked).style(|_theme, s| chip_style(s)),
            ]
            .spacing(4),
        );
    }

    let footer = row![
        button(text("Save").size(font::CALLOUT.size)).padding([8, 16]).on_press(Message::SaveClicked).style(|_theme, s| prominent_style(s)),
        button(text("Cancel").size(font::CALLOUT.size)).padding([8, 16]).on_press(Message::CancelClicked).style(|_theme, s| chip_style(s)),
    ]
    .spacing(8);

    let content = column![header, scrollable(body).height(Length::Fixed(400.0)), footer].spacing(10);
    Some(super::modal(content, 720.0, 600.0))
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
