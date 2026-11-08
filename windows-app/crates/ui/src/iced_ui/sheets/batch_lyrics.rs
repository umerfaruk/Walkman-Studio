//! Batch Lyrics Generator sheet for the iced shell, ported from
//! `crate::sheets::batch_lyrics` (`BatchLyricsGeneratorSheet.swift`). Reuses that
//! module's pure types/logic unchanged — `LyricsGeneratorScope`, `MetricFilter`,
//! `resolve_track_ids`, `items_per_second`, `eta_seconds`, `filter_outcomes`,
//! `LyricsGeneratorCommand`, [`BatchLyricsGeneratorState`] — and only rewrites `render`
//! as this module's `view`/`update`.
//!
//! `all_in_view_ids`/`selected_ids`/`active_item_id` are the opening view's own context
//! (same as the egui version's `render` parameters) — no view currently wires a trigger
//! to open this sheet (same deferred-sheet state as the others), so `shell.rs` passes
//! empty/`None` for now; a future milestone threads the real per-view context through
//! when a "Find Lyrics…" button actually opens this sheet instead of just recording its
//! view-local command.

use iced::widget::{button, checkbox, column, progress_bar, row, scrollable, text, text_input};
use iced::{Element, Length};

use walkman_domain::models::Track;

use crate::iced_ui::theme::{color, font};
use crate::sheets::batch_lyrics::{
    eta_seconds, filter_outcomes, items_per_second, resolve_track_ids, BatchLyricsGeneratorState, LyricsGeneratorCommand, LyricsGeneratorScope,
    MetricFilter,
};

#[derive(Debug, Clone)]
pub enum Message {
    ScopeSelected(LyricsGeneratorScope),
    SearchChanged(String),
    SelectAllToggled(Vec<String>),
    TrackToggled(String, bool),
    FilterSelected(MetricFilter),
    GenerateClicked(Vec<String>),
    CancelRunClicked,
    CancelClicked,
    DoneClicked,
}

pub fn update(state: &mut BatchLyricsGeneratorState, message: Message) {
    match message {
        Message::ScopeSelected(scope) => state.scope = scope,
        Message::SearchChanged(q) => state.choose_specific_search = q,
        Message::SelectAllToggled(visible_ids) => {
            let all_selected = !visible_ids.is_empty() && visible_ids.iter().all(|id| state.choose_specific_selected.contains(id));
            if all_selected {
                state.choose_specific_selected.retain(|id| !visible_ids.contains(id));
            } else {
                for id in visible_ids {
                    if !state.choose_specific_selected.contains(&id) {
                        state.choose_specific_selected.push(id);
                    }
                }
            }
        }
        Message::TrackToggled(id, checked) => {
            if checked {
                state.choose_specific_selected.push(id);
            } else {
                state.choose_specific_selected.retain(|s| s != &id);
            }
        }
        Message::FilterSelected(filter) => state.filter = filter,
        Message::GenerateClicked(track_ids) => {
            state.is_running = true;
            state.completed = 0;
            state.total = track_ids.len();
            state.commands.push(LyricsGeneratorCommand::Start { track_ids });
        }
        Message::CancelRunClicked => {
            state.commands.push(LyricsGeneratorCommand::Cancel);
            state.is_running = false;
        }
        Message::CancelClicked | Message::DoneClicked => state.close(),
    }
}

pub fn view<'a>(
    all_tracks: &'a [Track],
    all_in_view_ids: &'a [String],
    selected_ids: &'a [String],
    active_item_id: Option<&'a str>,
    missing_lyrics_ids: &'a [String],
    state: &'a BatchLyricsGeneratorState,
) -> Option<Element<'a, Message>> {
    if !state.open {
        return None;
    }

    let body = if state.is_running || !state.outcomes.is_empty() {
        progress_and_results(state)
    } else {
        scope_picker(all_tracks, missing_lyrics_ids, state)
    };

    let footer = footer(all_in_view_ids, selected_ids, active_item_id, state);

    let content = column![text("Batch Lyrics Generator").size(font::TITLE2.size).color(color::SILVER_LIGHT), body, footer].spacing(14);
    Some(super::modal(content, 640.0, 620.0))
}

fn scope_picker<'a>(all_tracks: &'a [Track], missing_lyrics_ids: &'a [String], state: &'a BatchLyricsGeneratorState) -> Element<'a, Message> {
    let scope_row = LyricsGeneratorScope::ALL.iter().fold(row![].spacing(6), |r, &scope| {
        let active = state.scope == scope;
        r.push(
            button(text(scope.title()).size(font::CALLOUT.size))
                .padding([6, 12])
                .on_press(Message::ScopeSelected(scope))
                .style(move |_theme, status| chip_style(active, status)),
        )
    });

    let mut col = column![scope_row].spacing(10);
    if state.scope == LyricsGeneratorScope::ChooseSpecific {
        col = col.push(choose_specific(all_tracks, missing_lyrics_ids, state));
    }
    col.into()
}

