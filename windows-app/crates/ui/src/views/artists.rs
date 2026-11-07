//! `ArtistsView` (spec task 14.5), ported from `ArtistsView.swift`: an artist grid with
//! (cache-backed) avatars, search, album/track drill-down, and context menus (R5.4).
//!
//! Avatars are placeholder initial-monogram circles, same caveat as `albums.rs`'s cover tiles:
//! bridging `artist_art_cache.rs`'s (task 12.3) cached bytes to an egui texture is a separate
//! concern this task doesn't own.

use eframe::egui;

use walkman_domain::models::{ArtistItem, Track};

use crate::theme::{button, color, font, glass, sizing};

/// Groups `tracks` into [`ArtistItem`]s by artist name (case-insensitive), each carrying its
/// own album grouping (reusing `albums::group_into_albums` per-artist).
pub fn group_into_artists(tracks: &[Track]) -> Vec<ArtistItem> {
    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<String, Vec<Track>> = BTreeMap::new();
    for t in tracks {
        buckets.entry(t.artist.to_lowercase()).or_default().push(t.clone());
    }
    let mut artists: Vec<ArtistItem> = buckets
        .into_values()
        .map(|artist_tracks| {
            let name = artist_tracks[0].artist.clone();
            let albums = crate::views::albums::group_into_albums(&artist_tracks);
            let album_count = albums.len();
            let track_count = artist_tracks.len();
            ArtistItem::new(name, album_count, track_count, albums, artist_tracks)
        })
        .collect();
    artists.sort_by_key(|a| a.name.to_lowercase());
    artists
}

pub fn filter_artists<'a>(artists: &'a [ArtistItem], query: &str) -> Vec<&'a ArtistItem> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return artists.iter().collect();
    }
    artists.iter().filter(|a| a.name.to_lowercase().contains(&query)).collect()
}

/// Which level of the artist → albums → tracks drill-down is active.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ArtistDrillDown {
    #[default]
    Grid,
    Albums { artist: String },
    Tracks { artist: String, album_id: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArtistsCommand {
    Play(String),
    EditMetadata(String),
    TrimCut(String),
    Transfer(Vec<String>),
    Delete(Vec<String>),
}

#[derive(Debug, Default)]
pub struct ArtistsViewState {
    pub search_query: String,
    pub drill_down: ArtistDrillDown,
    pub commands: Vec<ArtistsCommand>,
}

pub fn render(ui: &mut egui::Ui, tracks: &[Track], state: &mut ArtistsViewState) {
    let artists = group_into_artists(tracks);

    match state.drill_down.clone() {
        ArtistDrillDown::Grid => render_grid(ui, &artists, state),
        ArtistDrillDown::Albums { artist } => {
            if let Some(found) = artists.iter().find(|a| a.name == artist) {
                render_albums(ui, found, state);
            } else {
                state.drill_down = ArtistDrillDown::Grid;
            }
        }
        ArtistDrillDown::Tracks { artist, album_id } => {
            if let Some(found) = artists.iter().find(|a| a.name == artist) {
                if let Some(album) = found.albums.iter().find(|a| a.id() == album_id) {
                    render_tracks(ui, &artist, album, state);
                    return;
                }
            }
            state.drill_down = ArtistDrillDown::Grid;
        }
    }
}

fn render_grid(ui: &mut egui::Ui, artists: &[ArtistItem], state: &mut ArtistsViewState) {
    let frame = glass::liquid_glass(glass::State::Normal);
    frame.show(ui, |ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.search_query)
                .hint_text("Search Artists")
                .desired_width(220.0)
                .font(font::body()),
        );
    });
    ui.add_space(8.0);

    let filtered = filter_artists(artists, &state.search_query);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for artist in &filtered {
                render_avatar_tile(ui, artist, state);
            }
        });
        ui.add_space(sizing::TABLE_BOTTOM_INSET);
    });
}

const AVATAR_SIZE: f32 = 96.0;

fn render_avatar_tile(ui: &mut egui::Ui, artist: &ArtistItem, state: &mut ArtistsViewState) {
    let response = ui
        .vertical(|ui| {
            ui.set_width(AVATAR_SIZE + 16.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(AVATAR_SIZE, AVATAR_SIZE), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), AVATAR_SIZE / 2.0, color::INPUT);
            let initial = artist.name.chars().next().unwrap_or('?').to_uppercase().to_string();
            let galley = ui.painter().layout_no_wrap(initial, font::title1(), color::SILVER_MUTED);
            let pos = rect.center() - galley.size() * 0.5;
            ui.painter().galley(pos, galley, color::SILVER_MUTED);

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&artist.name).font(font::body_medium()).color(color::SILVER_LIGHT));
                if artist.is_hi_res {
                    crate::theme::widgets::hi_res_badge(ui);
                }
            });
            ui.label(
                egui::RichText::new(format!("{} albums", artist.album_count))
                    .font(font::caption())
                    .color(color::SILVER_MUTED),
            );
        })
        .response
        .interact(egui::Sense::click());

    if response.clicked() {
        state.drill_down = ArtistDrillDown::Albums { artist: artist.name.clone() };
    }

    let name = artist.name.clone();
    let paths: Vec<String> = artist.tracks.iter().map(|t| t.full_path.clone()).collect();
    response.context_menu(|ui| {
        if ui.button("Open Artist").clicked() {
            state.drill_down = ArtistDrillDown::Albums { artist: name.clone() };
            ui.close_menu();
        }
        if ui.button("Transfer All Tracks\u{2026}").clicked() {
            state.commands.push(ArtistsCommand::Transfer(paths.clone()));
            ui.close_menu();
        }
    });
}

