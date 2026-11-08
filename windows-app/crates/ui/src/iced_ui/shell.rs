//! Application shell, ported from `crate::shell` (egui) / `ContentView.swift`.
//!
//! Milestone 3 of the egui-to-iced migration plan: the three-column layout — a fixed
//! 260pt sidebar, a flexible central detail pane, and an optional 300pt trailing
//! inspector — plus the floating now-playing bar overlaid at the bottom center. Detail
//! panes are placeholders (just the section title) until the view-by-view milestone;
//! the sidebar uses the real, framework-agnostic [`crate::NavTab`] (no egui dependency)
//! so this already matches the real 13-section nav order/titles.

use iced::widget::{button, column, container, row, text, Row, Space};
use iced::{Border, Element, Length, Padding};

use crate::iced_ui::theme::{color, font, sizing};
use crate::iced_ui::widgets;
use crate::NavTab;

/// Shell-level state: which section is selected and whether the inspector is open.
/// Grows as later milestones port more of the egui `AppState` (device/track data,
/// per-view/per-sheet state, player state, …) into this module.
#[derive(Debug, Clone)]
pub struct AppState {
    pub nav_tab: NavTab,
    pub show_inspector: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            // Defaults to Local Music Library on launch, matching the egui shell.
            nav_tab: NavTab::LocalLibrary,
            show_inspector: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    NavSelected(NavTab),
    ToggleInspector,
}

pub fn update(state: &mut AppState, message: Message) {
    match message {
        Message::NavSelected(tab) => state.nav_tab = tab,
        Message::ToggleInspector => state.show_inspector = !state.show_inspector,
    }
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let mut body: Row<'_, Message> = row![sidebar(state), detail_pane(state)];
    if state.show_inspector {
        body = body.push(inspector_pane());
    }
    let body = body.width(Length::Fill).height(Length::Fill);

    iced::widget::stack![
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(iced::Background::Color(color::BACKGROUND)),
                ..container::Style::default()
            }),
        container(now_playing_bar())
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Bottom)
            .padding(Padding {
                top: 0.0,
                right: 0.0,
                bottom: 16.0,
                left: 0.0,
            }),
    ]
    .into()
}

fn sidebar(state: &AppState) -> Element<'_, Message> {
    let items = NavTab::ALL.iter().fold(column![].spacing(2), |col, &tab| {
        let selected = tab == state.nav_tab;
        col.push(
            button(text(tab.title()).size(font::BODY.size))
                .width(Length::Fill)
                .padding([8, 12])
                .on_press(Message::NavSelected(tab))
                .style(move |_theme, status| sidebar_item_style(selected, status)),
        )
    });

    container(column![
        container(text("Walkman Studio").size(font::TITLE3.size)).padding(Padding {
            top: 16.0,
            right: 16.0,
            bottom: 12.0,
            left: 16.0,
        }),
        iced::widget::scrollable(items.padding([0, 8])).height(Length::Fill),
    ])
    .width(Length::Fixed(sizing::SIDEBAR_WIDTH))
    .height(Length::Fill)
    .style(|_theme| container::Style {
        background: Some(iced::Background::Color(color::OBSIDIAN)),
        border: Border {
            color: color::BORDER,
            width: 1.0,
            ..Border::default()
        },
        ..container::Style::default()
    })
    .into()
}

fn sidebar_item_style(selected: bool, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let fill = if selected {
        color::with_alpha(iced::Color::WHITE, 0.14)
    } else if hovered {
        color::with_alpha(iced::Color::WHITE, 0.06)
    } else {
        iced::Color::TRANSPARENT
    };
    button::Style {
        background: Some(iced::Background::Color(fill)),
        text_color: if selected {
            color::SILVER_LIGHT
        } else {
            color::SILVER_MUTED
        },
        border: Border {
            radius: 8.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn detail_pane(state: &AppState) -> Element<'_, Message> {
    container(column![
        text(state.nav_tab.title()).size(font::TITLE1.size),
        Space::new(Length::Fill, Length::Fixed(8.0)),
        text("View not ported yet.")
            .size(font::BODY.size)
            .color(color::SILVER_MUTED),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding {
        top: 24.0,
        right: 24.0,
        bottom: sizing::TABLE_BOTTOM_INSET,
        left: 24.0,
    })
    .into()
}

fn inspector_pane<'a, Message: 'a>() -> Element<'a, Message> {
    container(column![
        text("Inspector").size(font::HEADLINE.size),
        Space::new(Length::Fill, Length::Fixed(8.0)),
        text("Select a track to inspect.")
            .size(font::BODY.size)
            .color(color::SILVER_MUTED),
    ])
    .padding(16)
    .width(Length::Fixed(sizing::INSPECTOR_WIDTH))
    .height(Length::Fill)
    .style(|_theme| container::Style {
        background: Some(iced::Background::Color(color::CARD)),
        border: Border {
            color: color::BORDER,
            width: 1.0,
            ..Border::default()
        },
        ..container::Style::default()
    })
    .into()
}

fn now_playing_bar<'a, Message: 'a>() -> Element<'a, Message> {
    container(
        row![text("Not Playing")
            .size(font::BODY.size)
            .color(color::SILVER_MUTED)]
        .padding([10, 16]),
    )
    .width(Length::Fixed(sizing::PLAYER_MAX_WIDTH))
    .style(|_theme| widgets::glass::studio_glass_card())
    .into()
}
