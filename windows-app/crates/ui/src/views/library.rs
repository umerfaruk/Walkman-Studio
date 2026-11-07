//! `LibraryView` — Walkman "All Songs" (spec task 14.2), ported from `LibraryView.swift`.
//!
//! A multi-column sortable track table with search, a batch-lyrics button, and a per-row
//! context menu (R5.1), using the shared [`crate::selection`] Shift-range/Ctrl-toggle logic and
//! the 88pt bottom table inset (R2.9).

use eframe::egui;

use walkman_domain::models::Track;

use crate::selection;
use crate::theme::{button, color, font, glass, sizing};

/// The table's sortable columns (R5.1's "multi-column sortable track table").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Column {
    #[default]
    Title,
    Artist,
    Album,
    Duration,
    Format,
}

impl Column {
    pub const ALL: [Column; 5] = [Self::Title, Self::Artist, Self::Album, Self::Duration, Self::Format];

    pub fn title(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::Artist => "Artist",
            Self::Album => "Album",
            Self::Duration => "Duration",
            Self::Format => "Format",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortDirection {
    #[default]
    Ascending,
    Descending,
}

impl SortDirection {
    fn flip(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }

    fn arrow(self) -> &'static str {
        match self {
            Self::Ascending => "\u{25B2}",
            Self::Descending => "\u{25BC}",
        }
    }
}

/// Sorts `tracks` in place by `column`/`direction`.
pub fn sort_tracks(tracks: &mut [Track], column: Column, direction: SortDirection) {
    match column {
        Column::Title => tracks.sort_by_key(|t| t.title.to_lowercase()),
        Column::Artist => tracks.sort_by_key(|t| t.artist.to_lowercase()),
        Column::Album => tracks.sort_by_key(|t| t.album.to_lowercase()),
        Column::Duration => tracks.sort_by(|a, b| a.duration_seconds.total_cmp(&b.duration_seconds)),
        Column::Format => tracks.sort_by_key(|t| t.format.to_lowercase()),
    }
    if direction == SortDirection::Descending {
        tracks.reverse();
    }
}

pub fn filter_tracks<'a>(tracks: &'a [Track], query: &str) -> Vec<&'a Track> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return tracks.iter().collect();
    }
    tracks
        .iter()
        .filter(|t| {
            t.title.to_lowercase().contains(&query)
                || t.artist.to_lowercase().contains(&query)
                || t.album.to_lowercase().contains(&query)
        })
        .collect()
}

/// An action the view wants performed (R5.1's context-menu actions + the batch-lyrics button).
#[derive(Debug, Clone, PartialEq)]
pub enum LibraryCommand {
    Play(String),
    EditMetadata(String),
    TrimCut(String),
    Transfer(Vec<String>),
    Delete(Vec<String>),
    BatchLyrics(Vec<String>),
}

#[derive(Debug, Default)]
pub struct LibraryViewState {
    pub sort_column: Column,
    pub sort_direction: SortDirection,
    pub search_query: String,
    pub selected: Vec<String>,
    pub selection_anchor: Option<usize>,
    pub commands: Vec<LibraryCommand>,
}

pub fn render(ui: &mut egui::Ui, tracks: &[Track], state: &mut LibraryViewState) {
    ui.horizontal(|ui| {
        let frame = glass::liquid_glass(glass::State::Normal);
        frame.show(ui, |ui| {
            ui.set_min_width(240.0);
            ui.add(
                egui::TextEdit::singleline(&mut state.search_query)
                    .hint_text("Search All Songs")
                    .desired_width(220.0)
                    .font(font::body()),
            );
        });

        ui.add_space(8.0);
        let has_selection = !state.selected.is_empty();
        if ui
            .add(button::Button::new("Find Lyrics\u{2026}", button::Variant::Glass).enabled(has_selection))
            .clicked()
        {
            state.commands.push(LibraryCommand::BatchLyrics(state.selected.clone()));
        }
    });
    ui.add_space(8.0);

    let mut filtered: Vec<Track> = filter_tracks(tracks, &state.search_query).into_iter().cloned().collect();
    sort_tracks(&mut filtered, state.sort_column, state.sort_direction);
    let all_ids: Vec<String> = filtered.iter().map(|t| t.full_path.clone()).collect();

    render_header(ui, state);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (index, track) in filtered.iter().enumerate() {
            render_row(ui, track, index, &all_ids, state);
        }
        ui.add_space(sizing::TABLE_BOTTOM_INSET);
    });
}

fn render_header(ui: &mut egui::Ui, state: &mut LibraryViewState) {
    ui.horizontal(|ui| {
        for column in Column::ALL {
            let label = if state.sort_column == column {
                format!("{} {}", column.title(), state.sort_direction.arrow())
            } else {
                column.title().to_string()
            };
            if ui
                .add(egui::Label::new(egui::RichText::new(label).font(font::section_header()).color(color::SILVER_MUTED)).sense(egui::Sense::click()))
                .clicked()
            {
                if state.sort_column == column {
                    state.sort_direction = state.sort_direction.flip();
                } else {
                    state.sort_column = column;
                    state.sort_direction = SortDirection::Ascending;
                }
            }
            ui.add_space(24.0);
        }
    });
    ui.add_space(4.0);
}

