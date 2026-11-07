//! `AlbumsView` (spec task 14.4), ported from `AlbumsView.swift`: a cover-art grid with 7 sort
//! options, format filter chips (+ a Hi-Res flag), an expandable search bar, album detail
//! drill-down, album/track context menus, and a batch-lyrics button (R5.3).
//!
//! No real cover-art texture loading is wired up yet (that needs `cover_art_cache.rs`, task
//! 12.3, bridged to an egui texture — a separate concern this task doesn't own); each grid tile
//! is a placeholder glass tile with the album title/artist overlaid instead of artwork.

use eframe::egui;

use walkman_domain::models::{AlbumItem, Track};

use crate::selection;
use crate::theme::{button, color, font, glass, sizing};

/// R5.3's seven sort options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlbumSortOption {
    #[default]
    RecentlyAdded,
    NameAz,
    NameZa,
    ArtistAz,
    ReleaseYearNewest,
    ReleaseYearOldest,
    TrackCountMost,
}

impl AlbumSortOption {
    pub const ALL: [AlbumSortOption; 7] = [
        Self::RecentlyAdded,
        Self::NameAz,
        Self::NameZa,
        Self::ArtistAz,
        Self::ReleaseYearNewest,
        Self::ReleaseYearOldest,
        Self::TrackCountMost,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::RecentlyAdded => "Recently Added",
            Self::NameAz => "Album Name A\u{2013}Z",
            Self::NameZa => "Album Name Z\u{2013}A",
            Self::ArtistAz => "Artist A\u{2013}Z",
            Self::ReleaseYearNewest => "Release Year (Newest)",
            Self::ReleaseYearOldest => "Release Year (Oldest)",
            Self::TrackCountMost => "Track Count (Most)",
        }
    }
}

/// Groups `tracks` into [`AlbumItem`]s by `(name, artist)`, matching the Swift grouping
/// (`AlbumItem.id` is `"{name}|{artist}"` lowercased).
pub fn group_into_albums(tracks: &[Track]) -> Vec<AlbumItem> {
    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<(String, String), Vec<Track>> = BTreeMap::new();
    for t in tracks {
        buckets.entry((t.album.to_lowercase(), t.artist.to_lowercase())).or_default().push(t.clone());
    }
    buckets
        .into_values()
        .map(|group| {
            let first = &group[0];
            AlbumItem::new(first.album.clone(), first.artist.clone(), first.year.clone(), group.len(), group)
        })
        .collect()
}

pub fn sort_albums(albums: &mut [AlbumItem], option: AlbumSortOption) {
    match option {
        AlbumSortOption::RecentlyAdded => albums.sort_by_key(|a| std::cmp::Reverse(a.latest_date_added)),
        AlbumSortOption::NameAz => albums.sort_by_key(|a| a.name.to_lowercase()),
        AlbumSortOption::NameZa => albums.sort_by_key(|a| std::cmp::Reverse(a.name.to_lowercase())),
        AlbumSortOption::ArtistAz => albums.sort_by_key(|a| a.artist.to_lowercase()),
        AlbumSortOption::ReleaseYearNewest => albums.sort_by_key(|a| std::cmp::Reverse(a.year.clone())),
        AlbumSortOption::ReleaseYearOldest => albums.sort_by_key(|a| a.year.clone()),
        AlbumSortOption::TrackCountMost => albums.sort_by_key(|a| std::cmp::Reverse(a.track_count)),
    }
}

/// Which format chip is active; `All` shows every album regardless of format.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum FormatFilter {
    #[default]
    All,
    Format(String),
}

/// Applies the format chip + Hi-Res-only toggle + search query to `albums`.
pub fn filter_albums<'a>(albums: &'a [AlbumItem], format: &FormatFilter, hi_res_only: bool, query: &str) -> Vec<&'a AlbumItem> {
    let query = query.trim().to_lowercase();
    albums
        .iter()
        .filter(|a| match format {
            FormatFilter::All => true,
            FormatFilter::Format(f) => a.primary_format().eq_ignore_ascii_case(f),
        })
        .filter(|a| !hi_res_only || a.is_hi_res)
        .filter(|a| query.is_empty() || a.name.to_lowercase().contains(&query) || a.artist.to_lowercase().contains(&query))
        .collect()
}

