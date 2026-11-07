//! `LanguageStudyView` (spec task 14.7), ported from `LanguageStudyView.swift`: tracks under
//! `/LEARNING/` or a language/audiobook/podcast/learning genre, with 5 display modes, 6 sort
//! options, context menus, and batch lyrics (R5.6).

use eframe::egui;

use walkman_domain::models::Track;

use crate::selection;
use crate::theme::{button, color, font, glass, sizing};

/// R5.6's five display modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LanguageDisplayMode {
    #[default]
    Tracks,
    Albums,
    Artists,
    RecentlyAdded,
    Folders,
}

impl LanguageDisplayMode {
    pub const ALL: [LanguageDisplayMode; 5] =
        [Self::Tracks, Self::Albums, Self::Artists, Self::RecentlyAdded, Self::Folders];

    pub fn title(self) -> &'static str {
        match self {
            Self::Tracks => "Tracks",
            Self::Albums => "Albums",
            Self::Artists => "Artists",
            Self::RecentlyAdded => "Recently Added",
            Self::Folders => "Folders",
        }
    }
}

/// R5.6's six sort options ("Speaker/Artist" and "Course/Album" are the language-study
/// terminology for the same `artist`/`album` fields `library.rs`'s table sorts by).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LanguageSortOption {
    #[default]
    TitleAz,
    TitleZa,
    SpeakerArtistAz,
    CourseAlbumAz,
    DurationLongest,
    RecentlyAdded,
}

impl LanguageSortOption {
    pub const ALL: [LanguageSortOption; 6] = [
        Self::TitleAz,
        Self::TitleZa,
        Self::SpeakerArtistAz,
        Self::CourseAlbumAz,
        Self::DurationLongest,
        Self::RecentlyAdded,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::TitleAz => "Title A\u{2013}Z",
            Self::TitleZa => "Title Z\u{2013}A",
            Self::SpeakerArtistAz => "Speaker/Artist A\u{2013}Z",
            Self::CourseAlbumAz => "Course/Album A\u{2013}Z",
            Self::DurationLongest => "Duration (Longest First)",
            Self::RecentlyAdded => "Recently Added (First)",
        }
    }
}

/// R5.6's filter: reuses the real `Track::is_language_study()` (checks `/LEARNING/` path or a
/// language/audiobook/podcast/learning genre).
pub fn language_study_tracks(tracks: &[Track]) -> Vec<Track> {
    tracks.iter().filter(|t| t.is_language_study()).cloned().collect()
}

pub fn sort_tracks(tracks: &mut [Track], option: LanguageSortOption) {
    match option {
        LanguageSortOption::TitleAz => tracks.sort_by_key(|t| t.title.to_lowercase()),
        LanguageSortOption::TitleZa => tracks.sort_by_key(|t| std::cmp::Reverse(t.title.to_lowercase())),
        LanguageSortOption::SpeakerArtistAz => tracks.sort_by_key(|t| t.artist.to_lowercase()),
        LanguageSortOption::CourseAlbumAz => tracks.sort_by_key(|t| t.album.to_lowercase()),
        LanguageSortOption::DurationLongest => tracks.sort_by(|a, b| b.duration_seconds.total_cmp(&a.duration_seconds)),
        LanguageSortOption::RecentlyAdded => tracks.sort_by_key(|t| std::cmp::Reverse(t.date_added)),
    }
}

/// One row in the Folders display mode: tracks grouped by their immediate parent directory.
#[derive(Debug, Clone, PartialEq)]
pub struct FolderGroup {
    pub folder_name: String,
    pub track_count: usize,
}

pub fn group_by_folder(tracks: &[Track]) -> Vec<FolderGroup> {
    use std::collections::BTreeMap;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for t in tracks {
        let folder = std::path::Path::new(&t.full_path)
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Unknown".to_string());
        *counts.entry(folder).or_insert(0) += 1;
    }
    counts.into_iter().map(|(folder_name, track_count)| FolderGroup { folder_name, track_count }).collect()
}

#[derive(Debug, Clone, PartialEq)]
pub enum LanguageStudyCommand {
    Play(String),
    EditMetadata(String),
    TrimCut(String),
    Transfer(Vec<String>),
    Delete(Vec<String>),
    BatchLyrics(Vec<String>),
}

