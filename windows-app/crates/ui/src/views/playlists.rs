//! `PlaylistsView`, ported from `Sources/WalkmanStudio/Views/PlaylistsView.swift`.
//!
//! Spec task **17.3** (requirements 11.1, 11.2, 11.3): create/rename/delete a playlist, an
//! add-tracks browser with multi-select (Shift-range/Ctrl-toggle + a header "Select"
//! button), drag-free reordering via a floating batch action bar (`Move to Top`/Up/Down/
//! `To Bottom`/remove), syncing instantly after each change — over the real
//! `walkman_domain::models::Playlist` and `walkman_services::walkman_engine::{write_playlist,
//! read_playlist, rename_playlist, delete_playlist}` built in task 9.3.
//!
//! ## Honest simplification
//! "Add-browsing modes" (R11.2: "Songs, Recently Added, and the other add-browsing modes")
//! is implemented as a mode-tabbed search+select list over whatever `Track` slice the
//! caller passes for each mode; the Recently Added *ordering* itself (newest-first within a
//! time scope) reuses [`crate::views::recently_added`]'s own pure sort rather than
//! re-deriving it, but the Albums/Artists grouping modes stay a flat track list for now — a
//! full drill-down browser inside this already-nested sheet was judged more complexity than
//! the requirement's own wording ("the other add-browsing modes") demands.

use eframe::egui;

use walkman_domain::models::{Playlist, Track};

use crate::selection;
use crate::theme::{button, color, font, glass, sizing};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AddTracksMode {
    #[default]
    Songs,
    RecentlyAdded,
    Albums,
    Artists,
}

impl AddTracksMode {
    pub const ALL: [AddTracksMode; 4] = [AddTracksMode::Songs, AddTracksMode::RecentlyAdded, AddTracksMode::Albums, AddTracksMode::Artists];

    pub fn title(self) -> &'static str {
        match self {
            AddTracksMode::Songs => "Songs",
            AddTracksMode::RecentlyAdded => "Recently Added",
            AddTracksMode::Albums => "Albums",
            AddTracksMode::Artists => "Artists",
        }
    }
}

