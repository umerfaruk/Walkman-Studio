//! SensMe\u{2122} Channels view for the iced shell, ported from `crate::views::senseme`
//! (`SenseMeChannelsView.swift`). Reuses that module's pure types/logic unchanged —
//! `SenseMeCommand`, [`SenseMeViewState`], `tracks_for_channel`, `channel_counts`,
//! `unanalyzed_track_ids`, `select_next`/`select_previous`, `shuffled_ids` — and only
//! rewrites `render` as this module's `view`/`update`. The egui version already
//! documents its own simplification (a flat strip of tiles instead of a true 3D
//! cover-flow carousel, since egui's 2D painter has no perspective transform); the iced
//! port keeps that same flat-strip approximation, just declaratively laid out instead of
//! hand-painted.

use iced::widget::{button, column, container, row, text};
use iced::{Element, Length};

use walkman_domain::models::Track;
use walkman_domain::senseme::SenseMeChannel;

use crate::iced_ui::theme::{color, font};
use crate::iced_ui::widgets;
use crate::views::senseme::{channel_counts, select_next, select_previous, shuffled_ids, tracks_for_channel, unanalyzed_track_ids, SenseMeCommand, SenseMeViewState};

const CAROUSEL_SIDE_TILES: isize = 2;
const TILE_SIZE: f32 = 120.0;
const SELECTED_TILE_SIZE: f32 = 150.0;

#[derive(Debug, Clone)]
pub enum Message {
    PreviousClicked,
    NextClicked,
    TileClicked(usize),
    ShuffleAllClicked,
    AnalyzeClicked,
    PlaySelectedClicked,
    ShuffleSelectedClicked,
}

pub fn update(tracks: &[Track], state: &mut SenseMeViewState, message: Message) {
    match message {
        Message::PreviousClicked => state.selected = select_previous(state.selected),
        Message::NextClicked => state.selected = select_next(state.selected),
        Message::TileClicked(index) => state.selected = index,
        Message::ShuffleAllClicked => {
            let ids: Vec<String> = tracks_for_channel(tracks, SenseMeChannel::ShuffleAllSongs).iter().map(|t| t.id().to_owned()).collect();
            state.shuffle_seed = state.shuffle_seed.wrapping_add(1);
            state.commands.push(SenseMeCommand::ShuffleChannel(shuffled_ids(&ids, state.shuffle_seed)));
        }
        Message::AnalyzeClicked => {
            state.commands.push(SenseMeCommand::AnalyzeUnanalyzed(unanalyzed_track_ids(tracks)));
        }
        Message::PlaySelectedClicked => {
            let channel = SenseMeChannel::ALL[state.selected];
            let ids: Vec<String> = tracks_for_channel(tracks, channel).iter().map(|t| t.id().to_owned()).collect();
            state.commands.push(SenseMeCommand::PlayChannel(ids));
        }
        Message::ShuffleSelectedClicked => {
            let channel = SenseMeChannel::ALL[state.selected];
            let ids: Vec<String> = tracks_for_channel(tracks, channel).iter().map(|t| t.id().to_owned()).collect();
            state.shuffle_seed = state.shuffle_seed.wrapping_add(1);
            state.commands.push(SenseMeCommand::ShuffleChannel(shuffled_ids(&ids, state.shuffle_seed)));
        }
    }
}

pub fn view<'a>(tracks: &[Track], state: &'a SenseMeViewState) -> Element<'a, Message> {
    column![toolbar(tracks), carousel(tracks, state), selected_detail(tracks, state)]
        .spacing(20)
        .padding(16)
        .into()
}

fn toolbar<'a>(tracks: &[Track]) -> Element<'a, Message> {
    let shuffle_all_has_tracks = !tracks_for_channel(tracks, SenseMeChannel::ShuffleAllSongs).is_empty();
    let unanalyzed_count = unanalyzed_track_ids(tracks).len();

    let mut shuffle_all = button(text("Shuffle All Songs").size(font::CALLOUT.size)).padding([8, 14]);
    if shuffle_all_has_tracks {
        shuffle_all = shuffle_all.on_press(Message::ShuffleAllClicked);
    }

    let mut analyze = button(text(format!("Analyze Library ({unanalyzed_count})")).size(font::CALLOUT.size)).padding([8, 14]);
    if unanalyzed_count > 0 {
        analyze = analyze.on_press(Message::AnalyzeClicked);
    }

    row![
        text("SensMe\u{2122} Channels").size(font::TITLE2.size).color(color::SILVER_LIGHT),
        iced::widget::Space::new(Length::Fixed(16.0), Length::Shrink),
        shuffle_all.style(|_theme, status| gold_style(status)),
        analyze.style(|_theme, status| chip_style(false, status)),
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center)
    .into()
}