#[derive(Debug, Default)]
pub struct LanguageStudyViewState {
    pub display_mode: LanguageDisplayMode,
    pub sort: LanguageSortOption,
    pub selected: Vec<String>,
    pub selection_anchor: Option<usize>,
    pub commands: Vec<LanguageStudyCommand>,
}

pub fn render(ui: &mut egui::Ui, all_tracks: &[Track], state: &mut LanguageStudyViewState) {
    let tracks = language_study_tracks(all_tracks);

    ui.horizontal(|ui| {
        for mode in LanguageDisplayMode::ALL {
            let variant = if state.display_mode == mode { button::Variant::Capsule } else { button::Variant::Glass };
            if ui.add(button::Button::new(mode.title(), variant)).clicked() {
                state.display_mode = mode;
            }
        }
        if state.display_mode != LanguageDisplayMode::Folders {
            ui.add_space(12.0);
            egui::ComboBox::from_id_salt("language_study_sort").selected_text(state.sort.title()).show_ui(ui, |ui| {
                for option in LanguageSortOption::ALL {
                    ui.selectable_value(&mut state.sort, option, option.title());
                }
            });
        }
        ui.add_space(12.0);
        let has_selection = !state.selected.is_empty();
        if ui.add(button::Button::new("Find Lyrics\u{2026}", button::Variant::Glass).enabled(has_selection)).clicked() {
            state.commands.push(LanguageStudyCommand::BatchLyrics(state.selected.clone()));
        }
    });
    ui.add_space(8.0);

    match state.display_mode {
        LanguageDisplayMode::Tracks => render_track_table(ui, &tracks, state),
        LanguageDisplayMode::RecentlyAdded => {
            let mut sorted = tracks.clone();
            sort_tracks(&mut sorted, LanguageSortOption::RecentlyAdded);
            render_track_table(ui, &sorted, state);
        }
        LanguageDisplayMode::Albums => render_course_table(ui, &tracks),
        LanguageDisplayMode::Artists => render_speaker_table(ui, &tracks),
        LanguageDisplayMode::Folders => render_folder_table(ui, &tracks),
    }
}

fn render_track_table(ui: &mut egui::Ui, tracks: &[Track], state: &mut LanguageStudyViewState) {
    let mut owned = tracks.to_vec();
    sort_tracks(&mut owned, state.sort);
    let all_ids: Vec<String> = owned.iter().map(|t| t.full_path.clone()).collect();

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (index, track) in owned.iter().enumerate() {
            render_track_row(ui, index, track, &all_ids, state);
        }
        ui.add_space(sizing::TABLE_BOTTOM_INSET);
    });
}

fn render_track_row(ui: &mut egui::Ui, index: usize, track: &Track, all_ids: &[String], state: &mut LanguageStudyViewState) {
    let selected = state.selected.contains(&track.full_path);
    let glass_state = if selected { glass::State::Selected } else { glass::State::Normal };
    let frame = glass::liquid_glass(glass_state);

    let response = frame
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&track.title).font(font::body_medium()).color(color::SILVER_LIGHT));
                ui.add_space(12.0);
                ui.label(egui::RichText::new(&track.artist).font(font::body()).color(color::SILVER_MUTED));
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new(walkman_services::walkman_engine::format_duration(track.duration_seconds))
                        .font(font::mono())
                        .color(color::SILVER_MUTED),
                );
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
            state.commands.push(LanguageStudyCommand::Play(path.clone()));
            ui.close_menu();
        }
        if ui.button("Edit Metadata\u{2026}").clicked() {
            state.commands.push(LanguageStudyCommand::EditMetadata(path.clone()));
            ui.close_menu();
        }
        if ui.button("Trim & Cut\u{2026}").clicked() {
            state.commands.push(LanguageStudyCommand::TrimCut(path.clone()));
            ui.close_menu();
        }
        if ui.button("Transfer\u{2026}").clicked() {
            state.commands.push(LanguageStudyCommand::Transfer(vec![path.clone()]));
            ui.close_menu();
        }
        ui.separator();
        if ui.button("Delete").clicked() {
            state.commands.push(LanguageStudyCommand::Delete(vec![path.clone()]));
            ui.close_menu();
        }
    });
    ui.add_space(2.0);
}

