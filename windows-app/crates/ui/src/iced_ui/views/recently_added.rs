//! Recently Added view for the iced shell, ported from `crate::views::recently_added`
//! (`RecentlyAddedView.swift`). Reuses that module's pure types/logic unchanged —
//! `TimeScope`, `RecentlyAddedCommand`, [`RecentlyAddedViewState`], and `ranked_tracks` —
//! and only rewrites `render` as this module's `view`/`update`.
//!
//! Documented simplifications (same as `local_library`'s): multi-select via ctrl/
//! shift-click collapses to single-select, and this pass drops the egui version's
//! right-click context menu (Play/Edit Metadata/Trim & Cut/Transfer/Delete per row) since
//! iced has no built-in context-menu widget — those actions return once a view needs a
//! context-menu equivalent (a later milestone, not blocking this one).

use chrono::Utc;
use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length};

use walkman_domain::models::Track;

use crate::iced_ui::theme::{color, font, sizing};
use crate::iced_ui::widgets;
use crate::views::recently_added::{ranked_tracks, RecentlyAddedCommand, RecentlyAddedViewState, TimeScope};

#[derive(Debug, Clone)]
pub enum Message {
    ScopeSelected(TimeScope),
    TrackClicked(String),
    FindLyricsClicked,
}

pub fn update(state: &mut RecentlyAddedViewState, message: Message) {
    match message {
        Message::ScopeSelected(scope) => state.scope = scope,
        Message::TrackClicked(path) => {
            state.selected.clear();
            state.selected.push(path);
        }
        Message::FindLyricsClicked => {
            state.commands.push(RecentlyAddedCommand::BatchLyrics(state.selected.clone()));
        }
    }
}

pub fn view<'a>(tracks: &'a [Track], state: &'a RecentlyAddedViewState) -> Element<'a, Message> {
    let scope_row = TimeScope::ALL.iter().fold(row![].spacing(6), |r, &scope| {
        let active = state.scope == scope;
        r.push(
            button(text(scope.title()).size(font::CALLOUT.size))
                .padding([6, 12])
                .on_press(Message::ScopeSelected(scope))
                .style(move |_theme, status| tab_style(active, status)),
        )
    });

    let has_selection = !state.selected.is_empty();
    let mut find_lyrics = button(text("Find Lyrics\u{2026}").size(font::CALLOUT.size)).padding([6, 12]);
    if has_selection {
        find_lyrics = find_lyrics.on_press(Message::FindLyricsClicked);
    }
    let find_lyrics = find_lyrics.style(|_theme, status| tab_style(false, status));

    let toolbar = row![scope_row, find_lyrics].spacing(12);

    let ranked = ranked_tracks(tracks, state.scope, Utc::now());
    let list = ranked.into_iter().enumerate().fold(column![].spacing(4), |col, (index, track)| {
        let selected = state.selected.contains(&track.full_path);
        col.push(track_row(index, track, selected))
    });

    let body = scrollable(list.padding(iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: sizing::TABLE_BOTTOM_INSET,
        left: 0.0,
    }))
    .height(Length::Fill);

    column![toolbar, body].spacing(12).padding(16).into()
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

fn track_row<'a>(index: usize, track: Track, selected: bool) -> Element<'a, Message> {
    let hi_res = track.bits_per_sample > 16 || track.sample_rate > 48_000;
    let path = track.full_path.clone();

    let mut content = row![
        text(format!("#{}", index + 1)).size(font::MONO.size).color(color::SILVER_MUTED),
        text(track.title).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
        text(track.artist).size(font::BODY.size).color(color::SILVER_MUTED),
    ]
    .spacing(12)
    .align_y(iced::alignment::Vertical::Center);

    if hi_res {
        content = content.push(
            container(text("HR").size(font::BADGE.size))
                .padding([2, 6])
                .style(|_theme| widgets::pill::hi_res_badge()),
        );
    }

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