fn carousel<'a>(tracks: &[Track], state: &'a SenseMeViewState) -> Element<'a, Message> {
    let counts = channel_counts(tracks);
    let len = SenseMeChannel::ALL.len() as isize;

    let mut tiles = row![].spacing(10);
    for offset in -CAROUSEL_SIDE_TILES..=CAROUSEL_SIDE_TILES {
        let index = (state.selected as isize + offset).rem_euclid(len) as usize;
        tiles = tiles.push(tile(index, counts[index], index == state.selected));
    }

    row![
        nav_button("\u{2039}", Message::PreviousClicked),
        container(tiles).center_x(Length::Fill),
        nav_button("\u{203A}", Message::NextClicked),
    ]
    .spacing(12)
    .align_y(iced::alignment::Vertical::Center)
    .into()
}

fn nav_button<'a>(label: &'static str, message: Message) -> Element<'a, Message> {
    button(text(label).size(font::TITLE2.size))
        .padding([8, 14])
        .on_press(message)
        .style(|_theme, status| chip_style(false, status))
        .into()
}

fn tile<'a>(index: usize, count: usize, selected: bool) -> Element<'a, Message> {
    let channel = SenseMeChannel::ALL[index];
    let size = if selected { SELECTED_TILE_SIZE } else { TILE_SIZE };

    let content = column![
        text(channel.display_name()).size(font::CALLOUT.size).color(if selected { color::HIRES_GOLD } else { color::SILVER_LIGHT }),
        text(format!("{count} tracks")).size(font::CAPTION.size).color(color::SILVER_MUTED),
    ]
    .spacing(4)
    .align_x(iced::alignment::Horizontal::Center);

    button(
        container(content)
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .center_x(Length::Fixed(size))
            .center_y(Length::Fixed(size)),
    )
    .on_press(Message::TileClicked(index))
    .style(move |_theme, _status| {
        let glass = widgets::glass::liquid_glass(selected);
        button::Style {
            background: glass.background,
            text_color: color::SILVER_LIGHT,
            border: glass.border,
            shadow: glass.shadow,
        }
    })
    .into()
}

fn selected_detail<'a>(tracks: &[Track], state: &'a SenseMeViewState) -> Element<'a, Message> {
    let channel = SenseMeChannel::ALL[state.selected];
    let channel_ids: Vec<String> = tracks_for_channel(tracks, channel).iter().map(|t| t.id().to_owned()).collect();
    let has_tracks = !channel_ids.is_empty();
    let count = channel_ids.len();

    let mut play = button(text("Play").size(font::CALLOUT.size)).padding([8, 16]);
    if has_tracks {
        play = play.on_press(Message::PlaySelectedClicked);
    }
    let mut shuffle = button(text("Shuffle").size(font::CALLOUT.size)).padding([8, 16]);
    if has_tracks {
        shuffle = shuffle.on_press(Message::ShuffleSelectedClicked);
    }

    column![
        text(channel.display_name()).size(font::TITLE1.size).color(color::SILVER_LIGHT),
        text(format!("{count} track{}", if count == 1 { "" } else { "s" })).size(font::CALLOUT.size).color(color::SILVER_MUTED),
        row![
            play.style(|_theme, status| prominent_style(status)),
            shuffle.style(|_theme, status| chip_style(false, status)),
        ]
        .spacing(8),
    ]
    .spacing(8)
    .align_x(iced::alignment::Horizontal::Center)
    .width(Length::Fill)
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
            radius: crate::iced_ui::theme::sizing::radius::CAPSULE.into(),
        },
        ..button::Style::default()
    }
}

fn gold_style(status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(iced::Background::Color(if hovered {
            color::lighten(color::HIRES_GOLD, 0.08)
        } else {
            color::HIRES_GOLD
        })),
        text_color: color::OBSIDIAN,
        border: iced::Border {
            color: color::with_alpha(iced::Color::WHITE, 0.25),
            width: 1.0,
            radius: crate::iced_ui::theme::sizing::radius::BUTTON.into(),
        },
        ..button::Style::default()
    }
}

fn prominent_style(status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(iced::Background::Color(if hovered {
            color::lighten(color::SILVER, 0.08)
        } else {
            color::SILVER
        })),
        text_color: color::OBSIDIAN,
        border: iced::Border {
            color: color::with_alpha(iced::Color::WHITE, 0.25),
            width: 1.0,
            radius: crate::iced_ui::theme::sizing::radius::BUTTON.into(),
        },
        ..button::Style::default()
    }
}