fn choose_specific<'a>(all_tracks: &'a [Track], missing_lyrics_ids: &'a [String], state: &'a BatchLyricsGeneratorState) -> Element<'a, Message> {
    let query = state.choose_specific_search.trim().to_lowercase();
    let visible_ids: Vec<String> =
        all_tracks.iter().filter(|t| query.is_empty() || t.title.to_lowercase().contains(&query) || t.artist.to_lowercase().contains(&query)).map(|t| t.id().to_owned()).collect();
    let all_selected = !visible_ids.is_empty() && visible_ids.iter().all(|id| state.choose_specific_selected.contains(id));

    let header = row![
        text_input("Search", &state.choose_specific_search).on_input(Message::SearchChanged).padding(8).size(font::BODY.size).width(Length::Fixed(300.0)),
        iced::widget::Space::new(Length::Fill, Length::Shrink),
        button(text(if all_selected { "Deselect All" } else { "Select All" }).size(font::CALLOUT.size))
            .padding([6, 12])
            .on_press(Message::SelectAllToggled(visible_ids))
            .style(|_theme, status| chip_style(false, status)),
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center);

    let count_label = text(format!("{} of {} selected", state.choose_specific_selected.len(), all_tracks.len())).size(font::CAPTION.size).color(color::SILVER_MUTED);

    let list = all_tracks
        .iter()
        .filter(|t| query.is_empty() || t.title.to_lowercase().contains(&query) || t.artist.to_lowercase().contains(&query))
        .fold(column![].spacing(4), |col, track| {
            let id = track.id().to_owned();
            let checked = state.choose_specific_selected.contains(&id);
            let missing = missing_lyrics_ids.iter().any(|m| m == &id);
            let mut row_el = row![
                checkbox("", checked).on_toggle({
                    let id = id.clone();
                    move |v| Message::TrackToggled(id.clone(), v)
                }),
                text(track.title.as_str()).size(font::BODY.size).color(color::SILVER_LIGHT),
                text(track.artist.as_str()).size(font::CAPTION.size).color(color::SILVER_MUTED),
            ]
            .spacing(8)
            .align_y(iced::alignment::Vertical::Center);
            if missing {
                row_el = row_el.push(text("No lyrics").size(font::CAPTION_BOLD.size).color(color::HIRES_GOLD));
            }
            col.push(row_el)
        });

    column![header, count_label, scrollable(list).height(Length::Fixed(300.0))].spacing(6).into()
}

fn progress_and_results(state: &BatchLyricsGeneratorState) -> Element<'_, Message> {
    let fraction = if state.total == 0 { 0.0 } else { state.completed as f32 / state.total as f32 };
    let speed = items_per_second(state.completed, state.elapsed_seconds);
    let eta = eta_seconds(state.completed, state.total, state.elapsed_seconds);

    let counts = walkman_services::lyrics::tally(&state.outcomes);
    let pills = [
        (MetricFilter::All, "All", state.outcomes.len()),
        (MetricFilter::Synced, "Synced .LRC", counts.synced),
        (MetricFilter::Plain, "Plain Text", counts.plain),
        (MetricFilter::Skipped, "Skipped", counts.skipped),
        (MetricFilter::NotFound, "Not Found", counts.not_found),
    ]
    .into_iter()
    .fold(row![].spacing(6), |r, (filter, label, count)| {
        let active = state.filter == filter;
        r.push(
            button(text(format!("{label} ({count})")).size(font::CAPTION.size))
                .padding([4, 10])
                .on_press(Message::FilterSelected(filter))
                .style(move |_theme, status| chip_style(active, status)),
        )
    });

    let filtered = filter_outcomes(&state.outcomes, state.filter);
    let results = filtered.into_iter().fold(column![].spacing(4), |col, outcome| {
        col.push(
            row![
                text(outcome.id.as_str()).size(font::CAPTION.size).color(color::SILVER_LIGHT),
                text(outcome.kind.metric_label()).size(font::CAPTION_BOLD.size).color(color::SILVER_MUTED),
            ]
            .spacing(8),
        )
    });

    column![
        progress_bar(0.0..=1.0, fraction),
        text(format!("{:.1} tracks/sec \u{2022} ETA {:.0}s", speed, eta)).size(font::CAPTION.size).color(color::SILVER_MUTED),
        pills,
        scrollable(results).height(Length::Fixed(220.0)),
    ]
    .spacing(10)
    .into()
}

fn footer<'a>(all_in_view_ids: &'a [String], selected_ids: &'a [String], active_item_id: Option<&'a str>, state: &'a BatchLyricsGeneratorState) -> Element<'a, Message> {
    if state.is_running {
        return button(text("Cancel").size(font::CALLOUT.size)).padding([8, 16]).on_press(Message::CancelRunClicked).style(|_theme, s| destructive_style(s)).into();
    }
    if state.outcomes.is_empty() {
        let track_ids = resolve_track_ids(state.scope, all_in_view_ids, selected_ids, active_item_id, &state.choose_specific_selected);
        let can_generate = !track_ids.is_empty();
        let mut generate = button(text("Generate").size(font::CALLOUT.size)).padding([8, 16]);
        if can_generate {
            generate = generate.on_press(Message::GenerateClicked(track_ids));
        }
        return row![
            generate.style(|_theme, s| prominent_style(s)),
            button(text("Cancel").size(font::CALLOUT.size)).padding([8, 16]).on_press(Message::CancelClicked).style(|_theme, s| chip_style(false, s)),
        ]
        .spacing(8)
        .into();
    }
    button(text("Done").size(font::CALLOUT.size)).padding([8, 16]).on_press(Message::DoneClicked).style(|_theme, s| prominent_style(s)).into()
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
            radius: crate::iced_ui::theme::sizing::radius::CAPSULE.into(),
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

fn destructive_style(status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(iced::Background::Color(color::with_alpha(color::DESTRUCTIVE, if hovered { 0.95 } else { 0.85 }))),
        text_color: color::SILVER_LIGHT,
        border: iced::Border {
            color: color::with_alpha(color::DESTRUCTIVE_LIGHT, 0.35),
            width: 1.0,
            radius: crate::iced_ui::theme::sizing::radius::BUTTON.into(),
        },
        ..button::Style::default()
    }
}
