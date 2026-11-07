//! `RecentlyAddedView` (spec task 14.3), ported from `RecentlyAddedView.swift`: ranked
//! recently-added tracks with a time-scope picker and a batch-lyrics button (R5.2).

use chrono::{DateTime, Utc};
use eframe::egui;

use walkman_domain::models::Track;

use crate::selection;
use crate::theme::{button, color, font, glass, sizing};

/// R5.2's four time scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeScope {
    Past24Hours,
    Past7Days,
    Past30Days,
    #[default]
    AllTime,
}

impl TimeScope {
    pub const ALL: [TimeScope; 4] = [Self::Past24Hours, Self::Past7Days, Self::Past30Days, Self::AllTime];

    pub fn title(self) -> &'static str {
        match self {
            Self::Past24Hours => "Past 24 Hours",
            Self::Past7Days => "Past 7 Days",
            Self::Past30Days => "Past 30 Days",
            Self::AllTime => "All Time",
        }
    }

    fn max_age(self) -> Option<chrono::Duration> {
        match self {
            Self::Past24Hours => Some(chrono::Duration::hours(24)),
            Self::Past7Days => Some(chrono::Duration::days(7)),
            Self::Past30Days => Some(chrono::Duration::days(30)),
            Self::AllTime => None,
        }
    }
}

/// Filters `tracks` to `scope`'s window (relative to `now`) and ranks them newest-first.
pub fn ranked_tracks(tracks: &[Track], scope: TimeScope, now: DateTime<Utc>) -> Vec<Track> {
    let mut filtered: Vec<Track> = match scope.max_age() {
        Some(max_age) => tracks.iter().filter(|t| now.signed_duration_since(t.date_added) <= max_age).cloned().collect(),
        None => tracks.to_vec(),
    };
    filtered.sort_by_key(|t| std::cmp::Reverse(t.date_added));
    filtered
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecentlyAddedCommand {
    Play(String),
    EditMetadata(String),
    TrimCut(String),
    Transfer(Vec<String>),
    Delete(Vec<String>),
    BatchLyrics(Vec<String>),
}

#[derive(Debug, Default)]
pub struct RecentlyAddedViewState {
    pub scope: TimeScope,
    pub selected: Vec<String>,
    pub selection_anchor: Option<usize>,
    pub commands: Vec<RecentlyAddedCommand>,
}

pub fn render(ui: &mut egui::Ui, tracks: &[Track], now: DateTime<Utc>, state: &mut RecentlyAddedViewState) {
    ui.horizontal(|ui| {
        for scope in TimeScope::ALL {
            let variant = if state.scope == scope { button::Variant::Capsule } else { button::Variant::Glass };
            if ui.add(button::Button::new(scope.title(), variant)).clicked() {
                state.scope = scope;
            }
        }

        ui.add_space(12.0);
        let has_selection = !state.selected.is_empty();
        if ui
            .add(button::Button::new("Find Lyrics\u{2026}", button::Variant::Glass).enabled(has_selection))
            .clicked()
        {
            state.commands.push(RecentlyAddedCommand::BatchLyrics(state.selected.clone()));
        }
    });
    ui.add_space(8.0);

    let ranked = ranked_tracks(tracks, state.scope, now);
    let all_ids: Vec<String> = ranked.iter().map(|t| t.full_path.clone()).collect();

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (index, track) in ranked.iter().enumerate() {
            render_row(ui, index, track, &all_ids, state);
        }
        ui.add_space(sizing::TABLE_BOTTOM_INSET);
    });
}

