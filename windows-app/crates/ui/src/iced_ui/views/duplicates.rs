//! Duplicates view for the iced shell, ported from `crate::views::duplicates`
//! (`DuplicatesView.swift`). Reuses that module's pure types/logic unchanged —
//! `DuplicatesCommand`, `DuplicatesSheet`, [`DuplicatesViewState`], `group_key` — plus
//! the real `walkman_services::duplicate_finder` clustering, and only rewrites `render`
//! as this module's `view`/`update`.
//!
//! Documented simplification (same as the other ported views deferring sheets to
//! Milestone 6): clicking Delete/"Keep Best Quality"/"Auto-Resolve Quality" sets
//! `state.sheet` to the matching confirm variant exactly like the egui version does, but
//! the confirm sheet itself isn't rendered yet, so nothing destructive can actually fire
//! from this view until Milestone 6 renders those sheets' "confirm" buttons.

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length};

use walkman_domain::models::Track;
use walkman_services::duplicate_finder::{self, DuplicateGroup, DuplicateScope, DuplicateSortOption};

use crate::inspector::format_size;
use crate::iced_ui::theme::{color, font, sizing};
use crate::iced_ui::widgets;
use crate::views::duplicates::{group_key, DuplicatesCommand, DuplicatesSheet, DuplicatesViewState};

#[derive(Debug, Clone)]
pub enum Message {
    ScopeSelected(DuplicateScope),
    SortSelected(DuplicateSortOption),
    GroupToggled(String),
    KeepBestClicked { group_key: String, track_ids: Vec<String> },
    AutoResolveClicked,
    DeleteClicked(String),
    RevealClicked(String),
    AuditionClicked(String),
}

pub fn update(state: &mut DuplicatesViewState, message: Message) {
    match message {
        Message::ScopeSelected(scope) => state.scope = scope,
        Message::SortSelected(sort) => state.sort = sort,
        Message::GroupToggled(key) => {
            if state.expanded.contains(&key) {
                state.expanded.retain(|k| k != &key);
            } else {
                state.expanded.push(key);
            }
        }
        Message::KeepBestClicked { group_key, track_ids } => {
            state.sheet = DuplicatesSheet::ResolveGroupConfirm { group_key, track_ids };
        }
        Message::AutoResolveClicked => state.sheet = DuplicatesSheet::AutoResolveConfirm,
        Message::DeleteClicked(track_id) => state.sheet = DuplicatesSheet::DeleteConfirm { track_id },
        Message::RevealClicked(path) => state.commands.push(DuplicatesCommand::RevealInExplorer(path)),
        Message::AuditionClicked(path) => state.commands.push(DuplicatesCommand::Audition(path)),
    }
}

pub fn view<'a>(tracks: &[Track], state: &'a DuplicatesViewState) -> Element<'a, Message> {
    let mut groups = duplicate_finder::find_duplicates(tracks, state.scope);
    duplicate_finder::sort_groups(&mut groups, state.sort);

    let body: Element<'a, Message> = if groups.is_empty() {
        container(text("No duplicates found").size(font::CALLOUT.size).color(color::SILVER_MUTED))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else {
        groups_list(groups, state)
    };

    column![toolbar(&duplicate_finder::find_duplicates(tracks, state.scope), state), body]
        .spacing(12)
        .padding(16)
        .into()
}

fn toolbar<'a>(groups: &[DuplicateGroup], state: &'a DuplicatesViewState) -> Element<'a, Message> {
    let scope_row = row![
        scope_chip(DuplicateScope::ExactSameAlbum, "Exact Same Album (Safe)", state.scope),
        scope_chip(DuplicateScope::CrossAlbum, "Include Cross-Album (Loose)", state.scope),
    ]
    .spacing(6);

    let sort_row = [
        DuplicateSortOption::SpaceRecoverable,
        DuplicateSortOption::TitleAZ,
        DuplicateSortOption::ArtistAZ,
        DuplicateSortOption::MostCopies,
    ]
    .into_iter()
    .fold(row![].spacing(4), |r, option| {
        let active = state.sort == option;
        r.push(
            button(text(sort_label(option)).size(font::CAPTION.size))
                .padding([4, 8])
                .on_press(Message::SortSelected(option))
                .style(move |_theme, status| chip_style(active, status)),
        )
    });

    let total = duplicate_finder::total_recoverable_bytes(groups);
    let total_label = text(format!("{} recoverable", format_size(total))).size(font::CAPTION.size).color(color::SILVER_MUTED);

    let mut auto_resolve = button(text("Auto-Resolve Quality").size(font::CALLOUT.size)).padding([6, 12]);
    if !groups.is_empty() {
        auto_resolve = auto_resolve.on_press(Message::AutoResolveClicked);
    }

    column![
        scope_row,
        sort_row,
        row![
            total_label,
            iced::widget::Space::new(Length::Fill, Length::Shrink),
            auto_resolve.style(|_theme, status| gold_style(status)),
        ]
        .align_y(iced::alignment::Vertical::Center),
    ]
    .spacing(8)
    .into()
}

