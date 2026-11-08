//! Device Overview view for the iced shell, ported from `crate::views::device_overview`
//! (`DeviceOverviewView.swift`). Reuses that module's pure types unchanged —
//! `DeviceQuickStats`, `DeviceOverviewCommand`, [`DeviceOverviewViewState`] — and only
//! rewrites `render` as this module's `view`/`update`. No real device model image is
//! wired up on the egui side either, so both show the same placeholder glyph tile. The
//! storage gauge reuses `iced_ui::widgets::gauge::storage_gauge` from Milestone 2 rather
//! than re-painting one.
//!
//! Documented simplification: the Android MTP help sheet (`render_mtp_help_sheet`) isn't
//! rendered yet — opening it just flips `state.show_mtp_help` same as before; actually
//! showing that modal is Milestone 6's job alongside the other sheets.

use iced::widget::{button, column, container, row, text};
use iced::{Element, Length};

use walkman_domain::models::WalkmanDevice;

use crate::iced_ui::theme::{color, font, sizing};
use crate::iced_ui::widgets;
use crate::views::device_overview::{DeviceOverviewCommand, DeviceOverviewViewState, DeviceQuickStats};

#[derive(Debug, Clone)]
pub enum Message {
    DeviceSelected(usize),
    RefreshClicked,
    EjectClicked(String),
    LearnMoreClicked,
}

pub fn update(state: &mut DeviceOverviewViewState, message: Message) {
    match message {
        Message::DeviceSelected(index) => state.selected_device_index = index,
        Message::RefreshClicked => state.commands.push(DeviceOverviewCommand::Refresh),
        Message::EjectClicked(mount) => state.commands.push(DeviceOverviewCommand::Eject(mount)),
        Message::LearnMoreClicked => state.show_mtp_help = true,
    }
}

pub fn view<'a>(devices: &'a [WalkmanDevice], stats: DeviceQuickStats, state: &'a DeviceOverviewViewState) -> Element<'a, Message> {
    if devices.is_empty() {
        return disconnected();
    }
    let index = state.selected_device_index.min(devices.len() - 1);
    let device = &devices[index];

    let mut content = column![].spacing(12);
    if devices.len() > 1 {
        content = content.push(device_switcher(devices, index));
    }
    content = content.push(header(device));
    content = content.push(quick_stats(stats));
    content = content.push(drive_gauges(device));
    content = content.push(toolbar(device));

    if device.device_info.is_android() {
        content = content.push(mtp_banner());
    }

    container(content).width(Length::Fill).height(Length::Fill).padding(16).into()
}

