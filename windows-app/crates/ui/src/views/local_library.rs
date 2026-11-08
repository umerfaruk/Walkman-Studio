//! `LocalLibraryView` (spec task 14.1), ported from `LocalLibraryView.swift`.
//!
//! The Mac-side → PC-side local music library: five display modes, eight sort options, a
//! search bar, per-track/per-folder context menus (including `Relocate Folder…`), a
//! `DirectTransfer` sheet, format-warning + removal-confirmation dialogs, and the
//! `Manage Local Folders…` sheet (R4.1-4.6).
//!
//! This view owns its own UI state ([`LocalLibraryViewState`]) and takes the tracks/folders to
//! display as plain parameters rather than reading them off [`crate::shell::AppState`] directly
//! — real data/service wiring (populating those parameters from `walkman_services::
//! local_library::LocalLibrary`, handling the commands this view emits) is task 18's job, not
//! this one's. [`render`] pushes every user-initiated action into
//! [`LocalLibraryViewState::commands`] for whoever owns the state to drain and act on.

use std::collections::BTreeMap;

use eframe::egui;

use walkman_domain::models::{LocalMusicFolder, Track};

use crate::theme::{button, color, font, glass, sizing};

/// R4.2's five display modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LocalDisplayMode {
    #[default]
    Songs,
    Albums,
    Artists,
    RecentlyAdded,
    Folders,
}

impl LocalDisplayMode {
    pub const ALL: [LocalDisplayMode; 5] =
        [Self::Songs, Self::Albums, Self::Artists, Self::RecentlyAdded, Self::Folders];

    pub fn title(self) -> &'static str {
        match self {
            Self::Songs => "Songs",
            Self::Albums => "Albums",
            Self::Artists => "Artists",
            Self::RecentlyAdded => "Recently Added",
            Self::Folders => "Folders",
        }
    }
}

/// R4.3's eight sort options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LocalSortOption {
    #[default]
    TitleAz,
    TitleZa,
    ArtistAz,
    AlbumAz,
    DateAddedNewest,
    DateAddedOldest,
    DurationLongest,
    AudioQuality,
}

impl LocalSortOption {
    pub const ALL: [LocalSortOption; 8] = [
        Self::TitleAz,
        Self::TitleZa,
        Self::ArtistAz,
        Self::AlbumAz,
        Self::DateAddedNewest,
        Self::DateAddedOldest,
        Self::DurationLongest,
        Self::AudioQuality,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::TitleAz => "Title A\u{2013}Z",
            Self::TitleZa => "Title Z\u{2013}A",
            Self::ArtistAz => "Artist A\u{2013}Z",
            Self::AlbumAz => "Album A\u{2013}Z",
            Self::DateAddedNewest => "Date Added (Newest)",
            Self::DateAddedOldest => "Date Added (Oldest)",
            Self::DurationLongest => "Duration (Longest)",
            Self::AudioQuality => "Audio Quality",
        }
    }
}

/// Sorts `tracks` in place per `option` (R4.3).
pub fn sort_tracks(tracks: &mut [Track], option: LocalSortOption) {
    match option {
        LocalSortOption::TitleAz => tracks.sort_by_key(|t| t.title.to_lowercase()),
        LocalSortOption::TitleZa => tracks.sort_by_key(|t| std::cmp::Reverse(t.title.to_lowercase())),
        LocalSortOption::ArtistAz => tracks.sort_by_key(|t| t.artist.to_lowercase()),
        LocalSortOption::AlbumAz => tracks.sort_by_key(|t| t.album.to_lowercase()),
        LocalSortOption::DateAddedNewest => tracks.sort_by_key(|t| std::cmp::Reverse(t.date_added)),
        LocalSortOption::DateAddedOldest => tracks.sort_by_key(|t| t.date_added),
        LocalSortOption::DurationLongest => {
            tracks.sort_by(|a, b| b.duration_seconds.total_cmp(&a.duration_seconds));
        }
        LocalSortOption::AudioQuality => tracks.sort_by_key(|t| std::cmp::Reverse(t.bitrate_kbps)),
    }
}

/// Case/whitespace-insensitive substring match against title/artist/album (R4.4's search bar).
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

