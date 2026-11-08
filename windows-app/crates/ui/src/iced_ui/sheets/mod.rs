//! Modal editor sheets for the iced shell (Milestone 6 of the egui-to-iced migration
//! plan), one module per `crates/ui/src/sheets/*` counterpart.

use iced::widget::{container, text, text_input};
use iced::{Element, Length};

use crate::iced_ui::theme::{color, font};
use crate::iced_ui::widgets::glass;

pub mod about;

/// A centered modal: a dimmed full-screen backdrop plus a [`glass::modal_glass_chassis`]
/// panel sized `width`×`height`. Callers overlay this on top of the base content via
/// `iced::widget::stack![base, modal(...)]`, the same way `shell.rs` already overlays the
/// floating now-playing bar.
pub fn modal<'a, Message: 'a>(content: impl Into<Element<'a, Message>>, width: f32, height: f32) -> Element<'a, Message> {
    container(
        container(content)
            .width(Length::Fixed(width))
            .height(Length::Fixed(height))
            .padding(24)
            .style(|_theme| glass::modal_glass_chassis()),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .style(|_theme| glass::modal_backdrop())
    .into()
}

/// A labeled single-line text field, shared by every sheet — ports
/// `crate::sheets::text_field`.
pub fn text_field<'a, Message: 'a + Clone>(label: &'static str, value: &'a str, on_change: impl Fn(String) -> Message + 'a) -> Element<'a, Message> {
    iced::widget::column![
        text(label).size(font::CAPTION.size).color(color::SILVER_MUTED),
        text_input("", value).on_input(on_change).padding(8).size(font::BODY.size).width(Length::Fill),
    ]
    .spacing(4)
    .into()
}