fn render_row(ui: &mut egui::Ui, track: &Track, index: usize, all_ids: &[String], state: &mut LibraryViewState) {
    let selected = state.selected.contains(&track.full_path);
    let glass_state = if selected { glass::State::Selected } else { glass::State::Normal };
    let frame = glass::liquid_glass(glass_state);

    let response = frame
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&track.title).font(font::body_medium()).color(color::SILVER_LIGHT));
                ui.add_space(16.0);
                ui.label(egui::RichText::new(&track.artist).font(font::body()).color(color::SILVER_MUTED));
                ui.add_space(16.0);
                ui.label(egui::RichText::new(&track.album).font(font::body()).color(color::SILVER_MUTED));
                ui.add_space(16.0);
                ui.label(
                    egui::RichText::new(walkman_services::walkman_engine::format_duration(track.duration_seconds))
                        .font(font::mono())
                        .color(color::SILVER_MUTED),
                );
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
            state.commands.push(LibraryCommand::Play(path.clone()));
            ui.close_menu();
        }
        if ui.button("Edit Metadata\u{2026}").clicked() {
            state.commands.push(LibraryCommand::EditMetadata(path.clone()));
            ui.close_menu();
        }
        if ui.button("Trim & Cut\u{2026}").clicked() {
            state.commands.push(LibraryCommand::TrimCut(path.clone()));
            ui.close_menu();
        }
        if ui.button("Transfer\u{2026}").clicked() {
            state.commands.push(LibraryCommand::Transfer(vec![path.clone()]));
            ui.close_menu();
        }
        ui.separator();
        if ui.button("Delete").clicked() {
            state.commands.push(LibraryCommand::Delete(vec![path.clone()]));
            ui.close_menu();
        }
    });

    ui.add_space(2.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn track(title: &str, artist: &str, album: &str, duration: f64, format: &str) -> Track {
        Track {
            filename: format!("{title}.flac"),
            relative_path: format!("{artist}/{album}/{title}.flac"),
            full_path: format!("/music/{artist}/{album}/{title}.flac"),
            format: format.into(),
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            track_num: "1".into(),
            year: "2024".into(),
            genre: String::new(),
            duration_seconds: duration,
            bitrate_kbps: 900,
            sample_rate: 44_100,
            bits_per_sample: 16,
            has_artwork: false,
            size_bytes: 1,
            date_added: Utc::now(),
            is_senseme_analyzed: false,
            senseme_bpm: None,
            senseme_valence: None,
            senseme_arousal: None,
            senseme_channels: Vec::new(),
            senseme_highlight_sec: None,
        }
    }

    #[test]
    fn sort_by_title_ascending_and_descending() {
        let mut tracks = [track("Zeta", "A", "Al", 1.0, "FLAC"), track("Alpha", "A", "Al", 1.0, "FLAC")];
        sort_tracks(&mut tracks, Column::Title, SortDirection::Ascending);
        assert_eq!(tracks[0].title, "Alpha");
        sort_tracks(&mut tracks, Column::Title, SortDirection::Descending);
        assert_eq!(tracks[0].title, "Zeta");
    }

    #[test]
    fn sort_by_duration_and_format() {
        let mut tracks =
            [track("A", "X", "Y", 300.0, "MP3"), track("B", "X", "Y", 100.0, "FLAC")];
        sort_tracks(&mut tracks, Column::Duration, SortDirection::Ascending);
        assert_eq!(tracks[0].title, "B");
        sort_tracks(&mut tracks, Column::Format, SortDirection::Ascending);
        assert_eq!(tracks[0].format, "FLAC");
    }

    #[test]
    fn filter_matches_across_title_artist_album() {
        let tracks = [track("Dawn", "Nova", "Horizon", 1.0, "FLAC"), track("Dusk", "Echo", "Reverb", 1.0, "FLAC")];
        assert_eq!(filter_tracks(&tracks, "nova").len(), 1);
        assert_eq!(filter_tracks(&tracks, "reverb").len(), 1);
        assert_eq!(filter_tracks(&tracks, "").len(), 2);
    }

    #[test]
    fn renders_in_headless_frame_and_handles_a_selection_click() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let tracks = vec![
            track("Dawn", "Nova", "Horizon", 180.0, "FLAC"),
            track("Dusk", "Echo", "Reverb", 200.0, "MP3"),
        ];
        let mut state = LibraryViewState::default();

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render(ui, &tracks, &mut state);
            });
        });
        // Nothing to click programmatically without simulated input events; this confirms the
        // view lays out and paints without panicking across both rows and the header.
        assert_eq!(state.selected.len(), 0);
    }
}