/// One row in the Albums display mode: an `(album, artist)` grouping of the filtered tracks.
#[derive(Debug, Clone, PartialEq)]
pub struct AlbumGroup {
    pub album: String,
    pub artist: String,
    pub track_count: usize,
}

pub fn group_by_album(tracks: &[&Track]) -> Vec<AlbumGroup> {
    let mut counts: BTreeMap<(String, String), (String, String, usize)> = BTreeMap::new();
    for t in tracks {
        let key = (t.album.to_lowercase(), t.artist.to_lowercase());
        let entry = counts.entry(key).or_insert_with(|| (t.album.clone(), t.artist.clone(), 0));
        entry.2 += 1;
    }
    let mut groups: Vec<AlbumGroup> = counts
        .into_values()
        .map(|(album, artist, track_count)| AlbumGroup { album, artist, track_count })
        .collect();
    groups.sort_by_key(|g| g.album.to_lowercase());
    groups
}

/// One row in the Artists display mode.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtistGroup {
    pub artist: String,
    pub track_count: usize,
}

pub fn group_by_artist(tracks: &[&Track]) -> Vec<ArtistGroup> {
    let mut counts: BTreeMap<String, (String, usize)> = BTreeMap::new();
    for t in tracks {
        let entry = counts.entry(t.artist.to_lowercase()).or_insert_with(|| (t.artist.clone(), 0));
        entry.1 += 1;
    }
    let mut groups: Vec<ArtistGroup> =
        counts.into_values().map(|(artist, track_count)| ArtistGroup { artist, track_count }).collect();
    groups.sort_by_key(|g| g.artist.to_lowercase());
    groups
}

/// Which modal sheet (if any) is open (R4.1/4.5/4.6).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum LocalLibrarySheet {
    #[default]
    None,
    ManageFolders,
    DirectTransfer { track_ids: Vec<String> },
    FormatWarning { track_ids: Vec<String> },
    RemoveFolderConfirm { folder_id: String, folder_name: String },
    RemoveMissingConfirm,
    AudioNotFound { path: String },
}

/// An action the view wants performed, pushed to [`LocalLibraryViewState::commands`] for the
/// owner of the state to drain (task 18 wires these to real `walkman_services` calls).
#[derive(Debug, Clone, PartialEq)]
pub enum LocalLibraryCommand {
    AddFolder,
    RemoveFolder(String),
    RelocateFolder(String),
    RescanFolder(String),
    RemoveMissingFoldersAndTracks,
    /// Transfer & Convert (All) — the format-warning alert's escalation action (R4.5).
    TransferAndConvertAll(Vec<String>),
    /// A plain direct transfer with no format warning needed.
    DirectTransfer(Vec<String>),
    OpenMetadataEditor(String),
    OpenTrimCut(String),
    Delete(Vec<String>),
}

/// This view's own UI state.
#[derive(Debug, Default)]
pub struct LocalLibraryViewState {
    pub display_mode: LocalDisplayMode,
    pub sort: LocalSortOption,
    pub search_query: String,
    pub selected: std::collections::HashSet<String>,
    pub sheet: LocalLibrarySheet,
    /// Actions emitted this/last frame; drained by the state's owner.
    pub commands: Vec<LocalLibraryCommand>,
}

/// Renders the Local Library view into `ui`. `tracks`/`folders` are the full, unfiltered sets;
/// this function applies `state`'s search/sort/display-mode itself.
pub fn render(ui: &mut egui::Ui, tracks: &[Track], folders: &[LocalMusicFolder], state: &mut LocalLibraryViewState) {
    render_mode_and_search_bar(ui, state);
    ui.add_space(8.0);

    let filtered = filter_tracks(tracks, &state.search_query);

    match state.display_mode {
        LocalDisplayMode::Songs => render_songs_table(ui, &filtered, state),
        LocalDisplayMode::RecentlyAdded => {
            let mut sorted: Vec<Track> = filtered.iter().map(|t| (*t).clone()).collect();
            sort_tracks(&mut sorted, LocalSortOption::DateAddedNewest);
            let refs: Vec<&Track> = sorted.iter().collect();
            render_songs_table(ui, &refs, state);
        }
        LocalDisplayMode::Albums => render_albums_list(ui, &filtered, state),
        LocalDisplayMode::Artists => render_artists_list(ui, &filtered, state),
        LocalDisplayMode::Folders => render_folders_list(ui, folders, state),
    }

    render_sheet(ui, folders, state);
}