fn scope_chip<'a>(scope: DuplicateScope, label: &'static str, selected: DuplicateScope) -> Element<'a, Message> {
    let active = scope == selected;
    button(text(label).size(font::CALLOUT.size))
        .padding([6, 12])
        .on_press(Message::ScopeSelected(scope))
        .style(move |_theme, status| chip_style(active, status))
        .into()
}

fn sort_label(option: DuplicateSortOption) -> &'static str {
    match option {
        DuplicateSortOption::SpaceRecoverable => "Space Recoverable",
        DuplicateSortOption::TitleAZ => "Song Title A\u{2013}Z",
        DuplicateSortOption::ArtistAZ => "Artist A\u{2013}Z",
        DuplicateSortOption::MostCopies => "Most Copies",
    }
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

fn groups_list<'a>(groups: Vec<DuplicateGroup>, state: &'a DuplicatesViewState) -> Element<'a, Message> {
    let list = groups.into_iter().fold(column![].spacing(6), |col, group| col.push(group_card(group, state)));

    scrollable(list.padding(iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: sizing::TABLE_BOTTOM_INSET,
        left: 0.0,
    }))
    .height(Length::Fill)
    .into()
}

fn group_card<'a>(group: DuplicateGroup, state: &'a DuplicatesViewState) -> Element<'a, Message> {
    let key = group_key(&group);
    let expanded = state.expanded.contains(&key);
    let best_path = group.best_quality_track().full_path.clone();
    let recoverable = format_size(group.recoverable_bytes());
    let copies = group.tracks.len();
    let has_quality_diff = group.has_quality_diff;
    let title = group.title().to_string();
    let artist = group.artist().to_string();

    let chevron_key = key.clone();
    let header = row![
        button(text(if expanded { "\u{25BE}" } else { "\u{25B8}" }).size(font::CALLOUT.size))
            .padding([4, 8])
            .on_press(Message::GroupToggled(chevron_key))
            .style(|_theme, status| chip_style(false, status)),
        column![
            text(title).size(font::BODY_MEDIUM.size).color(color::SILVER_LIGHT),
            text(artist).size(font::CAPTION.size).color(color::SILVER_MUTED),
        ]
        .spacing(2),
        text(format!("{copies} copies")).size(font::CAPTION.size).color(color::SILVER_MUTED),
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center);

    let header = if has_quality_diff {
        header.push(text("Quality Diff").size(font::CAPTION.size).color(color::HIRES_GOLD))
    } else {
        header
    };

    let keep_best_ids: Vec<String> = group.tracks.iter().map(|t| t.full_path.clone()).filter(|p| p != &best_path).collect();
    let header = header
        .push(iced::widget::Space::new(Length::Fill, Length::Shrink))
        .push(text(recoverable).size(font::CAPTION.size).color(color::SILVER_MUTED))
        .push(
            button(text("Keep Best Quality").size(font::CALLOUT.size))
                .padding([4, 10])
                .on_press(Message::KeepBestClicked { group_key: key, track_ids: keep_best_ids })
                .style(|_theme, status| chip_style(false, status)),
        );

    let mut content = column![header].spacing(6);
    if expanded {
        let tracks_col = group.tracks.into_iter().fold(column![].spacing(4), |col, track| {
            let path = track.full_path.clone();
            col.push(track_row(track, &best_path, path))
        });
        content = content.push(tracks_col);
    }

    container(content).width(Length::Fill).padding(10).style(|_theme| widgets::glass::liquid_glass(false)).into()
}

fn track_row<'a>(track: Track, best_path: &str, path: String) -> Element<'a, Message> {
    let star: Element<'a, Message> = if track.full_path == best_path {
        text("\u{2605}").size(font::CAPTION.size).color(color::HIRES_GOLD).into()
    } else {
        iced::widget::Space::new(Length::Fixed(14.0), Length::Shrink).into()
    };

    let detail = format!(
        "{} \u{2022} {} kbps \u{2022} {:.1} kHz \u{2022} {}-bit \u{2022} {}",
        track.format,
        track.bitrate_kbps,
        track.sample_rate as f64 / 1000.0,
        track.bits_per_sample,
        format_size(track.size_bytes)
    );

    row![
        star,
        text(detail).size(font::CAPTION.size).color(color::SILVER_MUTED),
        iced::widget::Space::new(Length::Fill, Length::Shrink),
        button(text("Delete").size(font::CAPTION.size))
            .padding([2, 8])
            .on_press(Message::DeleteClicked(path.clone()))
            .style(|_theme, status| destructive_style(status)),
        button(text("Reveal").size(font::CAPTION.size))
            .padding([2, 8])
            .on_press(Message::RevealClicked(path.clone()))
            .style(|_theme, status| chip_style(false, status)),
        button(text("Audition").size(font::CAPTION.size))
            .padding([2, 8])
            .on_press(Message::AuditionClicked(path))
            .style(|_theme, status| chip_style(false, status)),
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center)
    .into()
}

fn destructive_style(status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(iced::Background::Color(color::with_alpha(color::DESTRUCTIVE, if hovered { 0.95 } else { 0.85 }))),
        text_color: color::SILVER_LIGHT,
        border: iced::Border {
            color: color::with_alpha(color::DESTRUCTIVE_LIGHT, 0.35),
            width: 1.0,
            radius: sizing::radius::CAPSULE.into(),
        },
        ..button::Style::default()
    }
}
