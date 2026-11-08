//! Transfer Queue view for the iced shell, ported from `crate::views::transfer_queue`
//! (`TransferQueueView.swift`). Reuses that module's pure types/logic unchanged —
//! `PRESET_CHOICES`, `preset_label`, `status_label`, `TransferQueueCommand`,
//! `TransferQueueSheet`, [`TransferQueueViewState`] — and only rewrites `render` as this
//! module's `view`/`update`. Like the egui version, `ToggleExpanded` is a command (the
//! real `is_expanded` flag lives on `TransferItem` domain data, not view state), so this
//! view doesn't mutate it locally either.
//!
//! Documented simplifications: the per-row right-click context menu (Preview/Edit/
//! Remove) is dropped — Edit/Remove are already plain buttons on the row, same as the
//! egui version, so only "Preview" is lost pending an iced context-menu equivalent — and
//! Clear Queue/the duplicate-import guard/transfer-progress sheets aren't rendered yet
//! (Milestone 6); their trigger buttons set `state.sheet` exactly like the egui version
//! without anything rendering it.

use iced::widget::{button, checkbox, column, container, row, scrollable, text};
use iced::{Element, Length};

use walkman_domain::models::{TransferItem, TransferTargetMode};
use walkman_services::transcode::Preset;

use crate::iced_ui::theme::{color, font, sizing};
use crate::iced_ui::widgets;
use crate::views::transfer_queue::{preset_label, status_label, TransferQueueCommand, TransferQueueSheet, TransferQueueViewState, PRESET_CHOICES};

const PRESETS_PER_ROW: usize = 4;

#[derive(Debug, Clone)]
pub enum Message {
    TargetModeSelected(TransferTargetMode),
    PresetSelected(Preset),
    LoudnormToggled(bool),
    StartTransferClicked,
    ClearQueueClicked,
    ToggleExpandedClicked(String),
    RemoveClicked(String),
    EditClicked(String),
}

pub fn update(items: &[TransferItem], state: &mut TransferQueueViewState, message: Message) {
    match message {
        Message::TargetModeSelected(mode) => state.target_mode = mode,
        Message::PresetSelected(preset) => state.preset = preset,
        Message::LoudnormToggled(value) => state.loudnorm = value,
        Message::StartTransferClicked => {
            state.commands.push(TransferQueueCommand::StartTransfer {
                item_ids: items.iter().map(|i| i.id.clone()).collect(),
                target_mode: state.target_mode,
                preset: state.preset,
                loudnorm: state.loudnorm,
            });
        }
        Message::ClearQueueClicked => state.sheet = TransferQueueSheet::ClearConfirm,
        Message::ToggleExpandedClicked(id) => state.commands.push(TransferQueueCommand::ToggleExpanded(id)),
        Message::RemoveClicked(id) => state.commands.push(TransferQueueCommand::Remove(id)),
        Message::EditClicked(id) => state.commands.push(TransferQueueCommand::OpenEditor(id)),
    }
}

pub fn view<'a>(items: &'a [TransferItem], state: &'a TransferQueueViewState) -> Element<'a, Message> {
    let body: Element<'a, Message> = if items.is_empty() {
        container(text("No items queued").size(font::CALLOUT.size).color(color::SILVER_MUTED))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else {
        list(items, state)
    };

    column![toolbar(items, state), body].spacing(12).padding(16).into()
}