fn render_mode_and_search_bar(ui: &mut egui::Ui, state: &mut LocalLibraryViewState) {
    ui.horizontal(|ui| {
        for mode in LocalDisplayMode::ALL {
            let active = state.display_mode == mode;
            let variant = if active { button::Variant::Capsule } else { button::Variant::Glass };
            if ui.add(button::Button::new(mode.title(), variant)).clicked() {
                state.display_mode = mode;
            }
        }

        ui.add_space(12.0);
        if state.display_mode != LocalDisplayMode::Folders {
            egui::ComboBox::from_id_salt("local_library_sort")
                .selected_text(state.sort.title())
                .show_ui(ui, |ui| {
                    for option in LocalSortOption::ALL {
                        ui.selectable_value(&mut state.sort, option, option.title());
                    }
                });
        }

        let search_frame = glass::liquid_glass(glass::State::Normal);
        search_frame.show(ui, |ui| {
            ui.set_min_width(220.0);
            ui.add(
                egui::TextEdit::singleline(&mut state.search_query)
                    .hint_text("Search")
                    .desired_width(200.0)
                    .font(font::body()),
            );
        });

        if ui.add(button::Button::new("Manage Local Folders\u{2026}", button::Variant::Subtle)).clicked() {
            state.sheet = LocalLibrarySheet::ManageFolders;
        }
    });
}

fn render_songs_table(ui: &mut egui::Ui, tracks: &[&Track], state: &mut LocalLibraryViewState) {
    let mut owned: Vec<Track> = tracks.iter().map(|t| (*t).clone()).collect();
    sort_tracks(&mut owned, state.sort);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for track in &owned {
            render_track_row(ui, track, state);
        }
        ui.add_space(sizing::TABLE_BOTTOM_INSET);
    });
}

fn render_track_row(ui: &mut egui::Ui, track: &Track, state: &mut LocalLibraryViewState) {
    let selected = state.selected.contains(&track.full_path);
    let glass_state = if selected { glass::State::Selected } else { glass::State::Normal };
    let frame = glass::liquid_glass(glass_state);

    let response = frame
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&track.title).font(font::body_medium()).color(color::SILVER_LIGHT));
                ui.label(egui::RichText::new("\u{2014}").color(color::SILVER_MUTED));
                ui.label(egui::RichText::new(&track.artist).font(font::body()).color(color::SILVER_MUTED));
                if track.bits_per_sample > 16 || track.sample_rate > 48_000 {
                    crate::theme::widgets::hi_res_badge(ui);
                }
            });
        })
        .response;
    let response = response.interact(egui::Sense::click());

    if response.clicked() {
        let ctrl = ui.input(|i| i.modifiers.command || i.modifiers.ctrl);
        if ctrl {
            if !state.selected.remove(&track.full_path) {
                state.selected.insert(track.full_path.clone());
            }
        } else {
            state.selected.clear();
            state.selected.insert(track.full_path.clone());
        }
    }

    response.context_menu(|ui| {
        if ui.button("Play").clicked() {
            ui.close_menu();
        }
        if ui.button("Edit Metadata\u{2026}").clicked() {
            state.commands.push(LocalLibraryCommand::OpenMetadataEditor(track.full_path.clone()));
            ui.close_menu();
        }
        if ui.button("Trim & Cut\u{2026}").clicked() {
            state.commands.push(LocalLibraryCommand::OpenTrimCut(track.full_path.clone()));
            ui.close_menu();
        }
        if ui.button("Transfer to Walkman\u{2026}").clicked() {
            open_direct_transfer(state, vec![track.full_path.clone()]);
            ui.close_menu();
        }
        ui.separator();
        if ui.button("Delete").clicked() {
            state.commands.push(LocalLibraryCommand::Delete(vec![track.full_path.clone()]));
            ui.close_menu();
        }
    });

    ui.add_space(2.0);
}

