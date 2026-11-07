//! `HiResAudioView` (spec task 14.6), ported from `HiResAudioView.swift`: tracks filtered to
//! more than 16-bit or more than 48kHz, with Tracks/Albums/Artists view modes, their own sort
//! options, a format/quality filter, search, context menus, and batch lyrics (R5.5).

use eframe::egui;

use walkman_domain::models::Track;

use crate::selection;
use crate::theme::{button, color, font, glass, sizing};
use crate::views::albums::{self, AlbumSortOption};

/// R5.5's three view modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HiResViewMode {
    #[default]
    Tracks,
    Albums,
    Artists,
}

impl HiResViewMode {
    pub const ALL: [HiResViewMode; 3] = [Self::Tracks, Self::Albums, Self::Artists];

    pub fn title(self) -> &'static str {
        match self {
            Self::Tracks => "Tracks",
            Self::Albums => "Albums",
            Self::Artists => "Artists",
        }
    }
}

/// R5.5's six track sort options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrackSortOption {
    #[default]
    Default,
    Title,
    Artist,
    Album,
    HighestQuality,
    RecentlyAdded,
}

impl TrackSortOption {
    pub const ALL: [TrackSortOption; 6] =
        [Self::Default, Self::Title, Self::Artist, Self::Album, Self::HighestQuality, Self::RecentlyAdded];

    pub fn title(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Title => "Title",
            Self::Artist => "Artist",
            Self::Album => "Album",
            Self::HighestQuality => "Highest Quality",
            Self::RecentlyAdded => "Recently Added",
        }
    }
}

/// R5.5's "filter to tracks >16-bit or >48kHz".
pub fn hi_res_tracks(tracks: &[Track]) -> Vec<Track> {
    tracks.iter().filter(|t| t.is_hi_res()).cloned().collect()
}

pub fn sort_tracks(tracks: &mut [Track], option: TrackSortOption) {
    match option {
        TrackSortOption::Default => {}
        TrackSortOption::Title => tracks.sort_by_key(|t| t.title.to_lowercase()),
        TrackSortOption::Artist => tracks.sort_by_key(|t| t.artist.to_lowercase()),
        TrackSortOption::Album => tracks.sort_by_key(|t| t.album.to_lowercase()),
        TrackSortOption::HighestQuality => {
            tracks.sort_by_key(|t| std::cmp::Reverse(t.bits_per_sample as i64 * t.sample_rate as i64));
        }
        TrackSortOption::RecentlyAdded => tracks.sort_by_key(|t| std::cmp::Reverse(t.date_added)),
    }
}

/// The format/quality filter chip (R5.5): `All`, a specific format, or `HiRes` isn't needed
/// here (every track in this view is already Hi-Res) — `Format` is the only real narrowing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum QualityFilter {
    #[default]
    All,
    Format(String),
}

pub fn filter_tracks<'a>(tracks: &'a [Track], filter: &QualityFilter, query: &str) -> Vec<&'a Track> {
    let query = query.trim().to_lowercase();
    tracks
        .iter()
        .filter(|t| match filter {
            QualityFilter::All => true,
            QualityFilter::Format(f) => t.format.eq_ignore_ascii_case(f),
        })
        .filter(|t| {
            query.is_empty()
                || t.title.to_lowercase().contains(&query)
                || t.artist.to_lowercase().contains(&query)
                || t.album.to_lowercase().contains(&query)
        })
        .collect()
}

pub fn distinct_formats(tracks: &[Track]) -> Vec<String> {
    let mut formats: Vec<String> = tracks.iter().map(|t| t.format.clone()).collect();
    formats.sort();
    formats.dedup();
    formats
}

#[derive(Debug, Clone, PartialEq)]
pub enum HiResCommand {
    Play(String),
    EditMetadata(String),
    TrimCut(String),
    Transfer(Vec<String>),
    Delete(Vec<String>),
    BatchLyrics(Vec<String>),
}

