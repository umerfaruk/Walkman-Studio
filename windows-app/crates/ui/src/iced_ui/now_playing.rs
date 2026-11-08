//! The floating now-playing bar + fullscreen player overlay for the iced shell, ported
//! from `crate::now_playing` (`TrackPreviewBar.swift` / `FullScreenNowPlayingView.swift`).
//! Reuses that module's pure types/logic unchanged — [`NowPlayingPlaceholder`],
//! `player_bar_width`, `format_time` — and the real `walkman_services::player::Player`
//! binding (Milestone 7), calling its methods directly from `update()` rather than
//! relaying through the egui version's one-shot `PlayerRequests` flags (those exist
//! there only because egui's dispatch drains flags separately from rendering; iced's
//! `update()` already *is* that reducer, so there's nothing to relay through).
//!
//! Documented simplifications versus the egui version (both already documented as
//! simplifications there too, carried over unchanged): no real artwork texture exists
//! (a placeholder note-glyph tile), and the fullscreen "blurred ambient artwork"
//! background is a flat top-to-bottom tint wash rather than a live blur. New
//! simplification for this port: the egui version also collapses fullscreen on Esc; wiring
//! a global key-press subscription for just that is deferred — the collapse button covers
//! the same action.

use iced::widget::{button, column, container, row, slider, text};
use iced::{Element, Length};

use walkman_services::walkman_engine::{active_lyric_line_index, LrcLine};

use crate::iced_ui::theme::{color, font, sizing};
use crate::iced_ui::widgets::glass;
use crate::now_playing::{format_time, NowPlayingPlaceholder, PLAYER_BAR_HEIGHT};

/// How many lyric lines surround the active one are shown at once, same as the egui
/// version's `LYRICS_CONTEXT_LINES`.
const LYRICS_CONTEXT_LINES: usize = 2;

#[derive(Debug, Clone)]
pub enum Message {
    PreviousClicked,
    PlayPauseClicked,
    NextClicked,
    SeekChanged(f64),
    SeekReleased,
    InspectorClicked,
    FullscreenClicked,
    CollapseClicked,
}

/// Bar/overlay state, separate from the player itself — `shell.rs` owns the real
/// `Option<Arc<Player>>` and calls its methods directly in response to these messages
/// (mirroring the egui version's `transport_controls`/`scrubber_row` handlers, which also
/// call the player directly alongside raising their now-vestigial flags).
pub struct NowPlayingState {
    pub now_playing: Option<NowPlayingPlaceholder>,
    pub is_fullscreen: bool,
    pub seek_target: Option<f64>,
    pub lyrics_lines: Vec<LrcLine>,
    pub lyrics_lead_seconds: f64,
}

impl Default for NowPlayingState {
    fn default() -> Self {
        Self {
            now_playing: None,
            is_fullscreen: false,
            seek_target: None,
            lyrics_lines: Vec::new(),
            lyrics_lead_seconds: 0.3,
        }
    }
}

pub fn bar(state: &NowPlayingState) -> Element<'_, Message> {
    let content = row![
        transport_controls(state),
        container(center_display(state)).width(Length::Fill).padding([0, 10]),
        row![fullscreen_button(state), inspector_button()].spacing(6),
    ]
    .align_y(iced::alignment::Vertical::Center)
    .padding([7, 14]);

    container(content)
        .width(Length::Fill)
        .max_width(sizing::PLAYER_MAX_WIDTH)
        .height(Length::Fixed(PLAYER_BAR_HEIGHT))
        .style(|_theme| glass::studio_glass_card())
        .into()
}

fn transport_controls(state: &NowPlayingState) -> Element<'_, Message> {
    let enabled = state.now_playing.is_some();
    let playing = state.now_playing.as_ref().map(|t| t.is_playing).unwrap_or(false);

    let mut previous = glyph_button("\u{23EE}", 30.0);
    let mut play_pause = glyph_button(if playing { "\u{23F8}" } else { "\u{25B6}" }, 36.0);
    let mut next = glyph_button("\u{23ED}", 30.0);
    if enabled {
        previous = previous.on_press(Message::PreviousClicked);
        play_pause = play_pause.on_press(Message::PlayPauseClicked);
        next = next.on_press(Message::NextClicked);
    }

    row![previous.style(|_theme, s| chip_style(false, s)), play_pause.style(|_theme, s| prominent_style(s)), next.style(|_theme, s| chip_style(false, s))]
        .spacing(8)
        .into()
}