/// Opens the direct-transfer flow for `track_ids`, routing through the format-warning alert
/// first if the paths suggest a format a Walkman wouldn't accept as-is (R4.5). This view has no
/// device/format context of its own (that's `transfer_queue::is_format_supported`, task 10.2's
/// job, invoked once real device state exists) — it approximates with a simple extension check
/// so the sheet flow itself is exercisable before that wiring lands.
fn open_direct_transfer(state: &mut LocalLibraryViewState, track_ids: Vec<String>) {
    let any_uncommon = track_ids.iter().any(|p| {
        let ext = std::path::Path::new(p).extension().and_then(|e| e.to_str()).unwrap_or("").to_uppercase();
        !matches!(ext.as_str(), "MP3" | "FLAC" | "WAV" | "AAC" | "M4A" | "ALAC")
    });
    state.sheet = if any_uncommon {
        LocalLibrarySheet::FormatWarning { track_ids }
    } else {
        LocalLibrarySheet::DirectTransfer { track_ids }
    };
}

fn render_albums_list(ui: &mut egui::Ui, tracks: &[&Track], _state: &mut LocalLibraryViewState) {
    let groups = group_by_album(tracks);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for group in &groups {
            let frame = glass::liquid_glass(glass::State::Normal);
            let response = frame
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&group.album).font(font::title3()).color(color::SILVER_LIGHT));
                        ui.label(
                            egui::RichText::new(format!("{} \u{2022} {} tracks", group.artist, group.track_count))
                                .font(font::callout())
                                .color(color::SILVER_MUTED),
                        );
                    });
                })
                .response;
            response.context_menu(|ui| {
                if ui.button("Transfer Album to Walkman\u{2026}").clicked() {
                    ui.close_menu();
                }
            });
            ui.add_space(2.0);
        }
        ui.add_space(sizing::TABLE_BOTTOM_INSET);
    });
}

fn render_artists_list(ui: &mut egui::Ui, tracks: &[&Track], _state: &mut LocalLibraryViewState) {
    let groups = group_by_artist(tracks);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for group in &groups {
            let frame = glass::liquid_glass(glass::State::Normal);
            frame.show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&group.artist).font(font::title3()).color(color::SILVER_LIGHT));
                    ui.label(
                        egui::RichText::new(format!("{} tracks", group.track_count))
                            .font(font::callout())
                            .color(color::SILVER_MUTED),
                    );
                });
            });
            ui.add_space(2.0);
        }
        ui.add_space(sizing::TABLE_BOTTOM_INSET);
    });
}

fn render_folders_list(ui: &mut egui::Ui, folders: &[LocalMusicFolder], state: &mut LocalLibraryViewState) {
    ui.horizontal(|ui| {
        if ui.add(button::Button::new("Remove Missing Folders and Tracks\u{2026}", button::Variant::Destructive)).clicked()
        {
            state.sheet = LocalLibrarySheet::RemoveMissingConfirm;
        }
    });
    ui.add_space(6.0);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for folder in folders {
            let frame = glass::liquid_glass(glass::State::Normal);
            let response = frame
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(&folder.display_name).font(font::body_medium()).color(color::SILVER_LIGHT),
                        );
                        if folder.is_missing {
                            ui.label(egui::RichText::new("Missing").font(font::caption()).color(color::HIRES_GOLD));
                        }
                        ui.label(
                            egui::RichText::new(format!("{} tracks", folder.track_count))
                                .font(font::callout())
                                .color(color::SILVER_MUTED),
                        );
                    });
                })
                .response;

            response.context_menu(|ui| {
                if ui.button("Relocate Folder\u{2026}").clicked() {
                    state.commands.push(LocalLibraryCommand::RelocateFolder(folder.id.clone()));
                    ui.close_menu();
                }
                if ui.button("Rescan").clicked() {
                    state.commands.push(LocalLibraryCommand::RescanFolder(folder.id.clone()));
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Remove from Local Library\u{2026}").clicked() {
                    state.sheet = LocalLibrarySheet::RemoveFolderConfirm {
                        folder_id: folder.id.clone(),
                        folder_name: folder.display_name.clone(),
                    };
                    ui.close_menu();
                }
            });
            ui.add_space(2.0);
        }
        ui.add_space(sizing::TABLE_BOTTOM_INSET);
    });
}