fn render_row(ui: &mut egui::Ui, index: usize, track: &Track, all_ids: &[String], state: &mut RecentlyAddedViewState) {
    let selected = state.selected.contains(&track.full_path);
    let glass_state = if selected { glass::State::Selected } else { glass::State::Normal };
    let frame = glass::liquid_glass(glass_state);

    let response = frame
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("#{}", index + 1)).font(font::mono()).color(color::SILVER_MUTED),
                );
                ui.add_space(8.0);
                ui.label(egui::RichText::new(&track.title).font(font::body_medium()).color(color::SILVER_LIGHT));
                ui.add_space(12.0);
                ui.label(egui::RichText::new(&track.artist).font(font::body()).color(color::SILVER_MUTED));
                if track.bits_per_sample > 16 || track.sample_rate > 48_000 {
                    crate::theme::widgets::hi_res_badge(ui);
                }
            });
        })
        .response
        .interact(egui::Sense::click());

    if response.clicked() {
        let (shift, ctrl) = ui.input(|i| (i.modifiers.shift, i.modifiers.command || i.modifiers.ctrl));
        selection::handle_click(&mut state.selected, &mut state.selection_anchor, all_ids, index, &track.full_path, shift, ctrl);
    }

    let path = track.full_path.clone();
    response.context_menu(|ui| {
        if ui.button("Play").clicked() {
            state.commands.push(RecentlyAddedCommand::Play(path.clone()));
            ui.close_menu();
        }
        if ui.button("Edit Metadata\u{2026}").clicked() {
            state.commands.push(RecentlyAddedCommand::EditMetadata(path.clone()));
            ui.close_menu();
        }
        if ui.button("Trim & Cut\u{2026}").clicked() {
            state.commands.push(RecentlyAddedCommand::TrimCut(path.clone()));
            ui.close_menu();
        }
        if ui.button("Transfer\u{2026}").clicked() {
            state.commands.push(RecentlyAddedCommand::Transfer(vec![path.clone()]));
            ui.close_menu();
        }
        ui.separator();
        if ui.button("Delete").clicked() {
            state.commands.push(RecentlyAddedCommand::Delete(vec![path.clone()]));
            ui.close_menu();
        }
    });

    ui.add_space(2.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track_added(hours_ago: i64) -> Track {
        Track {
            filename: "x".into(),
            relative_path: "x".into(),
            full_path: format!("/music/track-{hours_ago}.flac"),
            format: "FLAC".into(),
            title: format!("Track {hours_ago}"),
            artist: "Artist".into(),
            album: "Album".into(),
            track_num: "1".into(),
            year: "2024".into(),
            genre: String::new(),
            duration_seconds: 180.0,
            bitrate_kbps: 900,
            sample_rate: 44_100,
            bits_per_sample: 16,
            has_artwork: false,
            size_bytes: 1,
            date_added: Utc::now() - chrono::Duration::hours(hours_ago),
            is_senseme_analyzed: false,
            senseme_bpm: None,
            senseme_valence: None,
            senseme_arousal: None,
            senseme_channels: Vec::new(),
            senseme_highlight_sec: None,
        }
    }

    #[test]
    fn past_24_hours_excludes_older_tracks() {
        let now = Utc::now();
        let tracks = [track_added(1), track_added(48)];
        let ranked = ranked_tracks(&tracks, TimeScope::Past24Hours, now);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].title, "Track 1");
    }

    #[test]
    fn past_7_and_30_days_widen_the_window() {
        let now = Utc::now();
        let tracks = [track_added(24 * 5), track_added(24 * 20), track_added(24 * 40)];
        assert_eq!(ranked_tracks(&tracks, TimeScope::Past7Days, now).len(), 1);
        assert_eq!(ranked_tracks(&tracks, TimeScope::Past30Days, now).len(), 2);
    }

    #[test]
    fn all_time_includes_everything_and_ranks_newest_first() {
        let now = Utc::now();
        let tracks = [track_added(100), track_added(1), track_added(50)];
        let ranked = ranked_tracks(&tracks, TimeScope::AllTime, now);
        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].title, "Track 1");
        assert_eq!(ranked[2].title, "Track 100");
    }

    #[test]
    fn renders_in_headless_frame_for_every_scope() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let tracks = vec![track_added(1), track_added(200)];
        let now = Utc::now();

        for scope in TimeScope::ALL {
            let mut state = RecentlyAddedViewState { scope, ..Default::default() };
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render(ui, &tracks, now, &mut state);
                });
            });
        }
    }
}