fn glyph_button(glyph: &'static str, size: f32) -> iced::widget::Button<'static, Message> {
    button(container(text(glyph).size(font::BODY.size)).center_x(Length::Fixed(size)).center_y(Length::Fixed(size))).width(Length::Fixed(size)).height(Length::Fixed(size)).padding(0)
}

fn center_display(state: &NowPlayingState) -> Element<'_, Message> {
    match &state.now_playing {
        Some(track) => active_display(track, state.seek_target),
        None => text("Walkman Studio").size(font::CALLOUT.size).color(color::SILVER_MUTED).into(),
    }
}

fn active_display(track: &NowPlayingPlaceholder, seek_target: Option<f64>) -> Element<'_, Message> {
    let art = container(text("\u{266A}").size(font::BODY.size).color(color::SILVER_MUTED))
        .width(Length::Fixed(34.0))
        .height(Length::Fixed(34.0))
        .center_x(Length::Fixed(34.0))
        .center_y(Length::Fixed(34.0))
        .style(|_theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(color::INPUT)),
            border: iced::Border {
                color: color::with_alpha(iced::Color::WHITE, 0.12),
                width: 1.0,
                radius: 5.0.into(),
            },
            ..iced::widget::container::Style::default()
        });

    let title = if track.title.trim().is_empty() { "Unknown Title" } else { track.title.as_str() };
    let mut title_row = row![text(title).size(font::CALLOUT.size).color(if track.is_playing { color::SILVER_LIGHT } else { color::SILVER })].spacing(6);
    if !track.artist.trim().is_empty() {
        title_row = title_row.push(text("\u{2022}").size(font::CAPTION.size).color(color::SILVER_MUTED));
        title_row = title_row.push(text(track.artist.as_str()).size(font::CAPTION.size).color(color::SILVER_MUTED));
    }
    if track.is_hi_res {
        title_row = title_row.push(
            container(text("HR").size(font::BADGE.size)).padding([1, 5]).style(|_theme| crate::iced_ui::widgets::pill::hi_res_badge()),
        );
    }

    row![art, column![title_row, scrubber_row(track, seek_target)].spacing(2)].spacing(8).align_y(iced::alignment::Vertical::Center).into()
}

fn scrubber_row(track: &NowPlayingPlaceholder, seek_target: Option<f64>) -> Element<'_, Message> {
    let duration = track.duration_secs.max(1.0);
    let pos = seek_target.unwrap_or(track.position_secs).clamp(0.0, duration);

    row![
        text(format_time(track.position_secs)).size(font::MONO_SMALL.size).color(color::SILVER_MUTED),
        slider(0.0..=duration, pos, Message::SeekChanged).on_release(Message::SeekReleased).step(1.0).width(Length::Fill),
        text(format_time(track.duration_secs)).size(font::MONO_SMALL.size).color(color::SILVER_MUTED),
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center)
    .into()
}

fn inspector_button<'a>() -> Element<'a, Message> {
    glyph_button("\u{2139}", 30.0).on_press(Message::InspectorClicked).style(|_theme, s| chip_style(false, s)).into()
}

fn fullscreen_button(state: &NowPlayingState) -> Element<'_, Message> {
    let is_fullscreen = state.is_fullscreen;
    glyph_button("\u{26F6}", 30.0).on_press(Message::FullscreenClicked).style(move |_theme, s| chip_style(is_fullscreen, s)).into()
}

fn chip_style(active: bool, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let fill = if active {
        color::HIRES_GOLD
    } else if hovered {
        color::with_alpha(iced::Color::WHITE, 0.10)
    } else {
        color::with_alpha(color::CARD, 0.55)
    };
    button::Style {
        background: Some(iced::Background::Color(fill)),
        text_color: if active { color::OBSIDIAN } else { color::SILVER_LIGHT },
        border: iced::Border {
            color: color::BORDER,
            width: 1.0,
            radius: sizing::radius::ICON_BADGE.into(),
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
            radius: sizing::radius::ICON_BADGE.into(),
        },
        ..button::Style::default()
    }
}