/// The distinct formats present across `albums`, for the format filter chips.
pub fn distinct_formats(albums: &[AlbumItem]) -> Vec<String> {
    let mut formats: Vec<String> = albums.iter().map(|a| a.primary_format()).collect();
    formats.sort();
    formats.dedup();
    formats
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlbumsCommand {
    Play(String),
    EditAlbumMetadata(String),
    EditTrackMetadata(String),
    TrimCut(String),
    Transfer(Vec<String>),
    Delete(Vec<String>),
    BatchLyrics(Vec<String>),
}

#[derive(Debug, Default)]
pub struct AlbumsViewState {
    pub sort: AlbumSortOption,
    pub format_filter: FormatFilter,
    pub hi_res_only: bool,
    pub search_expanded: bool,
    pub search_query: String,
    /// The album id (`AlbumItem::id()`) currently drilled into, if any.
    pub drilled_into: Option<String>,
    pub selected: Vec<String>,
    pub selection_anchor: Option<usize>,
    pub commands: Vec<AlbumsCommand>,
}

pub fn render(ui: &mut egui::Ui, tracks: &[Track], state: &mut AlbumsViewState) {
    let mut albums = group_into_albums(tracks);
    sort_albums(&mut albums, state.sort);

    if let Some(album_id) = state.drilled_into.clone() {
        if let Some(album) = albums.iter().find(|a| a.id() == album_id) {
            render_album_detail(ui, album, state);
            return;
        }
        state.drilled_into = None; // the album vanished (e.g. deleted); fall back to the grid.
    }

    render_toolbar(ui, &albums, state);
    ui.add_space(8.0);

    let filtered = filter_albums(&albums, &state.format_filter, state.hi_res_only, &state.search_query);
    render_grid(ui, &filtered, state);
}

fn render_toolbar(ui: &mut egui::Ui, albums: &[AlbumItem], state: &mut AlbumsViewState) {
    ui.horizontal(|ui| {
        let all_variant = if state.format_filter == FormatFilter::All { button::Variant::Capsule } else { button::Variant::Glass };
        if ui.add(button::Button::new("ALL", all_variant)).clicked() {
            state.format_filter = FormatFilter::All;
        }
        for format in distinct_formats(albums) {
            let active = state.format_filter == FormatFilter::Format(format.clone());
            let variant = if active { button::Variant::Capsule } else { button::Variant::Glass };
            if ui.add(button::Button::new(&format, variant)).clicked() {
                state.format_filter = FormatFilter::Format(format.clone());
            }
        }
        let hi_res_variant = if state.hi_res_only { button::Variant::Gold } else { button::Variant::Glass };
        if ui.add(button::Button::new("Hi-Res", hi_res_variant)).clicked() {
            state.hi_res_only = !state.hi_res_only;
        }

        ui.add_space(12.0);
        egui::ComboBox::from_id_salt("albums_sort")
            .selected_text(state.sort.title())
            .show_ui(ui, |ui| {
                for option in AlbumSortOption::ALL {
                    ui.selectable_value(&mut state.sort, option, option.title());
                }
            });

        ui.add_space(12.0);
        if state.search_expanded {
            let frame = glass::liquid_glass(glass::State::Normal);
            frame.show(ui, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.search_query)
                        .hint_text("Search Albums")
                        .desired_width(200.0)
                        .font(font::body()),
                );
            });
            if ui.add(button::Button::new("\u{2715}", button::Variant::Glass)).clicked() {
                state.search_expanded = false;
                state.search_query.clear();
            }
        } else if ui.add(button::Button::new("Search\u{2026}", button::Variant::Glass)).clicked() {
            state.search_expanded = true;
        }

        ui.add_space(12.0);
        let has_selection = !state.selected.is_empty();
        if ui.add(button::Button::new("Find Lyrics\u{2026}", button::Variant::Glass).enabled(has_selection)).clicked() {
            state.commands.push(AlbumsCommand::BatchLyrics(state.selected.clone()));
        }
    });
}

const TILE_SIZE: f32 = 160.0;

fn render_grid(ui: &mut egui::Ui, albums: &[&AlbumItem], state: &mut AlbumsViewState) {
    let all_ids: Vec<String> = albums.iter().map(|a| a.id()).collect();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for (index, album) in albums.iter().enumerate() {
                render_tile(ui, index, album, &all_ids, state);
            }
        });
        ui.add_space(sizing::TABLE_BOTTOM_INSET);
    });
}