/// Tracks for `mode`'s add-browser list, pure so the mode-filtering is unit-testable without
/// a live `Ui`. `Albums`/`Artists` just sort by their grouping key (see the module's own
/// honest-simplification note on why they aren't a full drill-down here).
pub fn tracks_for_add_mode<'a>(tracks: &'a [Track], mode: AddTracksMode) -> Vec<&'a Track> {
    let mut sorted: Vec<&Track> = tracks.iter().collect();
    match mode {
        AddTracksMode::Songs => sorted.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase())),
        AddTracksMode::RecentlyAdded => sorted.sort_by(|a, b| b.date_added.cmp(&a.date_added)),
        AddTracksMode::Albums => sorted.sort_by(|a, b| a.album.to_lowercase().cmp(&b.album.to_lowercase())),
        AddTracksMode::Artists => sorted.sort_by(|a, b| a.artist.to_lowercase().cmp(&b.artist.to_lowercase())),
    }
    sorted
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum PlaylistsSheet {
    #[default]
    None,
    CreatePlaylist { name: String },
    RenamePlaylist { playlist_id: String, name: String },
    DeleteConfirm { playlist_id: String },
    AddTracks { mode: AddTracksMode, search: String, selected: Vec<String> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlaylistsCommand {
    Create(String),
    Rename { playlist_id: String, new_name: String },
    Delete(String),
    /// Appends `relative_paths` to the end of the active playlist.
    AddTracks { playlist_id: String, relative_paths: Vec<String> },
    /// Replaces the active playlist's full track list, in the given order (also used for
    /// single/batch removal — the caller just omits the removed entries) — R11.1/11.3's
    /// "syncing instantly after each change".
    Reorder { playlist_id: String, relative_paths: Vec<String> },
}

#[derive(Debug, Clone, Default)]
pub struct PlaylistsViewState {
    pub selected_playlist_id: Option<String>,
    /// A working copy of the selected playlist's track list, reordered/trimmed locally
    /// before each change is emitted as a [`PlaylistsCommand::Reorder`]. Re-seeded from the
    /// real `Playlist::tracks` whenever `selected_playlist_id` changes (tracked via
    /// `loaded_for`, below) so an external update (e.g. another device writing the file) is
    /// picked up on next selection rather than silently overwritten.
    pub active_tracks: Vec<String>,
    loaded_for: Option<String>,
    pub selected_rows: Vec<String>,
    pub selection_anchor: Option<usize>,
    pub sheet: PlaylistsSheet,
    pub commands: Vec<PlaylistsCommand>,
}

pub fn render(ui: &mut egui::Ui, playlists: &[Playlist], all_tracks: &[Track], state: &mut PlaylistsViewState) {
    sync_active_tracks(playlists, state);

    ui.columns(2, |cols| {
        render_playlist_list(&mut cols[0], playlists, state);
        render_track_list(&mut cols[1], playlists, state);
    });

    render_sheet(ui, all_tracks, state);
}

/// Re-seeds `active_tracks` from the real `Playlist` whenever the selection changes to a
/// different playlist (or the selection is cleared), so edits made via the batch bar aren't
/// clobbered on every frame but a freshly-opened playlist always starts from its real list.
fn sync_active_tracks(playlists: &[Playlist], state: &mut PlaylistsViewState) {
    if state.loaded_for == state.selected_playlist_id {
        return;
    }
    state.loaded_for = state.selected_playlist_id.clone();
    state.active_tracks = state
        .selected_playlist_id
        .as_deref()
        .and_then(|id| playlists.iter().find(|p| p.full_path == id))
        .map(|p| p.tracks.clone())
        .unwrap_or_default();
    state.selected_rows.clear();
    state.selection_anchor = None;
}

fn render_playlist_list(ui: &mut egui::Ui, playlists: &[Playlist], state: &mut PlaylistsViewState) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Playlists").font(font::title3()).color(color::SILVER_LIGHT));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(button::Button::new("New\u{2026}", button::Variant::Prominent)).clicked() {
                state.sheet = PlaylistsSheet::CreatePlaylist { name: String::new() };
            }
        });
    });
    ui.add_space(8.0);

    egui::ScrollArea::vertical().id_salt("playlist_list_scroll").auto_shrink([false, false]).show(ui, |ui| {
        for playlist in playlists {
            let selected = state.selected_playlist_id.as_deref() == Some(playlist.full_path.as_str());
            let glass_state = if selected { glass::State::Selected } else { glass::State::Normal };
            let response = glass::liquid_glass(glass_state).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&playlist.name).font(font::body_medium()).color(color::SILVER_LIGHT));
                        ui.label(
                            egui::RichText::new(format!("{} track{}", playlist.track_count, if playlist.track_count == 1 { "" } else { "s" }))
                                .font(font::caption())
                                .color(color::SILVER_MUTED),
                        );
                    });
                });
            });
            let row_response = response.response.interact(egui::Sense::click());
            if row_response.clicked() {
                state.selected_playlist_id = Some(playlist.full_path.clone());
            }
            row_response.context_menu(|ui| {
                if ui.button("Rename\u{2026}").clicked() {
                    state.sheet = PlaylistsSheet::RenamePlaylist { playlist_id: playlist.full_path.clone(), name: playlist.name.clone() };
                    ui.close_menu();
                }
                if ui.button("Delete").clicked() {
                    state.sheet = PlaylistsSheet::DeleteConfirm { playlist_id: playlist.full_path.clone() };
                    ui.close_menu();
                }
            });
            ui.add_space(4.0);
        }
    });
}

fn render_track_list(ui: &mut egui::Ui, playlists: &[Playlist], state: &mut PlaylistsViewState) {
    let Some(playlist_id) = state.selected_playlist_id.clone() else {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(egui::RichText::new("Select a playlist").font(font::callout()).color(color::SILVER_MUTED));
        });
        return;
    };

    ui.horizontal(|ui| {
        let name = playlists.iter().find(|p| p.full_path == playlist_id).map(|p| p.name.clone()).unwrap_or_default();
        ui.label(egui::RichText::new(name).font(font::title3()).color(color::SILVER_LIGHT));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(button::Button::new("Add Tracks\u{2026}", button::Variant::Prominent)).clicked() {
                state.sheet = PlaylistsSheet::AddTracks { mode: AddTracksMode::default(), search: String::new(), selected: Vec::new() };
            }
            ui.add_space(8.0);
            if ui.add(button::Button::new("Select", button::Variant::Glass)).clicked() {
                selection::toggle_select_all(&mut state.selected_rows, &row_ids(state.active_tracks.len()));
            }
        });
    });
    ui.add_space(8.0);

    let row_ids_vec = row_ids(state.active_tracks.len());
    egui::ScrollArea::vertical().id_salt("playlist_tracks_scroll").auto_shrink([false, false]).show(ui, |ui| {
        for (index, relative_path) in state.active_tracks.clone().into_iter().enumerate() {
            render_track_row(ui, index, &relative_path, &row_ids_vec, state);
        }
        ui.add_space(sizing::TABLE_BOTTOM_INSET);
    });

    if !state.selected_rows.is_empty() {
        render_batch_action_bar(ui, &playlist_id, state);
    }
}