fn render_course_table(ui: &mut egui::Ui, tracks: &[Track]) {
    let albums = crate::views::albums::group_into_albums(tracks);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for album in &albums {
            let frame = glass::liquid_glass(glass::State::Normal);
            frame.show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&album.name).font(font::body_medium()).color(color::SILVER_LIGHT));
                    ui.label(
                        egui::RichText::new(format!("{} \u{2022} {} lessons", album.artist, album.track_count))
                            .font(font::caption())
                            .color(color::SILVER_MUTED),
                    );
                });
            });
            ui.add_space(2.0);
        }
        ui.add_space(sizing::TABLE_BOTTOM_INSET);
    });
}

fn render_speaker_table(ui: &mut egui::Ui, tracks: &[Track]) {
    let artists = crate::views::artists::group_into_artists(tracks);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for artist in &artists {
            let frame = glass::liquid_glass(glass::State::Normal);
            frame.show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&artist.name).font(font::body_medium()).color(color::SILVER_LIGHT));
                    ui.label(
                        egui::RichText::new(format!("{} tracks", artist.track_count))
                            .font(font::caption())
                            .color(color::SILVER_MUTED),
                    );
                });
            });
            ui.add_space(2.0);
        }
        ui.add_space(sizing::TABLE_BOTTOM_INSET);
    });
}

fn render_folder_table(ui: &mut egui::Ui, tracks: &[Track]) {
    let folders = group_by_folder(tracks);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for folder in &folders {
            let frame = glass::liquid_glass(glass::State::Normal);
            frame.show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&folder.folder_name).font(font::body_medium()).color(color::SILVER_LIGHT));
                    ui.label(
                        egui::RichText::new(format!("{} tracks", folder.track_count))
                            .font(font::caption())
                            .color(color::SILVER_MUTED),
                    );
                });
            });
            ui.add_space(2.0);
        }
        ui.add_space(sizing::TABLE_BOTTOM_INSET);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn track(path: &str, title: &str, artist: &str, album: &str, genre: &str, duration: f64) -> Track {
        Track {
            filename: title.to_string(),
            relative_path: path.to_string(),
            full_path: path.to_string(),
            format: "MP3".into(),
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            track_num: "1".into(),
            year: "2024".into(),
            genre: genre.into(),
            duration_seconds: duration,
            bitrate_kbps: 128,
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
    fn language_study_tracks_filters_by_path_or_genre() {
        let tracks = [
            track("/LEARNING/French/lesson1.mp3", "Lesson 1", "Tutor", "French 101", "", 300.0),
            track("/Music/song.mp3", "Song", "Band", "Album", "Rock", 200.0),
            track("/Music/book.mp3", "Chapter 1", "Author", "Book", "Audiobook", 1800.0),
        ];
        let filtered = language_study_tracks(&tracks);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().any(|t| t.title == "Lesson 1"));
        assert!(filtered.iter().any(|t| t.title == "Chapter 1"));
    }

    #[test]
    fn sort_by_duration_longest_first() {
        let mut tracks = [
            track("/LEARNING/a.mp3", "Short", "T", "C", "", 100.0),
            track("/LEARNING/b.mp3", "Long", "T", "C", "", 3000.0),
        ];
        sort_tracks(&mut tracks, LanguageSortOption::DurationLongest);
        assert_eq!(tracks[0].title, "Long");
    }

    #[test]
    fn group_by_folder_counts_by_parent_directory() {
        let tracks = [
            track("/LEARNING/French/lesson1.mp3", "L1", "T", "C", "", 1.0),
            track("/LEARNING/French/lesson2.mp3", "L2", "T", "C", "", 1.0),
            track("/LEARNING/Spanish/lesson1.mp3", "L3", "T", "C", "", 1.0),
        ];
        let folders = group_by_folder(&tracks);
        assert_eq!(folders.len(), 2);
        let french = folders.iter().find(|f| f.folder_name == "French").unwrap();
        assert_eq!(french.track_count, 2);
    }

    #[test]
    fn renders_in_headless_frame_for_every_display_mode() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let tracks = vec![
            track("/LEARNING/French/lesson1.mp3", "Lesson 1", "Tutor", "French 101", "Language", 300.0),
            track("/LEARNING/French/lesson2.mp3", "Lesson 2", "Tutor", "French 101", "Language", 310.0),
        ];

        for mode in LanguageDisplayMode::ALL {
            let mut state = LanguageStudyViewState { display_mode: mode, ..Default::default() };
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render(ui, &tracks, &mut state);
                });
            });
        }
    }
}