#[derive(Debug, Default)]
pub struct HiResViewState {
    pub mode: HiResViewMode,
    pub track_sort: TrackSortOption,
    pub album_sort: AlbumSortOption,
    pub quality_filter: QualityFilter,
    pub search_query: String,
    pub selected: Vec<String>,
    pub selection_anchor: Option<usize>,
    pub commands: Vec<HiResCommand>,
}

pub fn render(ui: &mut egui::Ui, all_tracks: &[Track], state: &mut HiResViewState) {
    let tracks = hi_res_tracks(all_tracks);

    render_toolbar(ui, &tracks, state);
    ui.add_space(8.0);

    match state.mode {
        HiResViewMode::Tracks => render_tracks(ui, &tracks, state),
        HiResViewMode::Albums => render_albums(ui, &tracks, state),
        HiResViewMode::Artists => render_artists(ui, &tracks),
    }
}

fn render_toolbar(ui: &mut egui::Ui, tracks: &[Track], state: &mut HiResViewState) {
    ui.horizontal(|ui| {
        for mode in HiResViewMode::ALL {
            let variant = if state.mode == mode { button::Variant::Capsule } else { button::Variant::Glass };
            if ui.add(button::Button::new(mode.title(), variant)).clicked() {
                state.mode = mode;
            }
        }
        ui.add_space(12.0);

        if state.mode == HiResViewMode::Tracks {
            let all_variant = if state.quality_filter == QualityFilter::All { button::Variant::Capsule } else { button::Variant::Glass };
            if ui.add(button::Button::new("ALL", all_variant)).clicked() {
                state.quality_filter = QualityFilter::All;
            }
            for format in distinct_formats(tracks) {
                let active = state.quality_filter == QualityFilter::Format(format.clone());
                let variant = if active { button::Variant::Capsule } else { button::Variant::Glass };
                if ui.add(button::Button::new(&format, variant)).clicked() {
                    state.quality_filter = QualityFilter::Format(format.clone());
                }
            }
            ui.add_space(12.0);
            egui::ComboBox::from_id_salt("hi_res_track_sort").selected_text(state.track_sort.title()).show_ui(ui, |ui| {
                for option in TrackSortOption::ALL {
                    ui.selectable_value(&mut state.track_sort, option, option.title());
                }
            });
        } else if state.mode == HiResViewMode::Albums {
            egui::ComboBox::from_id_salt("hi_res_album_sort").selected_text(state.album_sort.title()).show_ui(ui, |ui| {
                for option in AlbumSortOption::ALL {
                    ui.selectable_value(&mut state.album_sort, option, option.title());
                }
            });
        }

        ui.add_space(12.0);
        let frame = glass::liquid_glass(glass::State::Normal);
        frame.show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.search_query)
                    .hint_text("Search Hi-Res Audio")
                    .desired_width(200.0)
                    .font(font::body()),
            );
        });

        ui.add_space(12.0);
        let has_selection = !state.selected.is_empty();
        if ui.add(button::Button::new("Find Lyrics\u{2026}", button::Variant::Glass).enabled(has_selection)).clicked() {
            state.commands.push(HiResCommand::BatchLyrics(state.selected.clone()));
        }
    });
}

fn render_tracks(ui: &mut egui::Ui, tracks: &[Track], state: &mut HiResViewState) {
    let filtered = filter_tracks(tracks, &state.quality_filter, &state.search_query);
    let mut owned: Vec<Track> = filtered.into_iter().cloned().collect();
    sort_tracks(&mut owned, state.track_sort);
    let all_ids: Vec<String> = owned.iter().map(|t| t.full_path.clone()).collect();

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (index, track) in owned.iter().enumerate() {
            render_track_row(ui, index, track, &all_ids, state);
        }
        ui.add_space(sizing::TABLE_BOTTOM_INSET);
    });
}