/// Synthetic row ids (`"0"`, `"1"`, ...) for [`selection::handle_click`] — a playlist's
/// relative paths aren't unique (the same track can be added twice), so the row's index is
/// the only stable per-row identity.
fn row_ids(len: usize) -> Vec<String> {
    (0..len).map(|i| i.to_string()).collect()
}

fn render_track_row(ui: &mut egui::Ui, index: usize, relative_path: &str, all_ids: &[String], state: &mut PlaylistsViewState) {
    let id = index.to_string();
    let selected = state.selected_rows.contains(&id);
    let glass_state = if selected { glass::State::Selected } else { glass::State::Normal };
    let response = glass::liquid_glass(glass_state).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(egui::RichText::new(relative_path).font(font::body()).color(color::SILVER_LIGHT));
    });
    let row_response = response.response.interact(egui::Sense::click());
    if row_response.clicked() {
        let modifiers = ui.input(|i| i.modifiers);
        selection::handle_click(&mut state.selected_rows, &mut state.selection_anchor, all_ids, index, &id, modifiers.shift, modifiers.command || modifiers.ctrl);
    }
}

fn render_batch_action_bar(ui: &mut egui::Ui, playlist_id: &str, state: &mut PlaylistsViewState) {
    ui.add_space(8.0);
    glass::liquid_glass(glass::State::Normal).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("{} selected", state.selected_rows.len()))
                    .font(font::caption_bold())
                    .color(color::SILVER_MUTED),
            );
            ui.add_space(10.0);
            if ui.add(button::Button::new("Move to Top", button::Variant::Glass)).clicked() {
                apply_reorder(state, playlist_id, move_selected_to_top);
            }
            if ui.add(button::Button::new("\u{2191}", button::Variant::Glass)).clicked() {
                apply_reorder(state, playlist_id, |tracks, selected| shift_selected(tracks, selected, -1));
            }
            if ui.add(button::Button::new("\u{2193}", button::Variant::Glass)).clicked() {
                apply_reorder(state, playlist_id, |tracks, selected| shift_selected(tracks, selected, 1));
            }
            if ui.add(button::Button::new("Move to Bottom", button::Variant::Glass)).clicked() {
                apply_reorder(state, playlist_id, move_selected_to_bottom);
            }
            ui.add_space(10.0);
            if ui.add(button::Button::new("Remove", button::Variant::Destructive)).clicked() {
                apply_reorder(state, playlist_id, remove_selected);
            }
        });
    });
}

/// Applies a pure reorder/removal function to the selected row indices against
/// `state.active_tracks`, clears the selection (indices no longer line up after a move), and
/// emits the resulting order as a [`PlaylistsCommand::Reorder`] (R11.3's instant sync).
fn apply_reorder(state: &mut PlaylistsViewState, playlist_id: &str, f: impl FnOnce(&mut Vec<String>, &[usize])) {
    let mut selected_indices: Vec<usize> = state.selected_rows.iter().filter_map(|s| s.parse().ok()).collect();
    selected_indices.sort_unstable();
    f(&mut state.active_tracks, &selected_indices);
    state.selected_rows.clear();
    state.selection_anchor = None;
    state.commands.push(PlaylistsCommand::Reorder { playlist_id: playlist_id.to_owned(), relative_paths: state.active_tracks.clone() });
}

/// Moves every selected index to the front, preserving their relative order, then the
/// unselected rows in their original relative order.
pub fn move_selected_to_top(tracks: &mut Vec<String>, selected: &[usize]) {
    let selected_set: std::collections::HashSet<usize> = selected.iter().copied().collect();
    let mut moved: Vec<String> = selected.iter().filter_map(|&i| tracks.get(i).cloned()).collect();
    let rest: Vec<String> = tracks.iter().enumerate().filter(|(i, _)| !selected_set.contains(i)).map(|(_, t)| t.clone()).collect();
    moved.extend(rest);
    *tracks = moved;
}