fn render_tile(ui: &mut egui::Ui, index: usize, album: &AlbumItem, all_ids: &[String], state: &mut AlbumsViewState) {
    let id = album.id();
    let selected = state.selected.contains(&id);
    let glass_state = if selected { glass::State::Selected } else { glass::State::Normal };
    let frame = glass::liquid_glass(glass_state);

    let response = frame
        .show(ui, |ui| {
            ui.set_width(TILE_SIZE);
            ui.vertical(|ui| {
                let (cover_rect, _) = ui.allocate_exact_size(egui::vec2(TILE_SIZE - 16.0, TILE_SIZE - 16.0), egui::Sense::hover());
                ui.painter().rect_filled(cover_rect, 6.0, color::INPUT);
                let initial = album.name.chars().next().unwrap_or('?').to_uppercase().to_string();
                let galley = ui.painter().layout_no_wrap(initial, font::title1(), color::SILVER_MUTED);
                let pos = cover_rect.center() - galley.size() * 0.5;
                ui.painter().galley(pos, galley, color::SILVER_MUTED);

                ui.label(egui::RichText::new(&album.name).font(font::body_medium()).color(color::SILVER_LIGHT));
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&album.artist).font(font::caption()).color(color::SILVER_MUTED));
                    if album.is_hi_res {
                        crate::theme::widgets::hi_res_badge(ui);
                    }
                });
            });
        })
        .response
        .interact(egui::Sense::click());

    if response.clicked() {
        let (shift, ctrl) = ui.input(|i| (i.modifiers.shift, i.modifiers.command || i.modifiers.ctrl));
        selection::handle_click(&mut state.selected, &mut state.selection_anchor, all_ids, index, &id, shift, ctrl);
    }
    if response.double_clicked() {
        state.drilled_into = Some(id.clone());
    }

    response.context_menu(|ui| {
        if ui.button("Open Album").clicked() {
            state.drilled_into = Some(id.clone());
            ui.close_menu();
        }
        if ui.button("Edit Album Metadata\u{2026}").clicked() {
            state.commands.push(AlbumsCommand::EditAlbumMetadata(id.clone()));
            ui.close_menu();
        }
        if ui.button("Transfer Album\u{2026}").clicked() {
            let paths: Vec<String> = album.tracks.iter().map(|t| t.full_path.clone()).collect();
            state.commands.push(AlbumsCommand::Transfer(paths));
            ui.close_menu();
        }
    });
}