fn render_track_row(ui: &mut egui::Ui, index: usize, track: &Track, all_ids: &[String], state: &mut HiResViewState) {
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
                crate::theme::widgets::hi_res_badge(ui);
                ui.label(
                    egui::RichText::new(format!("{}bit/{}kHz", track.bits_per_sample, track.sample_rate / 1000))
                        .font(font::mono_small())
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
            state.commands.push(HiResCommand::Play(path.clone()));
            ui.close_menu();
        }
        if ui.button("Edit Metadata\u{2026}").clicked() {
            state.commands.push(HiResCommand::EditMetadata(path.clone()));
            ui.close_menu();
        }
        if ui.button("Trim & Cut\u{2026}").clicked() {
            state.commands.push(HiResCommand::TrimCut(path.clone()));
            ui.close_menu();
        }
        if ui.button("Transfer\u{2026}").clicked() {
            state.commands.push(HiResCommand::Transfer(vec![path.clone()]));
            ui.close_menu();
        }
        ui.separator();
        if ui.button("Delete").clicked() {
            state.commands.push(HiResCommand::Delete(vec![path.clone()]));
            ui.close_menu();
        }
    });
    ui.add_space(2.0);
}

fn render_albums(ui: &mut egui::Ui, tracks: &[Track], state: &mut HiResViewState) {
    let mut grouped = albums::group_into_albums(tracks);
    albums::sort_albums(&mut grouped, state.album_sort);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for album in &grouped {
            let frame = glass::liquid_glass(glass::State::Normal);
            frame.show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&album.name).font(font::body_medium()).color(color::SILVER_LIGHT));
                    ui.label(egui::RichText::new(&album.artist).font(font::caption()).color(color::SILVER_MUTED));
                    crate::theme::widgets::hi_res_badge(ui);
                });
            });
            ui.add_space(2.0);
        }
        ui.add_space(sizing::TABLE_BOTTOM_INSET);
    });
}

fn render_artists(ui: &mut egui::Ui, tracks: &[Track]) {
    let grouped = crate::views::artists::group_into_artists(tracks);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for artist in &grouped {
            let frame = glass::liquid_glass(glass::State::Normal);
            frame.show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(egui::RichText::new(&artist.name).font(font::body_medium()).color(color::SILVER_LIGHT));
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

    fn track(title: &str, artist: &str, album: &str, bits: i32, rate: i32, format: &str) -> Track {
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
            duration_seconds: 180.0,
            bitrate_kbps: 900,
            sample_rate: rate,
            bits_per_sample: bits,
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
    fn hi_res_tracks_filters_to_over_16_bit_or_over_48khz() {
        let tracks = [
            track("A", "X", "Y", 16, 44_100, "MP3"),
            track("B", "X", "Y", 24, 48_000, "FLAC"),
            track("C", "X", "Y", 16, 96_000, "FLAC"),
        ];
        let hi_res = hi_res_tracks(&tracks);
        assert_eq!(hi_res.len(), 2);
        assert!(hi_res.iter().all(|t| t.title != "A"));
    }

    #[test]
    fn sort_by_highest_quality_prefers_bits_times_rate() {
        let mut tracks = [
            track("Low", "X", "Y", 24, 48_000, "FLAC"),
            track("High", "X", "Y", 24, 192_000, "FLAC"),
        ];
        sort_tracks(&mut tracks, TrackSortOption::HighestQuality);
        assert_eq!(tracks[0].title, "High");
    }

    #[test]
    fn filter_by_format_and_search() {
        let tracks = [track("Dawn", "Nova", "Horizon", 24, 96_000, "FLAC"), track("Dusk", "Echo", "Reverb", 24, 96_000, "ALAC")];
        assert_eq!(filter_tracks(&tracks, &QualityFilter::Format("FLAC".into()), "").len(), 1);
        assert_eq!(filter_tracks(&tracks, &QualityFilter::All, "echo").len(), 1);
    }

    #[test]
    fn renders_in_headless_frame_for_every_view_mode() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let tracks = vec![
            track("Dawn", "Nova", "Horizon", 24, 96_000, "FLAC"),
            track("Dusk", "Nova", "Horizon", 24, 96_000, "FLAC"),
        ];

        for mode in HiResViewMode::ALL {
            let mut state = HiResViewState { mode, ..Default::default() };
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render(ui, &tracks, &mut state);
                });
            });
        }
    }
}