fn render_albums(ui: &mut egui::Ui, artist: &ArtistItem, state: &mut ArtistsViewState) {
    ui.horizontal(|ui| {
        if ui.add(button::Button::new("\u{2190} Back to Artists", button::Variant::Glass)).clicked() {
            state.drill_down = ArtistDrillDown::Grid;
        }
    });
    ui.add_space(8.0);
    ui.label(egui::RichText::new(&artist.name).font(font::title2()).color(color::SILVER_LIGHT));
    ui.add_space(12.0);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for album in &artist.albums {
            let frame = glass::liquid_glass(glass::State::Normal);
            let response = frame
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&album.name).font(font::body_medium()).color(color::SILVER_LIGHT));
                        ui.label(
                            egui::RichText::new(format!("{} tracks", album.track_count))
                                .font(font::caption())
                                .color(color::SILVER_MUTED),
                        );
                    });
                })
                .response
                .interact(egui::Sense::click());

            if response.clicked() {
                state.drill_down = ArtistDrillDown::Tracks { artist: artist.name.clone(), album_id: album.id() };
            }
            ui.add_space(2.0);
        }
        ui.add_space(sizing::TABLE_BOTTOM_INSET);
    });
}

fn render_tracks(ui: &mut egui::Ui, artist: &str, album: &walkman_domain::models::AlbumItem, state: &mut ArtistsViewState) {
    ui.horizontal(|ui| {
        if ui.add(button::Button::new("\u{2190} Back to Albums", button::Variant::Glass)).clicked() {
            state.drill_down = ArtistDrillDown::Albums { artist: artist.to_string() };
        }
    });
    ui.add_space(8.0);
    ui.label(egui::RichText::new(&album.name).font(font::title2()).color(color::SILVER_LIGHT));
    ui.add_space(12.0);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for track in &album.tracks {
            let frame = glass::liquid_glass(glass::State::Normal);
            let response = frame
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(egui::RichText::new(&track.title).font(font::body_medium()).color(color::SILVER_LIGHT));
                })
                .response
                .interact(egui::Sense::click());

            let path = track.full_path.clone();
            response.context_menu(|ui| {
                if ui.button("Play").clicked() {
                    state.commands.push(ArtistsCommand::Play(path.clone()));
                    ui.close_menu();
                }
                if ui.button("Edit Metadata\u{2026}").clicked() {
                    state.commands.push(ArtistsCommand::EditMetadata(path.clone()));
                    ui.close_menu();
                }
                if ui.button("Trim & Cut\u{2026}").clicked() {
                    state.commands.push(ArtistsCommand::TrimCut(path.clone()));
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Delete").clicked() {
                    state.commands.push(ArtistsCommand::Delete(vec![path.clone()]));
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
    use chrono::Utc;

    fn track(title: &str, artist: &str, album: &str) -> Track {
        Track {
            filename: format!("{title}.flac"),
            relative_path: format!("{artist}/{album}/{title}.flac"),
            full_path: format!("/music/{artist}/{album}/{title}.flac"),
            format: "FLAC".into(),
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            track_num: "1".into(),
            year: "2024".into(),
            genre: String::new(),
            duration_seconds: 180.0,
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
    fn group_into_artists_groups_by_name_and_sorts_alphabetically() {
        let tracks = [
            track("A", "Zeta", "Al1"),
            track("B", "Alpha", "Al2"),
            track("C", "Alpha", "Al2"),
        ];
        let artists = group_into_artists(&tracks);
        assert_eq!(artists.len(), 2);
        assert_eq!(artists[0].name, "Alpha");
        assert_eq!(artists[0].track_count, 2);
        assert_eq!(artists[0].album_count, 1);
        assert_eq!(artists[1].name, "Zeta");
    }

    #[test]
    fn filter_artists_matches_case_insensitively() {
        let tracks = [track("A", "Nova", "Al1"), track("B", "Echo", "Al2")];
        let artists = group_into_artists(&tracks);
        assert_eq!(filter_artists(&artists, "nova").len(), 1);
        assert_eq!(filter_artists(&artists, "").len(), 2);
    }

    #[test]
    fn renders_in_headless_frame_for_every_drill_down_level() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let tracks = vec![track("Dawn", "Nova", "Horizon"), track("Dusk", "Nova", "Horizon")];
        let artists = group_into_artists(&tracks);
        let album_id = artists[0].albums[0].id();

        for drill_down in [
            ArtistDrillDown::Grid,
            ArtistDrillDown::Albums { artist: "Nova".to_string() },
            ArtistDrillDown::Tracks { artist: "Nova".to_string(), album_id },
        ] {
            let mut state = ArtistsViewState { drill_down, ..Default::default() };
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render(ui, &tracks, &mut state);
                });
            });
        }
    }
}