/// A centered modal: a dimmed full-screen backdrop plus a [`glass::modal_glass_chassis`] panel
/// sized `size`, matching the `Area`-based overlay pattern `now_playing.rs`'s fullscreen player
/// already established.
fn modal(ctx: &egui::Context, id: &str, size: egui::Vec2, content: impl FnOnce(&mut egui::Ui)) {
    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new(id)).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
        ui.set_min_size(screen.size());
        ui.painter().rect_filled(screen, 0.0, egui::Color32::from_black_alpha(140));

        let panel_rect = egui::Rect::from_center_size(screen.center(), size);
        let mut panel_ui = ui.new_child(egui::UiBuilder::new().max_rect(panel_rect));
        glass::modal_glass_chassis().show(&mut panel_ui, |ui| {
            ui.set_min_size(size - egui::vec2(32.0, 32.0));
            content(ui);
        });
    });
}

fn render_sheet(ui: &mut egui::Ui, folders: &[LocalMusicFolder], state: &mut LocalLibraryViewState) {
    let ctx = ui.ctx().clone();
    match state.sheet.clone() {
        LocalLibrarySheet::None => {}
        LocalLibrarySheet::ManageFolders => {
            modal(&ctx, "manage_local_folders_sheet", egui::vec2(560.0, 420.0), |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Manage Local Folders").font(font::title2()).color(color::SILVER_LIGHT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(button::Button::new("Add Folder\u{2026}", button::Variant::Prominent)).clicked() {
                            state.commands.push(LocalLibraryCommand::AddFolder);
                        }
                    });
                });
                ui.add_space(10.0);

                if folders.is_empty() {
                    ui.label(egui::RichText::new("No folders registered yet").font(font::callout()).color(color::SILVER_MUTED));
                } else {
                    egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                        for folder in folders {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(&folder.display_name).font(font::body_medium()).color(color::SILVER_LIGHT));
                                    let status = if folder.is_missing {
                                        "Missing".to_owned()
                                    } else {
                                        format!("{} tracks", folder.track_count)
                                    };
                                    ui.label(egui::RichText::new(status).font(font::caption()).color(color::SILVER_MUTED));
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.add(button::Button::new("Remove", button::Variant::Destructive)).clicked() {
                                        state.sheet = LocalLibrarySheet::RemoveFolderConfirm {
                                            folder_id: folder.id.clone(),
                                            folder_name: folder.display_name.clone(),
                                        };
                                    }
                                    ui.add_space(6.0);
                                    if ui.add(button::Button::new("Rescan", button::Variant::Glass)).clicked() {
                                        state.commands.push(LocalLibraryCommand::RescanFolder(folder.id.clone()));
                                    }
                                });
                            });
                            ui.add_space(4.0);
                        }
                    });
                }

                ui.add_space(12.0);
                if ui.add(button::Button::new("Close", button::Variant::Glass)).clicked() {
                    state.sheet = LocalLibrarySheet::None;
                }
            });
        }
        LocalLibrarySheet::DirectTransfer { track_ids } => {
            modal(&ctx, "direct_transfer_sheet", egui::vec2(480.0, 240.0), |ui| {
                ui.label(egui::RichText::new("Transfer to Walkman").font(font::title2()).color(color::SILVER_LIGHT));
                ui.label(
                    egui::RichText::new(format!("{} track(s) selected", track_ids.len()))
                        .font(font::callout())
                        .color(color::SILVER_MUTED),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(button::Button::new("Transfer", button::Variant::Prominent)).clicked() {
                        state.commands.push(LocalLibraryCommand::DirectTransfer(track_ids.clone()));
                        state.sheet = LocalLibrarySheet::None;
                    }
                    if ui.add(button::Button::new("Cancel", button::Variant::Glass)).clicked() {
                        state.sheet = LocalLibrarySheet::None;
                    }
                });
            });
        }
        LocalLibrarySheet::FormatWarning { track_ids } => {
            modal(&ctx, "format_warning_sheet", egui::vec2(480.0, 260.0), |ui| {
                ui.label(egui::RichText::new("Unsupported Format").font(font::title2()).color(color::SILVER_LIGHT));
                ui.label(
                    egui::RichText::new(
                        "One or more selected tracks may not play on this Walkman without converting.",
                    )
                    .font(font::callout())
                    .color(color::SILVER_MUTED),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(button::Button::new("Transfer & Convert (All)", button::Variant::Gold)).clicked() {
                        state.commands.push(LocalLibraryCommand::TransferAndConvertAll(track_ids.clone()));
                        state.sheet = LocalLibrarySheet::None;
                    }
                    if ui.add(button::Button::new("Cancel", button::Variant::Glass)).clicked() {
                        state.sheet = LocalLibrarySheet::None;
                    }
                });
            });
        }
        LocalLibrarySheet::RemoveFolderConfirm { folder_id, folder_name } => {
            modal(&ctx, "remove_folder_confirm_sheet", egui::vec2(440.0, 200.0), |ui| {
                ui.label(
                    egui::RichText::new("Remove Folder from Local Library?").font(font::title3()).color(color::SILVER_LIGHT),
                );
                ui.label(egui::RichText::new(&folder_name).font(font::callout()).color(color::SILVER_MUTED));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(button::Button::new("Remove", button::Variant::Destructive)).clicked() {
                        state.commands.push(LocalLibraryCommand::RemoveFolder(folder_id.clone()));
                        state.sheet = LocalLibrarySheet::None;
                    }
                    if ui.add(button::Button::new("Cancel", button::Variant::Glass)).clicked() {
                        state.sheet = LocalLibrarySheet::None;
                    }
                });
            });
        }
        LocalLibrarySheet::RemoveMissingConfirm => {
            modal(&ctx, "remove_missing_confirm_sheet", egui::vec2(440.0, 200.0), |ui| {
                ui.label(
                    egui::RichText::new("Remove Missing Folders and Tracks?")
                        .font(font::title3())
                        .color(color::SILVER_LIGHT),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(button::Button::new("Remove", button::Variant::Destructive)).clicked() {
                        state.commands.push(LocalLibraryCommand::RemoveMissingFoldersAndTracks);
                        state.sheet = LocalLibrarySheet::None;
                    }
                    if ui.add(button::Button::new("Cancel", button::Variant::Glass)).clicked() {
                        state.sheet = LocalLibrarySheet::None;
                    }
                });
            });
        }
        LocalLibrarySheet::AudioNotFound { path } => {
            modal(&ctx, "audio_not_found_sheet", egui::vec2(440.0, 180.0), |ui| {
                ui.label(egui::RichText::new("Audio File Not Found").font(font::title3()).color(color::SILVER_LIGHT));
                ui.label(egui::RichText::new(&path).font(font::mono()).color(color::SILVER_MUTED));
                ui.add_space(12.0);
                if ui.add(button::Button::new("OK", button::Variant::Prominent)).clicked() {
                    state.sheet = LocalLibrarySheet::None;
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn track(title: &str, artist: &str, album: &str, bits: i32, rate: i32, bitrate: i32) -> Track {
        Track {
            filename: format!("{title}.flac"),
            relative_path: format!("{artist}/{album}/{title}.flac"),
            full_path: format!("/local/{artist}/{album}/{title}.flac"),
            format: "FLAC".into(),
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            track_num: "1".into(),
            year: "2024".into(),
            genre: String::new(),
            duration_seconds: 180.0,
            bitrate_kbps: bitrate,
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
    fn filter_tracks_matches_title_artist_or_album_case_insensitively() {
        let tracks = vec![
            track("Dawn", "Nova", "Horizon", 16, 44_100, 900),
            track("Dusk", "Echo", "Reverb", 16, 44_100, 900),
        ];
        assert_eq!(filter_tracks(&tracks, "nova").len(), 1);
        assert_eq!(filter_tracks(&tracks, "REVERB").len(), 1);
        assert_eq!(filter_tracks(&tracks, "").len(), 2);
        assert_eq!(filter_tracks(&tracks, "nonexistent").len(), 0);
    }

    #[test]
    fn sort_tracks_title_az_and_za() {
        let mut tracks = vec![
            track("Zeta", "A", "Al", 16, 44_100, 900),
            track("Alpha", "A", "Al", 16, 44_100, 900),
        ];
        sort_tracks(&mut tracks, LocalSortOption::TitleAz);
        assert_eq!(tracks[0].title, "Alpha");
        sort_tracks(&mut tracks, LocalSortOption::TitleZa);
        assert_eq!(tracks[0].title, "Zeta");
    }

    #[test]
    fn sort_tracks_by_date_added_newest_and_oldest() {
        let mut tracks = vec![
            track("A", "X", "Y", 16, 44_100, 900),
            track("B", "X", "Y", 16, 44_100, 900),
        ];
        tracks[0].date_added = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        tracks[1].date_added = Utc.with_ymd_and_hms(2023, 1, 1, 0, 0, 0).unwrap();

        sort_tracks(&mut tracks, LocalSortOption::DateAddedNewest);
        assert_eq!(tracks[0].title, "B");
        sort_tracks(&mut tracks, LocalSortOption::DateAddedOldest);
        assert_eq!(tracks[0].title, "A");
    }

    #[test]
    fn sort_tracks_by_audio_quality_prefers_higher_bitrate() {
        let mut tracks = vec![
            track("Low", "X", "Y", 16, 44_100, 128),
            track("High", "X", "Y", 24, 96_000, 900),
        ];
        sort_tracks(&mut tracks, LocalSortOption::AudioQuality);
        assert_eq!(tracks[0].title, "High");
    }

    #[test]
    fn group_by_album_counts_tracks_per_album_artist_pair() {
        let tracks = [
            track("A", "Nova", "Horizon", 16, 44_100, 900),
            track("B", "Nova", "Horizon", 16, 44_100, 900),
            track("C", "Echo", "Reverb", 16, 44_100, 900),
        ];
        let refs: Vec<&Track> = tracks.iter().collect();
        let groups = group_by_album(&refs);
        assert_eq!(groups.len(), 2);
        let horizon = groups.iter().find(|g| g.album == "Horizon").unwrap();
        assert_eq!(horizon.track_count, 2);
        assert_eq!(horizon.artist, "Nova");
    }

    #[test]
    fn group_by_artist_counts_tracks_per_artist() {
        let tracks = [
            track("A", "Nova", "Horizon", 16, 44_100, 900),
            track("B", "Nova", "Echoes", 16, 44_100, 900),
        ];
        let refs: Vec<&Track> = tracks.iter().collect();
        let groups = group_by_artist(&refs);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].track_count, 2);
    }

    #[test]
    fn open_direct_transfer_routes_through_format_warning_for_uncommon_extensions() {
        let mut state = LocalLibraryViewState::default();
        open_direct_transfer(&mut state, vec!["/music/song.ogg".to_string()]);
        assert!(matches!(state.sheet, LocalLibrarySheet::FormatWarning { .. }));

        let mut state2 = LocalLibraryViewState::default();
        open_direct_transfer(&mut state2, vec!["/music/song.flac".to_string()]);
        assert!(matches!(state2.sheet, LocalLibrarySheet::DirectTransfer { .. }));
    }

    /// Drives the view through every display mode and every sheet in a headless egui frame
    /// (same convention as `detail.rs`'s router test): an out-of-range rect, a bad font family,
    /// or a malformed shape would panic during layout/paint, so this exercises real rendering
    /// without a live window.
    #[test]
    fn renders_in_headless_frame_for_every_display_mode_and_sheet() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);

        let tracks = vec![
            track("Dawn", "Nova", "Horizon", 24, 96_000, 900),
            track("Dusk", "Echo", "Reverb", 16, 44_100, 320),
        ];
        let folders = vec![LocalMusicFolder {
            id: "f1".into(),
            path: "/music".into(),
            display_name: "Music".into(),
            date_added: Utc::now(),
            last_scanned: Utc::now(),
            track_count: 2,
            total_bytes: 100,
            is_missing: false,
        }];

        let sheets = [
            LocalLibrarySheet::None,
            LocalLibrarySheet::ManageFolders,
            LocalLibrarySheet::DirectTransfer { track_ids: vec!["a".into()] },
            LocalLibrarySheet::FormatWarning { track_ids: vec!["a".into()] },
            LocalLibrarySheet::RemoveFolderConfirm { folder_id: "f1".into(), folder_name: "Music".into() },
            LocalLibrarySheet::RemoveMissingConfirm,
            LocalLibrarySheet::AudioNotFound { path: "/music/missing.flac".into() },
        ];

        for mode in LocalDisplayMode::ALL {
            for sheet in &sheets {
                let mut state = LocalLibraryViewState { display_mode: mode, sheet: sheet.clone(), ..Default::default() };
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        render(ui, &tracks, &folders, &mut state);
                    });
                });
            }
        }
    }
}
