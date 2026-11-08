//! Song Metadata editor sheet for the iced shell, ported from
//! `crate::sheets::song_metadata` (`SongMetadataEditorSheet.swift`). Reuses that
//! module's pure types/logic unchanged — `SongEditorTab`, `SongEditorFields`,
//! `SongEditorCommand`, [`SongEditorState`] — and only rewrites `render` as this
//! module's `view`/`update`.
//!
//! Same lyrics-field `text_editor::Content` persistence note as
//! `iced_ui::sheets::lyrics_editor` (see that module's doc comment) — `shell.rs` owns
//! the `Content` alongside `SongEditorState` and keeps `fields.lyrics` in sync with it.
//! The artwork tab's drop zone is a plain placeholder container (no real artwork
//! texture loading exists on the egui side either).

use iced::widget::{button, column, container, row, scrollable, text, text_editor};
use iced::{Element, Length};

use crate::iced_ui::sheets::text_field;
use crate::iced_ui::theme::{color, font};
use crate::sheets::song_metadata::{SongEditorCommand, SongEditorState, SongEditorTab};

#[derive(Debug, Clone)]
pub enum Message {
    TabSelected(SongEditorTab),
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
    ImportArtworkClicked,
    ExportArtworkClicked,
    SaveClicked,
    CancelClicked,
}

pub fn update(state: &mut SongEditorState, lyrics_content: &mut text_editor::Content, message: Message) {
    match message {
        Message::TabSelected(tab) => state.tab = tab,
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
            state.commands.push(SongEditorCommand::SearchOnlineMetadata {
                track_id: state.track_id.clone(),
                artist: state.fields.artist.clone(),
                title: state.fields.title.clone(),
            });
        }
        Message::FindLyricsClicked => {
            state.commands.push(SongEditorCommand::SearchLyrics {
                track_id: state.track_id.clone(),
                artist: state.fields.artist.clone(),
                title: state.fields.title.clone(),
                album: state.fields.album.clone(),
                duration_seconds: state.track_duration_seconds,
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
        Message::ImportArtworkClicked => state.commands.push(SongEditorCommand::ImportArtworkFromFile(state.track_id.clone())),
        Message::ExportArtworkClicked => state.commands.push(SongEditorCommand::ExportArtworkToPc(state.track_id.clone())),
        Message::SaveClicked => {
            state.commands.push(SongEditorCommand::Save { track_id: state.track_id.clone(), fields: state.fields.clone() });
            state.close();
        }
        Message::CancelClicked => state.close(),
    }
}

pub fn view<'a>(state: &'a SongEditorState, lyrics_content: &'a text_editor::Content) -> Option<Element<'a, Message>> {
    if !state.open {
        return None;
    }

    let tabs = row![
        tab_chip(SongEditorTab::TagsAndSpecs, "Tags & Specs", state.tab),
        tab_chip(SongEditorTab::AlbumArtwork, "Album Artwork", state.tab),
    ]
    .spacing(6);

    let body = match state.tab {
        SongEditorTab::TagsAndSpecs => tags_and_specs_tab(state, lyrics_content),
        SongEditorTab::AlbumArtwork => artwork_tab(state),
    };

    let footer = row![
        button(text("Save").size(font::CALLOUT.size)).padding([8, 16]).on_press(Message::SaveClicked).style(|_theme, s| prominent_style(s)),
        button(text("Cancel").size(font::CALLOUT.size)).padding([8, 16]).on_press(Message::CancelClicked).style(|_theme, s| chip_style(s)),
    ]
    .spacing(8);

    let content = column![
        text("Edit Song Metadata").size(font::TITLE2.size).color(color::SILVER_LIGHT),
        tabs,
        scrollable(body).height(Length::Fixed(500.0)),
        footer,
    ]
    .spacing(10);

    Some(super::modal(content, 960.0, 700.0))
}

fn tags_and_specs_tab<'a>(state: &'a SongEditorState, lyrics_content: &'a text_editor::Content) -> Element<'a, Message> {
    let mut col = column![
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
        text_editor(lyrics_content).height(Length::Fixed(120.0)).on_action(Message::LyricsEdited),
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
        col = col.push(text("Online Matches").size(font::CAPTION_BOLD.size).color(color::SILVER_MUTED)).push(candidates);
    }

    if let Some(lyrics) = &state.lyrics_candidate {
        col = col.push(
            column![
                text("Lyrics Match").size(font::CAPTION_BOLD.size).color(color::SILVER_MUTED),
                text(lyrics.as_str()).size(font::MONO_SMALL.size).color(color::SILVER_LIGHT),
                button(text("Apply to Lyrics Field").size(font::CAPTION.size)).padding([4, 10]).on_press(Message::ApplyLyricsMatchClicked).style(|_theme, s| chip_style(s)),
            ]
            .spacing(4),
        );
    }

    col.into()
}

fn artwork_tab(state: &SongEditorState) -> Element<'_, Message> {
    let cover = container(text("\u{266A}").size(font::TITLE1.size).color(color::SILVER_MUTED))
        .width(Length::Fixed(200.0))
        .height(Length::Fixed(200.0))
        .center_x(Length::Fixed(200.0))
        .center_y(Length::Fixed(200.0))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(color::INPUT)),
            border: iced::Border {
                color: color::with_alpha(iced::Color::WHITE, 0.15),
                width: 1.0,
                radius: 10.0.into(),
            },
            ..container::Style::default()
        });

    let compliance: Element<'_, Message> = match &state.artwork_compliance {
        None => text("No artwork loaded").size(font::CAPTION.size).color(color::SILVER_MUTED).into(),
        Some(compliance) if compliance.is_walkman_ready() => {
            text(format!("{}\u{00D7}{} \u{2022} Walkman Ready", compliance.width, compliance.height)).size(font::CAPTION_BOLD.size).color(color::HIRES_GOLD).into()
        }
        Some(compliance) => compliance.badges().into_iter().fold(column![].spacing(2), |col, badge| {
            col.push(text(badge).size(font::CAPTION.size).color(color::SILVER_MUTED))
        }).into(),
    };

    row![
        cover,
        column![
            button(text("Import Artwork\u{2026}").size(font::CALLOUT.size)).padding([8, 16]).on_press(Message::ImportArtworkClicked).style(|_theme, s| prominent_style(s)),
            button(text("Export to PC\u{2026}").size(font::CALLOUT.size)).padding([8, 16]).on_press(Message::ExportArtworkClicked).style(|_theme, s| chip_style(s)),
            compliance,
        ]
        .spacing(6),
    ]
    .spacing(16)
    .into()
}

fn tab_chip<'a>(tab: SongEditorTab, label: &'static str, selected: SongEditorTab) -> Element<'a, Message> {
    let active = tab == selected;
    button(text(label).size(font::CALLOUT.size))
        .padding([6, 12])
        .on_press(Message::TabSelected(tab))
        .style(move |_theme, status| chip_style_active(active, status))
        .into()
}

fn chip_style(status: button::Status) -> button::Style {
    chip_style_active(false, status)
}

fn chip_style_active(active: bool, status: button::Status) -> button::Style {
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
            radius: crate::iced_ui::theme::sizing::radius::CAPSULE.into(),
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