fn disconnected<'a>() -> Element<'a, Message> {
    container(
        column![
            text("No Walkman Connected").size(font::TITLE2.size).color(color::SILVER_LIGHT),
            text("Connect a Sony Walkman via USB to see its overview here.")
                .size(font::CALLOUT.size)
                .color(color::SILVER_MUTED),
        ]
        .spacing(4)
        .align_x(iced::alignment::Horizontal::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

fn device_switcher(devices: &[WalkmanDevice], selected: usize) -> Element<'_, Message> {
    devices
        .iter()
        .enumerate()
        .fold(row![].spacing(6), |r, (index, device)| {
            let active = index == selected;
            r.push(
                button(text(device.device_info.specific_model.as_str()).size(font::CALLOUT.size))
                    .padding([6, 12])
                    .on_press(Message::DeviceSelected(index))
                    .style(move |_theme, status| chip_style(active, status)),
            )
        })
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

fn header(device: &WalkmanDevice) -> Element<'_, Message> {
    let glyph = container(text("\u{1F3B5}").size(font::TITLE1.size).color(color::SILVER_MUTED))
        .width(Length::Fixed(72.0))
        .height(Length::Fixed(72.0))
        .center_x(Length::Fixed(72.0))
        .center_y(Length::Fixed(72.0))
        .style(|_theme| container::Style {
            background: Some(iced::Background::Color(color::INPUT)),
            border: iced::Border {
                radius: 10.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });

    let info = column![
        text(device.device_info.display_name.clone()).size(font::TITLE2.size).color(color::SILVER_LIGHT),
        text(format!(
            "{} \u{2022} {} \u{2022} FW {}",
            device.device_info.series_name,
            device.device_info.os_family(),
            device.device_info.firmware_version
        ))
        .size(font::CALLOUT.size)
        .color(color::SILVER_MUTED),
        text(format!(
            "{} \u{2022} {:.0} GB internal",
            device.device_info.display_resolution, device.device_info.internal_capacity_gb
        ))
        .size(font::CAPTION.size)
        .color(color::SILVER_MUTED),
    ]
    .spacing(2);

    row![glyph, info].spacing(12).align_y(iced::alignment::Vertical::Center).into()
}

fn quick_stats<'a>(stats: DeviceQuickStats) -> Element<'a, Message> {
    let tile = |label: &'static str, value: usize| {
        container(
            column![
                text(value.to_string()).size(font::TITLE3.size).color(color::SILVER_LIGHT),
                text(label).size(font::CAPTION.size).color(color::SILVER_MUTED),
            ]
            .spacing(2),
        )
        .padding(12)
        .style(|_theme| widgets::glass::liquid_glass(false))
    };

    row![
        tile("Albums", stats.album_count),
        tile("Artists", stats.artist_count),
        tile("Hi-Res Tracks", stats.hi_res_count),
    ]
    .spacing(8)
    .into()
}

fn drive_gauges(device: &WalkmanDevice) -> Element<'_, Message> {
    device.drives.iter().fold(column![].spacing(6), |col, drive| {
        col.push(
            container(
                column![
                    row![
                        text(drive.name.clone()).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
                        iced::widget::Space::new(Length::Fill, Length::Shrink),
                        text(format!(
                            "{} / {}",
                            walkman_services::walkman_engine::format_bytes(drive.used_bytes),
                            walkman_services::walkman_engine::format_bytes(drive.total_bytes)
                        ))
                        .size(font::CAPTION.size)
                        .color(color::SILVER_MUTED),
                    ],
                    widgets::gauge::storage_gauge(storage_fraction(drive.used_bytes, drive.total_bytes)),
                ]
                .spacing(6),
            )
            .padding(12)
            .width(Length::Fill)
            .style(|_theme| widgets::glass::liquid_glass(false)),
        )
    }).into()
}

fn storage_fraction(used: i64, total: i64) -> f32 {
    if total <= 0 {
        0.0
    } else {
        (used.max(0) as f32) / (total as f32)
    }
}

fn toolbar(device: &WalkmanDevice) -> Element<'_, Message> {
    let mut r = row![button(text("Refresh").size(font::CALLOUT.size))
        .padding([6, 12])
        .on_press(Message::RefreshClicked)
        .style(|_theme, status| chip_style(false, status))]
    .spacing(8);

    if let Some(drive) = device.internal_drive().or_else(|| device.drives.first()) {
        let mount = drive.mount_point.clone();
        r = r.push(
            button(text("Eject").size(font::CALLOUT.size))
                .padding([6, 12])
                .on_press(Message::EjectClicked(mount))
                .style(|_theme, status| destructive_style(status)),
        );
    }

    r.into()
}

fn destructive_style(status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(iced::Background::Color(color::with_alpha(
            color::DESTRUCTIVE,
            if hovered { 0.95 } else { 0.85 },
        ))),
        text_color: color::SILVER_LIGHT,
        border: iced::Border {
            color: color::with_alpha(color::DESTRUCTIVE_LIGHT, 0.35),
            width: 1.0,
            radius: sizing::radius::CAPSULE.into(),
        },
        ..button::Style::default()
    }
}

fn mtp_banner<'a>() -> Element<'a, Message> {
    container(
        row![
            text("This Android Walkman exposes storage via MTP, not a drive letter.")
                .size(font::CALLOUT.size)
                .color(color::SILVER_MUTED),
            iced::widget::Space::new(Length::Fill, Length::Shrink),
            button(text("Learn More\u{2026}").size(font::CALLOUT.size))
                .padding([6, 12])
                .on_press(Message::LearnMoreClicked)
                .style(|_theme, status| gold_style(status)),
        ]
        .align_y(iced::alignment::Vertical::Center),
    )
    .padding(12)
    .width(Length::Fill)
    .style(|_theme| widgets::glass::liquid_glass(false))
    .into()
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
            radius: sizing::radius::BUTTON.into(),
        },
        ..button::Style::default()
    }
}
