//! About sheet for the iced shell, ported from `crate::sheets::about`
//! (`AboutWalkmanStudioView.swift`). Reuses that module's pure types unchanged —
//! `AboutTab`, [`AboutSheetState`] — plus the real `device_detector::CATALOG` and
//! `ENGINE_RULES`/developer-support copy, and only rewrites `render` as this module's
//! `view`/`update`.

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length};

use walkman_domain::models::WalkmanDevice;
use walkman_services::device_detector::CATALOG;

use crate::iced_ui::theme::{color, font};
use crate::iced_ui::widgets::glass;
use crate::sheets::about::{AboutSheetState, AboutTab};

#[derive(Debug, Clone, Copy)]
pub enum Message {
    TabSelected(AboutTab),
    CloseClicked,
}

pub fn update(state: &mut AboutSheetState, message: Message) {
    match message {
        Message::TabSelected(tab) => state.tab = tab,
        Message::CloseClicked => state.close(),
    }
}

/// Builds the modal `Element` if the sheet is open, `None` otherwise — the caller
/// (`shell.rs`) only pushes this onto the stack when it's `Some`.
pub fn view<'a>(devices: &'a [WalkmanDevice], state: &'a AboutSheetState) -> Option<Element<'a, Message>> {
    if !state.open {
        return None;
    }

    let header = row![
        text("About Walkman Studio").size(font::TITLE2.size).color(color::SILVER_LIGHT),
        iced::widget::Space::new(Length::Fill, Length::Shrink),
        button(text("Close").size(font::CALLOUT.size)).padding([6, 12]).on_press(Message::CloseClicked).style(|_theme, status| chip_style(status)),
    ]
    .align_y(iced::alignment::Vertical::Center);

    let tab_row = AboutTab::ALL.iter().fold(row![].spacing(6), |r, &tab| {
        let active = state.tab == tab;
        r.push(
            button(text(tab.title()).size(font::CALLOUT.size))
                .padding([6, 12])
                .on_press(Message::TabSelected(tab))
                .style(move |_theme, status| chip_style_active(active, status)),
        )
    });

    let body = match state.tab {
        AboutTab::Hardware => hardware_tab(devices),
        AboutTab::SupportedDaps => supported_daps_tab(),
        AboutTab::EngineRules => engine_rules_tab(),
        AboutTab::DeveloperSupport => developer_support_tab(),
    };

    let content = column![header, tab_row, scrollable(body).height(Length::Fill)].spacing(10);
    Some(super::modal(content, 560.0, 520.0))
}

fn chip_style(status: button::Status) -> button::Style {
    chip_style_active(false, status)
}

fn chip_style_active(active: bool, status: button::Status) -> button::Style {
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

fn hardware_tab<'a>(devices: &'a [WalkmanDevice]) -> Element<'a, Message> {
    if devices.is_empty() {
        return text("No Walkman connected").size(font::CALLOUT.size).color(color::SILVER_MUTED).into();
    }

    devices
        .iter()
        .fold(column![].spacing(10), |col, device| {
            let info = &device.device_info;
            let rows = [
                ("Series", info.series_name.as_str()),
                ("Model", info.specific_model.as_str()),
                ("Vendor", info.vendor.as_str()),
                ("Firmware", info.firmware_version.as_str()),
                ("Display", info.display_resolution.as_str()),
                ("OS", if info.is_non_android { "Sony OS" } else { "Android" }),
            ]
            .into_iter()
            .fold(column![].spacing(2), |rows_col, (label, value)| {
                rows_col.push(row![text(label).size(font::CAPTION.size).color(color::SILVER_MUTED), text(value).size(font::CAPTION_BOLD.size).color(color::SILVER_LIGHT)].spacing(6))
            });

            col.push(
                column![
                    text(info.display_name.as_str()).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
                    rows,
                    text(format!("{:.0} GB internal", info.internal_capacity_gb)).size(font::CAPTION.size).color(color::SILVER_MUTED),
                ]
                .spacing(4),
            )
        })
        .into()
}

fn supported_daps_tab<'a>() -> Element<'a, Message> {
    CATALOG
        .iter()
        .fold(column![].spacing(2), |col, entry| {
            col.push(
                row![
                    text(entry.series_name).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
                    text(if entry.is_non_android { "Sony OS" } else { "Android" }).size(font::CAPTION.size).color(color::SILVER_MUTED),
                    text(entry.display_resolution).size(font::CAPTION.size).color(color::SILVER_MUTED),
                ]
                .spacing(8),
            )
        })
        .into()
}

/// The 4 compliance "rule cards", carried over verbatim from `crate::sheets::about`.
const ENGINE_RULES: [(&str, &str); 4] = [
    (
        "Baseline RGB JPEG Artwork",
        "Strips progressive passes, CMYK color spaces, and limits resolution to 800\u{d7}800 to prevent display engine freezes.",
    ),
    (
        "Strict ID3v2.3 (UTF-16)",
        "Enforces UTF-16 with BOM (encoding=1) and binary ID3v2.3 tags, avoiding ID3v2.4 database corruption.",
    ),
    (
        "CRLF Relative .M3U8 Playlists",
        "Writes Windows-compliant CRLF (\\r\\n) line endings and relative POSIX paths directly into /MUSIC/.",
    ),
    (
        "macOS Shadow File Purge",
        "Merges AppleDouble forks with dot_clean -m and removes ._* and .DS_Store ghost files before safe eject.",
    ),
];

fn engine_rules_tab<'a>() -> Element<'a, Message> {
    ENGINE_RULES
        .into_iter()
        .fold(column![].spacing(8), |col, (title, detail)| {
            col.push(
                container(
                    column![
                        text(title).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
                        text(detail).size(font::CAPTION.size).color(color::SILVER_MUTED),
                    ]
                    .spacing(2),
                )
                .padding(10)
                .width(Length::Fill)
                .style(|_theme| glass::liquid_glass(false)),
            )
        })
        .into()
}

fn developer_support_tab<'a>() -> Element<'a, Message> {
    column![
        row![
            text("John Lloyd Malabanan").size(font::TITLE2.size).color(color::SILVER_LIGHT),
            text("Creator").size(font::BADGE.size).color(color::HIRES_GOLD),
        ]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center),
        text("GitHub: @Jhay-EL \u{2022} UX/UI Designer & Software Developer").size(font::SUBHEADLINE_MEDIUM.size).color(color::SILVER_MUTED),
        text(
            "Walkman Studio was designed and engineered from the ground up to solve the challenges of using \
             modern macOS with Sony Walkman digital audio players. It automates strict ID3v2.3 & FLAC tagging, \
             Baseline RGB JPEG cover art conversion, Sony 12 Tone / SenseMe acoustic analysis, synchronized .LRC \
             lyrics, batch metadata editing, dual-partition storage routing, and AppleDouble ghost file cleanup.",
        )
        .size(font::CALLOUT.size)
        .color(color::SILVER_MUTED),
    ]
    .spacing(8)
    .into()
}
