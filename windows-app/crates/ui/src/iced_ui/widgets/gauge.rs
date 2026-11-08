//! Storage gauge, ported from `crate::theme::widgets::paint_storage_gauge`. The egui
//! version hand-paints a gradient-filled rect; here the fill proportion is built from
//! two styled containers in a row (filled : empty, via `Length::FillPortion`) with a
//! flat color sampled from `theme::gradient::STORAGE` at its midpoint, since iced
//! containers don't take a gradient background directly without a `canvas::Program`.
//! Revisit with a real `canvas`-painted gradient if the flat approximation reads wrong
//! next to the macOS reference once this is wired into the device-overview view.

use crate::iced_ui::theme::{color, gradient};
use iced::widget::{container, Space};
use iced::{Background, Border, Element, Length};

/// Builds a horizontal storage gauge filled to `fraction` (clamped to `0.0..=1.0`).
pub fn storage_gauge<'a, Message: 'a>(fraction: f32) -> Element<'a, Message> {
    let fraction = fraction.clamp(0.0, 1.0);
    // `FillPortion` is a u16 weight, not a fraction; scale to 1000ths for resolution
    // and keep both portions >= 1 so an empty/full gauge still renders its track.
    let filled = ((fraction * 1000.0).round() as u16).max(1);
    let empty = (1000u16.saturating_sub(filled)).max(1);
    let fill_color = gradient::STORAGE.sample(0.5);

    let filled_bar = container(Space::new(Length::Fill, Length::Fill))
        .width(Length::FillPortion(filled))
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(Background::Color(fill_color)),
            border: Border {
                radius: 4.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });

    let empty_bar = container(Space::new(Length::Fill, Length::Fill))
        .width(Length::FillPortion(empty))
        .height(Length::Fill);

    container(
        iced::widget::Row::new()
            .push(filled_bar)
            .push(empty_bar)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fixed(6.0))
    .style(|_theme| container::Style {
        background: Some(Background::Color(color::INPUT)),
        border: Border {
            radius: 4.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .into()
}