pub fn fullscreen_overlay(state: &NowPlayingState) -> Option<Element<'_, Message>> {
    if !state.is_fullscreen {
        return None;
    }

    let collapse = container(
        button(text("\u{2715}").size(font::TITLE3.size))
            .padding(8)
            .on_press(Message::CollapseClicked)
            .style(|_theme, s| chip_style(false, s)),
    )
    .width(Length::Fill)
    .align_x(iced::alignment::Horizontal::Right)
    .padding(16);

    let body: Element<'_, Message> = match &state.now_playing {
        Some(track) => column![
            fullscreen_artwork(track),
            fullscreen_title_block(track),
            fullscreen_lyrics(state, track),
        ]
        .spacing(20)
        .align_x(iced::alignment::Horizontal::Center)
        .into(),
        None => text("Nothing playing").size(font::TITLE1.size).color(color::SILVER_MUTED).into(),
    };

    let mut footer = column![transport_controls(state)].align_x(iced::alignment::Horizontal::Center).spacing(14);
    if let Some(track) = &state.now_playing {
        footer = footer.push(container(scrubber_row(track, state.seek_target)).width(Length::Fixed(480.0)));
    }

    let content = column![
        collapse,
        container(body).width(Length::Fill).height(Length::Fill).center_x(Length::Fill).center_y(Length::Fill),
        container(footer).width(Length::Fill).align_x(iced::alignment::Horizontal::Center).padding(32),
    ]
    .height(Length::Fill);

    Some(
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(color::OBSIDIAN)),
                ..iced::widget::container::Style::default()
            })
            .into(),
    )
}

fn fullscreen_artwork(track: &NowPlayingPlaceholder) -> Element<'_, Message> {
    let tile = container(text("\u{266A}").size(iced::Pixels(48.0)).color(color::SILVER_MUTED))
        .width(Length::Fixed(220.0))
        .height(Length::Fixed(220.0))
        .center_x(Length::Fixed(220.0))
        .center_y(Length::Fixed(220.0))
        .style(|_theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(color::INPUT)),
            border: iced::Border {
                color: color::with_alpha(iced::Color::WHITE, 0.12),
                width: 1.0,
                radius: 14.0.into(),
            },
            ..iced::widget::container::Style::default()
        });

    if track.is_hi_res {
        column![tile, container(text("HR").size(font::BADGE.size)).padding([2, 6]).style(|_theme| crate::iced_ui::widgets::pill::hi_res_badge())]
            .spacing(10)
            .align_x(iced::alignment::Horizontal::Center)
            .into()
    } else {
        tile.into()
    }
}

fn fullscreen_title_block(track: &NowPlayingPlaceholder) -> Element<'_, Message> {
    let title = if track.title.trim().is_empty() { "Unknown Title" } else { track.title.as_str() };
    let mut col = column![text(title).size(font::TITLE2.size).color(color::SILVER_LIGHT)].align_x(iced::alignment::Horizontal::Center).spacing(4);
    if !track.artist.trim().is_empty() {
        col = col.push(text(track.artist.as_str()).size(font::CALLOUT.size).color(color::SILVER_MUTED));
    }
    col.into()
}

fn fullscreen_lyrics<'a>(state: &'a NowPlayingState, track: &'a NowPlayingPlaceholder) -> Element<'a, Message> {
    if state.lyrics_lines.is_empty() {
        return column![].into();
    }
    let Some(active) = active_lyric_line_index(&state.lyrics_lines, track.position_secs, state.lyrics_lead_seconds) else {
        return column![].into();
    };
    let start = active.saturating_sub(LYRICS_CONTEXT_LINES);
    let end = (active + LYRICS_CONTEXT_LINES + 1).min(state.lyrics_lines.len());

    state.lyrics_lines[start..end].iter().enumerate().fold(column![].align_x(iced::alignment::Horizontal::Center).spacing(6), |col, (i, line)| {
        let index = start + i;
        let (size, line_color) = if index == active {
            (font::TITLE3.size, color::SILVER_LIGHT)
        } else {
            (font::CALLOUT.size, color::with_alpha(color::SILVER_MUTED, 0.7))
        };
        col.push(text(line.text.as_str()).size(size).color(line_color))
    }).into()
}