fn toolbar<'a>(items: &[TransferItem], state: &'a TransferQueueViewState) -> Element<'a, Message> {
    let mode_row = TransferTargetMode::ALL.iter().fold(row![].spacing(6), |r, &mode| {
        let active = state.target_mode == mode;
        r.push(
            button(text(mode.title()).size(font::CALLOUT.size))
                .padding([6, 12])
                .on_press(Message::TargetModeSelected(mode))
                .style(move |_theme, status| chip_style(active, status)),
        )
    });

    let preset_rows = PRESET_CHOICES.chunks(PRESETS_PER_ROW).fold(column![].spacing(4), |col, chunk| {
        let r = chunk.iter().fold(row![].spacing(4), |r, &preset| {
            let active = state.preset == preset;
            r.push(
                button(text(preset_label(preset)).size(font::CAPTION.size))
                    .padding([4, 8])
                    .on_press(Message::PresetSelected(preset))
                    .style(move |_theme, status| chip_style(active, status)),
            )
        });
        col.push(r)
    });

    let loudnorm = checkbox("Loudness Normalize", state.loudnorm).on_toggle(Message::LoudnormToggled).size(16).text_size(font::CALLOUT.size);

    let mut start = button(text("Start Transfer").size(font::CALLOUT.size)).padding([6, 14]);
    if !items.is_empty() {
        start = start.on_press(Message::StartTransferClicked);
    }
    let mut clear = button(text("Clear Queue").size(font::CALLOUT.size)).padding([6, 14]);
    if !items.is_empty() {
        clear = clear.on_press(Message::ClearQueueClicked);
    }

    column![
        mode_row,
        preset_rows,
        row![loudnorm, iced::widget::Space::new(Length::Fill, Length::Shrink), start.style(|_theme, s| prominent_style(s)), clear.style(|_theme, s| destructive_style(s))]
            .spacing(8)
            .align_y(iced::alignment::Vertical::Center),
    ]
    .spacing(8)
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
            radius: sizing::radius::CAPSULE.into(),
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
            radius: sizing::radius::BUTTON.into(),
        },
        ..button::Style::default()
    }
}

fn destructive_style(status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(iced::Background::Color(color::with_alpha(color::DESTRUCTIVE, if hovered { 0.95 } else { 0.85 }))),
        text_color: color::SILVER_LIGHT,
        border: iced::Border {
            color: color::with_alpha(color::DESTRUCTIVE_LIGHT, 0.35),
            width: 1.0,
            radius: sizing::radius::BUTTON.into(),
        },
        ..button::Style::default()
    }
}

fn list<'a>(items: &'a [TransferItem], state: &'a TransferQueueViewState) -> Element<'a, Message> {
    let col = items.iter().fold(column![].spacing(4), |col, item| {
        let selected = state.selected.contains(&item.id);
        col.push(item_row(item, selected))
    });

    scrollable(col.padding(iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: sizing::TABLE_BOTTOM_INSET,
        left: 0.0,
    }))
    .height(Length::Fill)
    .into()
}

fn item_row<'a>(item: &'a TransferItem, selected: bool) -> Element<'a, Message> {
    let chevron = if item.is_expanded { "\u{25BE}" } else { "\u{25B8}" };

    let mut header = row![
        button(text(chevron).size(font::CALLOUT.size))
            .padding([4, 8])
            .on_press(Message::ToggleExpandedClicked(item.id.clone()))
            .style(|_theme, status| chip_style(false, status)),
        text(status_label(&item.status)).size(font::CAPTION.size).color(color::SILVER_MUTED),
        column![
            text(item.title.as_str()).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
            text(item.artist.as_str()).size(font::CAPTION.size).color(color::SILVER_MUTED),
        ]
        .spacing(2),
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center);

    if item.is_duplicate_on_walkman {
        header = header.push(text("Duplicate").size(font::CAPTION.size).color(color::HIRES_GOLD));
    }

    header = header
        .push(iced::widget::Space::new(Length::Fill, Length::Shrink))
        .push(
            button(text("Remove").size(font::CAPTION.size))
                .padding([4, 8])
                .on_press(Message::RemoveClicked(item.id.clone()))
                .style(|_theme, status| chip_style(false, status)),
        )
        .push(
            button(text("Edit\u{2026}").size(font::CAPTION.size))
                .padding([4, 8])
                .on_press(Message::EditClicked(item.id.clone()))
                .style(|_theme, status| chip_style(false, status)),
        );

    let mut content = column![header].spacing(6);
    if item.is_expanded {
        let detail = format!(
            "{} \u{2022} {} kbps \u{2022} {:.1} kHz \u{2022} {}-bit",
            item.format,
            item.bitrate_kbps,
            item.sample_rate as f64 / 1000.0,
            item.bits_per_sample
        );
        content = content.push(text(detail).size(font::CAPTION.size).color(color::SILVER_MUTED));
        if let Some(info) = &item.duplicate_match_info {
            content = content.push(text(info.as_str()).size(font::CAPTION.size).color(color::SILVER_MUTED));
        }
    }

    container(content)
        .width(Length::Fill)
        .padding(10)
        .style(move |_theme| widgets::glass::liquid_glass(selected))
        .into()
}