fn render_album_detail(ui: &mut egui::Ui, album: &AlbumItem, state: &mut AlbumsViewState) {
    ui.horizontal(|ui| {
        if ui.add(button::Button::new("\u{2190} Back to Albums", button::Variant::Glass)).clicked() {
            state.drilled_into = None;
        }
    });
    ui.add_space(8.0);
    ui.label(egui::RichText::new(&album.name).font(font::title2()).color(color::SILVER_LIGHT));
    ui.label(egui::RichText::new(&album.artist).font(font::callout()).color(color::SILVER_MUTED));
    ui.add_space(12.0);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for track in &album.tracks {
            let frame = glass::liquid_glass(glass::State::Normal);
            let response = frame
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&track.track_num).font(font::mono()).color(color::SILVER_MUTED));
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(&track.title).font(font::body_medium()).color(color::SILVER_LIGHT));
                    });
                })
                .response
                .interact(egui::Sense::click());

            let path = track.full_path.clone();
            response.context_menu(|ui| {
                if ui.button("Play").clicked() {
                    state.commands.push(AlbumsCommand::Play(path.clone()));
                    ui.close_menu();
                }
                if ui.button("Edit Metadata\u{2026}").clicked() {
                    state.commands.push(AlbumsCommand::EditTrackMetadata(path.clone()));
                    ui.close_menu();
                }
                if ui.button("Trim & Cut\u{2026}").clicked() {
                    state.commands.push(AlbumsCommand::TrimCut(path.clone()));
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Delete").clicked() {
                    state.commands.push(AlbumsCommand::Delete(vec![path.clone()]));
                    ui.close_menu();
                }
            });
            ui.add_space(2.0);
        }
        ui.add_space(sizing::TABLE_BOTTOM_INSET);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn track(title: &str, artist: &str, album: &str, year: &str, format: &str, bits: i32, rate: i32) -> Track {
        Track {
            filename: format!("{title}.flac"),
            relative_path: format!("{artist}/{album}/{title}.flac"),
            full_path: format!("/music/{artist}/{album}/{title}.flac"),
            format: format.into(),
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            track_num: "1".into(),
            year: year.into(),
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
    fn group_into_albums_groups_by_name_and_artist() {
        let tracks = [
            track("A", "Nova", "Horizon", "2024", "FLAC", 16, 44_100),
            track("B", "Nova", "Horizon", "2024", "FLAC", 16, 44_100),
            track("C", "Echo", "Reverb", "2021", "MP3", 16, 44_100),
        ];
        let albums = group_into_albums(&tracks);
        assert_eq!(albums.len(), 2);
        let horizon = albums.iter().find(|a| a.name == "Horizon").unwrap();
        assert_eq!(horizon.track_count, 2);
    }

    #[test]
    fn sort_by_name_az_za_and_track_count() {
        let tracks = [
            track("A", "X", "Zeta", "2024", "FLAC", 16, 44_100),
            track("B", "X", "Alpha", "2024", "FLAC", 16, 44_100),
            track("C", "X", "Alpha", "2024", "FLAC", 16, 44_100),
        ];
        let mut albums = group_into_albums(&tracks);
        sort_albums(&mut albums, AlbumSortOption::NameAz);
        assert_eq!(albums[0].name, "Alpha");
        sort_albums(&mut albums, AlbumSortOption::NameZa);
        assert_eq!(albums[0].name, "Zeta");
        sort_albums(&mut albums, AlbumSortOption::TrackCountMost);
        assert_eq!(albums[0].name, "Alpha");
        assert_eq!(albums[0].track_count, 2);
    }

    #[test]
    fn sort_by_release_year_newest_and_oldest() {
        let tracks = [
            track("A", "X", "Old", "2000", "FLAC", 16, 44_100),
            track("B", "X", "New", "2024", "FLAC", 16, 44_100),
        ];
        let mut albums = group_into_albums(&tracks);
        sort_albums(&mut albums, AlbumSortOption::ReleaseYearNewest);
        assert_eq!(albums[0].name, "New");
        sort_albums(&mut albums, AlbumSortOption::ReleaseYearOldest);
        assert_eq!(albums[0].name, "Old");
    }

    #[test]
    fn sort_by_recently_added_uses_latest_date_added() {
        let mut tracks = [
            track("A", "X", "Old", "2020", "FLAC", 16, 44_100),
            track("B", "X", "New", "2020", "FLAC", 16, 44_100),
        ];
        tracks[0].date_added = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        tracks[1].date_added = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let mut albums = group_into_albums(&tracks);
        sort_albums(&mut albums, AlbumSortOption::RecentlyAdded);
        assert_eq!(albums[0].name, "New");
    }

    #[test]
    fn filter_by_format_hi_res_and_search() {
        let tracks = [
            track("A", "Nova", "Horizon", "2024", "FLAC", 24, 96_000),
            track("B", "Echo", "Reverb", "2021", "MP3", 16, 44_100),
        ];
        let albums = group_into_albums(&tracks);

        let all = filter_albums(&albums, &FormatFilter::All, false, "");
        assert_eq!(all.len(), 2);

        let flac_only = filter_albums(&albums, &FormatFilter::Format("FLAC".into()), false, "");
        assert_eq!(flac_only.len(), 1);
        assert_eq!(flac_only[0].name, "Horizon");

        let hi_res_only = filter_albums(&albums, &FormatFilter::All, true, "");
        assert_eq!(hi_res_only.len(), 1);
        assert_eq!(hi_res_only[0].name, "Horizon");

        let searched = filter_albums(&albums, &FormatFilter::All, false, "reverb");
        assert_eq!(searched.len(), 1);
        assert_eq!(searched[0].name, "Reverb");
    }

    #[test]
    fn distinct_formats_is_sorted_and_deduplicated() {
        let tracks = [
            track("A", "X", "Al1", "2024", "mp3", 16, 44_100),
            track("B", "X", "Al2", "2024", "FLAC", 16, 44_100),
            track("C", "X", "Al3", "2024", "FLAC", 16, 44_100),
        ];
        let albums = group_into_albums(&tracks);
        assert_eq!(distinct_formats(&albums), vec!["FLAC".to_string(), "mp3".to_string()]);
    }

    #[test]
    fn renders_in_headless_frame_for_grid_and_drill_down() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let tracks = vec![
            track("Dawn", "Nova", "Horizon", "2024", "FLAC", 24, 96_000),
            track("Dusk", "Nova", "Horizon", "2024", "FLAC", 24, 96_000),
        ];

        let mut grid_state = AlbumsViewState { search_expanded: true, ..Default::default() };
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render(ui, &tracks, &mut grid_state);
            });
        });

        let albums = group_into_albums(&tracks);
        let mut detail_state = AlbumsViewState { drilled_into: Some(albums[0].id()), ..Default::default() };
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render(ui, &tracks, &mut detail_state);
            });
        });
    }
}