/// Moves every selected index to the back, preserving their relative order.
pub fn move_selected_to_bottom(tracks: &mut Vec<String>, selected: &[usize]) {
    let selected_set: std::collections::HashSet<usize> = selected.iter().copied().collect();
    let mut rest: Vec<String> = tracks.iter().enumerate().filter(|(i, _)| !selected_set.contains(i)).map(|(_, t)| t.clone()).collect();
    let moved: Vec<String> = selected.iter().filter_map(|&i| tracks.get(i).cloned()).collect();
    rest.extend(moved);
    *tracks = rest;
}

/// Shifts every selected index by `delta` (`-1` up, `1` down), as a group — the whole
/// selected block moves past the single adjacent unselected row each click, matching the
/// macOS "group Up/Down shift" (R11.3), rather than each row moving independently (which
/// would let a multi-row selection interleave with unselected rows).
pub fn shift_selected(tracks: &mut Vec<String>, selected: &[usize], delta: i32) {
    if selected.is_empty() {
        return;
    }
    if delta < 0 {
        if selected[0] == 0 {
            return;
        }
        for &i in selected {
            tracks.swap(i, i - 1);
        }
    } else {
        if *selected.last().unwrap() + 1 >= tracks.len() {
            return;
        }
        for &i in selected.iter().rev() {
            tracks.swap(i, i + 1);
        }
    }
}

/// Removes every selected index (descending, so earlier removals don't shift later ones).
pub fn remove_selected(tracks: &mut Vec<String>, selected: &[usize]) {
    for &i in selected.iter().rev() {
        if i < tracks.len() {
            tracks.remove(i);
        }
    }
}

