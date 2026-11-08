//! Lyrics editor sheet for the iced shell, ported from `crate::sheets::lyrics_editor`
//! (spec task 18, R15.6). Reuses that module's pure types/logic unchanged —
//! `LyricsEditorCommand`, [`LyricsEditorState`], `build_lines` — and only rewrites
//! `render` as this module's `view`/`update`.
//!
//! `iced::widget::text_editor` needs its `Content` (cursor/selection state) persisted
//! across frames rather than rebuilt each call — unlike `text_input`'s plain `&str`, it
//! can't be derived fresh from `LyricsEditorState::raw_text` every `view()` without
//! losing cursor position on every keystroke. Rather than giving that shared,
//! egui-also-used struct an iced-specific field, the caller (`shell.rs`) owns the
//! `Content` alongside its `LyricsEditorState` and passes it in here; `raw_text` is kept
//! in sync with it on every edit so the two never drift.
use iced::widget::{button, column, row, scrollable, text, text_editor};
use iced::{Element, Length};

use crate::iced_ui::theme::{color, font};
use crate::sheets::lyrics_editor::{build_lines, LyricsEditorCommand, LyricsEditorState};

#[derive(Debug, Clone)]
pub enum Message {
    TextEdited(text_editor::Action),
    TapClicked(usize),
    SaveClicked,
    CancelClicked,
}

pub fn update(state: &mut LyricsEditorState, content: &mut text_editor::Content, playhead_seconds: f64, message: Message) {
    match message {
        Message::TextEdited(action) => {
            content.perform(action);
            state.raw_text = content.text();
            state.synced = vec![None; non_empty_line_count(&state.raw_text)];
        }
        Message::TapClicked(index) => {
            if let Some(slot) = state.synced.get_mut(index) {
                *slot = Some(playhead_seconds);
            }
        }
        Message::SaveClicked => {
            let lines = build_lines(&state.raw_text, &state.synced, state.duration_seconds);
            state.commands.push(LyricsEditorCommand::Save { track_id: state.track_id.clone(), lines });
            state.close();
        }
        Message::CancelClicked => state.close(),
    }
}

fn non_empty_line_count(text: &str) -> usize {
    text.lines().filter(|l| !l.trim().is_empty()).count()
}

pub fn view<'a>(state: &'a LyricsEditorState, content: &'a text_editor::Content) -> Option<Element<'a, Message>> {
    if !state.open {
        return None;
    }

    let editor = text_editor(content).height(Length::Fixed(120.0)).on_action(Message::TextEdited);

    let lines: Vec<&str> = state.raw_text.lines().filter(|l| !l.trim().is_empty()).collect();
    let sync_list = lines.iter().enumerate().fold(column![].spacing(4), |col, (index, &line)| {
        let timestamp_label = state.synced.get(index).copied().flatten().map(format_timestamp).unwrap_or_else(|| "\u{2014}".to_owned());
        col.push(
            row![
                text(timestamp_label).size(font::MONO_SMALL.size).color(color::HIRES_GOLD),
                text(line).size(font::BODY.size).color(color::SILVER_LIGHT),
                iced::widget::Space::new(Length::Fill, Length::Shrink),
                button(text("Tap").size(font::CAPTION.size)).padding([2, 8]).on_press(Message::TapClicked(index)).style(|_theme, s| chip_style(s)),
            ]
            .spacing(8)
            .align_y(iced::alignment::Vertical::Center),
        )
    });

    let can_save = !state.raw_text.trim().is_empty();
    let mut save = button(text("Save").size(font::CALLOUT.size)).padding([8, 16]);
    if can_save {
        save = save.on_press(Message::SaveClicked);
    }

    let body = column![
        text("Paste or type lyrics below, one line per line, then tap each line while the track plays to sync it.")
            .size(font::CAPTION.size)
            .color(color::SILVER_MUTED),
        editor,
        text("Sync").size(font::CAPTION_BOLD.size).color(color::SILVER_MUTED),
        scrollable(sync_list).height(Length::Fixed(280.0)),
        row![
            save.style(|_theme, s| prominent_style(s)),
            button(text("Cancel").size(font::CALLOUT.size)).padding([8, 16]).on_press(Message::CancelClicked).style(|_theme, s| chip_style(s)),
        ]
        .spacing(8),
    ]
    .spacing(10);

    let full = column![text("Edit Lyrics").size(font::TITLE2.size).color(color::SILVER_LIGHT), body].spacing(8);
    Some(super::modal(full, 640.0, 600.0))
}

fn format_timestamp(seconds: f64) -> String {
    let total = seconds.max(0.0) as u64;
    format!("{}:{:02}", total / 60, total % 60)
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
