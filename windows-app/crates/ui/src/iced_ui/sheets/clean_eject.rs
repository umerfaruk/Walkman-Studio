//! Clean & Eject sheet for the iced shell, ported from `crate::sheets::clean_eject`
//! (`CleanEjectSheet.swift`). Reuses that module's pure types unchanged —
//! `CleanEjectCommand`, [`CleanEjectSheetState`]. The egui version's spinner during the
//! running state becomes plain "Working…" text — iced has no built-in spinner widget,
//! and adding an animated one is cosmetic rather than structural.

use iced::widget::{button, column, text};
use iced::Element;

use crate::iced_ui::theme::{color, font};
use crate::sheets::clean_eject::{CleanEjectCommand, CleanEjectSheetState};

#[derive(Debug, Clone)]
pub enum Message {
    CleanAndEjectClicked,
    CloseClicked,
}

pub fn update(state: &mut CleanEjectSheetState, message: Message) {
    match message {
        Message::CleanAndEjectClicked => {
            state.is_running = true;
            state.commands.push(CleanEjectCommand::StartCleanAndEject { mount_path: state.mount_path.clone() });
        }
        Message::CloseClicked => {
            state.commands.push(CleanEjectCommand::Close);
            state.close();
        }
    }
}

pub fn view(state: &CleanEjectSheetState) -> Option<Element<'_, Message>> {
    if !state.open {
        return None;
    }

    let body = match &state.result {
        None if state.is_running => column![
            text(format!("Cleaning and ejecting {}\u{2026}", state.drive_label)).size(font::CALLOUT.size).color(color::SILVER_MUTED),
            text("Working\u{2026}").size(font::CAPTION.size).color(color::SILVER_MUTED),
        ]
        .spacing(10),
        None => column![
            text(format!("Remove junk files and safely eject {}?", state.drive_label)).size(font::CALLOUT.size).color(color::SILVER_MUTED),
            iced::widget::row![
                button(text("Clean & Eject").size(font::CALLOUT.size)).padding([8, 16]).on_press(Message::CleanAndEjectClicked).style(|_theme, s| prominent_style(s)),
                button(text("Cancel").size(font::CALLOUT.size)).padding([8, 16]).on_press(Message::CloseClicked).style(|_theme, s| chip_style(s)),
            ]
            .spacing(8),
        ]
        .spacing(12),
        Some(Ok(result)) => column![
            text("Safe to unplug.").size(font::CALLOUT.size).color(color::SILVER_LIGHT),
            text(format!("Removed {} junk file(s), {} folder(s).", result.cleaned.files_removed, result.cleaned.folders_removed))
                .size(font::CAPTION.size)
                .color(color::SILVER_MUTED),
            button(text("Done").size(font::CALLOUT.size)).padding([8, 16]).on_press(Message::CloseClicked).style(|_theme, s| prominent_style(s)),
        ]
        .spacing(12),
        Some(Err(message)) => column![
            text("Eject Failed").size(font::CALLOUT.size).color(color::HIRES_GOLD),
            text(message.as_str()).size(font::CAPTION.size).color(color::SILVER_MUTED),
            button(text("Close").size(font::CALLOUT.size)).padding([8, 16]).on_press(Message::CloseClicked).style(|_theme, s| chip_style(s)),
        ]
        .spacing(12),
    };

    let content = column![text("Clean & Eject").size(font::TITLE2.size).color(color::SILVER_LIGHT), body].spacing(10);
    Some(super::modal(content, 420.0, 240.0))
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