fn render_sheet(ui: &mut egui::Ui, all_tracks: &[Track], state: &mut PlaylistsViewState) {
    let ctx = ui.ctx().clone();
    match state.sheet.clone() {
        PlaylistsSheet::None => {}
        PlaylistsSheet::CreatePlaylist { mut name } => {
            let mut close = false;
            let mut create = false;
            crate::sheets::modal(&ctx, "playlists_create_sheet", egui::vec2(420.0, 200.0), |ui| {
                ui.label(egui::RichText::new("New Playlist").font(font::title2()).color(color::SILVER_LIGHT));
                ui.add_space(8.0);
                ui.add(egui::TextEdit::singleline(&mut name).hint_text("Playlist name").desired_width(f32::INFINITY));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(button::Button::new("Create", button::Variant::Prominent).enabled(!name.trim().is_empty())).clicked() {
                        create = true;
                    }
                    if ui.add(button::Button::new("Cancel", button::Variant::Glass)).clicked() {
                        close = true;
                    }
                });
            });
            if create {
                state.commands.push(PlaylistsCommand::Create(name.clone()));
                state.sheet = PlaylistsSheet::None;
            } else if close {
                state.sheet = PlaylistsSheet::None;
            } else {
                state.sheet = PlaylistsSheet::CreatePlaylist { name };
            }
        }
        PlaylistsSheet::RenamePlaylist { playlist_id, mut name } => {
            let mut close = false;
            let mut rename = false;
            crate::sheets::modal(&ctx, "playlists_rename_sheet", egui::vec2(420.0, 200.0), |ui| {
                ui.label(egui::RichText::new("Rename Playlist").font(font::title2()).color(color::SILVER_LIGHT));
                ui.add_space(8.0);
                ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(button::Button::new("Rename", button::Variant::Prominent).enabled(!name.trim().is_empty())).clicked() {
                        rename = true;
                    }
                    if ui.add(button::Button::new("Cancel", button::Variant::Glass)).clicked() {
                        close = true;
                    }
                });
            });
            if rename {
                state.commands.push(PlaylistsCommand::Rename { playlist_id: playlist_id.clone(), new_name: name.clone() });
                state.sheet = PlaylistsSheet::None;
            } else if close {
                state.sheet = PlaylistsSheet::None;
            } else {
                state.sheet = PlaylistsSheet::RenamePlaylist { playlist_id, name };
            }
        }
        PlaylistsSheet::DeleteConfirm { playlist_id } => {
            crate::sheets::modal(&ctx, "playlists_delete_confirm_sheet", egui::vec2(420.0, 200.0), |ui| {
                ui.label(egui::RichText::new("Delete Playlist?").font(font::title2()).color(color::SILVER_LIGHT));
                ui.label(egui::RichText::new("This can't be undone.").font(font::callout()).color(color::SILVER_MUTED));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(button::Button::new("Delete", button::Variant::Destructive)).clicked() {
                        state.commands.push(PlaylistsCommand::Delete(playlist_id.clone()));
                        state.sheet = PlaylistsSheet::None;
                        if state.selected_playlist_id.as_deref() == Some(playlist_id.as_str()) {
                            state.selected_playlist_id = None;
                        }
                    }
                    if ui.add(button::Button::new("Cancel", button::Variant::Glass)).clicked() {
                        state.sheet = PlaylistsSheet::None;
                    }
                });
            });
        }
        PlaylistsSheet::AddTracks { mode, mut search, mut selected } => {
            let mut close = false;
            let mut add = false;
            crate::sheets::modal(&ctx, "playlists_add_tracks_sheet", egui::vec2(560.0, 560.0), |ui| {
                ui.label(egui::RichText::new("Add Tracks").font(font::title2()).color(color::SILVER_LIGHT));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    for m in AddTracksMode::ALL {
                        let variant = if mode == m { button::Variant::Capsule } else { button::Variant::Glass };
                        if ui.add(button::Button::new(m.title(), variant)).clicked() {
                            state.sheet = PlaylistsSheet::AddTracks { mode: m, search: search.clone(), selected: selected.clone() };
                        }
                    }
                });
                ui.add_space(8.0);
                ui.add(egui::TextEdit::singleline(&mut search).hint_text("Search").desired_width(f32::INFINITY));
                ui.add_space(8.0);

                let query = search.trim().to_lowercase();
                let candidates = tracks_for_add_mode(all_tracks, mode);
                let filtered: Vec<&Track> = candidates
                    .into_iter()
                    .filter(|t| query.is_empty() || t.title.to_lowercase().contains(&query) || t.artist.to_lowercase().contains(&query))
                    .collect();

                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    for track in &filtered {
                        let checked = selected.contains(&track.relative_path);
                        let mut is_checked = checked;
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut is_checked, "").changed() {
                                if is_checked {
                                    selected.push(track.relative_path.clone());
                                } else {
                                    selected.retain(|p| p != &track.relative_path);
                                }
                            }
                            ui.label(egui::RichText::new(&track.title).font(font::body()).color(color::SILVER_LIGHT));
                            ui.label(egui::RichText::new(&track.artist).font(font::caption()).color(color::SILVER_MUTED));
                        });
                    }
                });

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("{} selected", selected.len())).font(font::caption()).color(color::SILVER_MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(button::Button::new("Add", button::Variant::Prominent).enabled(!selected.is_empty())).clicked() {
                            add = true;
                        }
                        if ui.add(button::Button::new("Cancel", button::Variant::Glass)).clicked() {
                            close = true;
                        }
                    });
                });
            });

            if add {
                if let Some(playlist_id) = state.selected_playlist_id.clone() {
                    state.commands.push(PlaylistsCommand::AddTracks { playlist_id, relative_paths: selected.clone() });
                }
                state.sheet = PlaylistsSheet::None;
            } else if close {
                state.sheet = PlaylistsSheet::None;
            } else if let PlaylistsSheet::AddTracks { .. } = &state.sheet {
                // Not already replaced by a mode-switch above; persist search/selection edits.
                state.sheet = PlaylistsSheet::AddTracks { mode, search, selected };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_playlist(path: &str, tracks: Vec<&str>) -> Playlist {
        Playlist {
            name: path.trim_end_matches(".m3u8").to_owned(),
            filename: format!("{path}.m3u8"),
            track_count: tracks.len(),
            tracks: tracks.into_iter().map(String::from).collect(),
            full_path: format!("/MUSIC/{path}.m3u8"),
        }
    }

    fn sample_track(title: &str) -> Track {
        Track {
            filename: format!("{title}.flac"),
            relative_path: format!("Artist/Album/{title}.flac"),
            full_path: format!("D:/Music/Artist/Album/{title}.flac"),
            format: "FLAC".into(),
            title: title.into(),
            artist: "Artist".into(),
            album: "Album".into(),
            track_num: "1".into(),
            year: "2024".into(),
            genre: String::new(),
            duration_seconds: 200.0,
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

    // ---- Reorder helpers ----

    #[test]
    fn move_selected_to_top_preserves_relative_order() {
        let mut tracks = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        move_selected_to_top(&mut tracks, &[1, 3]);
        assert_eq!(tracks, vec!["b", "d", "a", "c"]);
    }

    #[test]
    fn move_selected_to_bottom_preserves_relative_order() {
        let mut tracks = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        move_selected_to_bottom(&mut tracks, &[0, 2]);
        assert_eq!(tracks, vec!["b", "d", "a", "c"]);
    }

    #[test]
    fn shift_selected_up_moves_the_whole_block_and_stops_at_the_top() {
        let mut tracks = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        shift_selected(&mut tracks, &[1, 2], -1);
        assert_eq!(tracks, vec!["b", "c", "a", "d"]);

        // Already at the top: no-op.
        let mut at_top = vec!["a".into(), "b".into()];
        shift_selected(&mut at_top, &[0], -1);
        assert_eq!(at_top, vec!["a", "b"]);
    }

    #[test]
    fn shift_selected_down_moves_the_whole_block_and_stops_at_the_bottom() {
        let mut tracks = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        shift_selected(&mut tracks, &[1, 2], 1);
        assert_eq!(tracks, vec!["a", "d", "b", "c"]);

        let mut at_bottom = vec!["a".into(), "b".into()];
        shift_selected(&mut at_bottom, &[1], 1);
        assert_eq!(at_bottom, vec!["a", "b"]);
    }

    #[test]
    fn remove_selected_drops_every_selected_index() {
        let mut tracks = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        remove_selected(&mut tracks, &[0, 2]);
        assert_eq!(tracks, vec!["b", "d"]);
    }

    // ---- Add-mode sorting ----

    #[test]
    fn tracks_for_songs_mode_sorts_by_title() {
        let tracks = vec![sample_track("Zebra"), sample_track("Apple")];
        let sorted = tracks_for_add_mode(&tracks, AddTracksMode::Songs);
        assert_eq!(sorted[0].title, "Apple");
        assert_eq!(sorted[1].title, "Zebra");
    }

    // ---- active_tracks sync ----

    #[test]
    fn selecting_a_playlist_loads_its_tracks_and_clears_selection() {
        let playlists = vec![sample_playlist("Mix", vec!["a.flac", "b.flac"])];
        let mut state = PlaylistsViewState::default();
        state.selected_playlist_id = Some(playlists[0].full_path.clone());
        state.selected_rows.push("0".into());

        sync_active_tracks(&playlists, &mut state);

        assert_eq!(state.active_tracks, vec!["a.flac", "b.flac"]);
        assert!(state.selected_rows.is_empty());
    }

    #[test]
    fn sync_is_a_noop_when_the_selection_has_not_changed() {
        let playlists = vec![sample_playlist("Mix", vec!["a.flac"])];
        let mut state = PlaylistsViewState::default();
        state.selected_playlist_id = Some(playlists[0].full_path.clone());
        sync_active_tracks(&playlists, &mut state);

        // Simulate a local edit (as the batch bar would make) and confirm a second sync with
        // the same selection doesn't clobber it.
        state.active_tracks.push("b.flac".into());
        sync_active_tracks(&playlists, &mut state);
        assert_eq!(state.active_tracks, vec!["a.flac", "b.flac"]);
    }

    // ---- Headless render smoke test ----

    #[test]
    fn playlists_view_renders_in_headless_frame_for_every_state() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);

        let playlists = vec![sample_playlist("Mix", vec!["a.flac", "b.flac"])];
        let tracks = vec![sample_track("Song A"), sample_track("Song B")];

        let sheets = [
            PlaylistsSheet::None,
            PlaylistsSheet::CreatePlaylist { name: "New Mix".into() },
            PlaylistsSheet::RenamePlaylist { playlist_id: playlists[0].full_path.clone(), name: "Mix".into() },
            PlaylistsSheet::DeleteConfirm { playlist_id: playlists[0].full_path.clone() },
            PlaylistsSheet::AddTracks { mode: AddTracksMode::Songs, search: String::new(), selected: Vec::new() },
        ];

        for (selected_playlist, sheet) in [(None, PlaylistsSheet::None)]
            .into_iter()
            .chain(sheets.into_iter().map(|s| (Some(playlists[0].full_path.clone()), s)))
        {
            let mut state = PlaylistsViewState { selected_playlist_id: selected_playlist, sheet, ..PlaylistsViewState::default() };
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render(ui, &playlists, &tracks, &mut state);
                });
            });
        }
    }
}
