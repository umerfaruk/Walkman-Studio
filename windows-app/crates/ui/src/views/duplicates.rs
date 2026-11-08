//! `DuplicatesView`, ported from `Sources/WalkmanStudio/Views/DuplicatesView.swift`.
//!
//! Spec task **17.2** (requirements 10.4, 10.5): the scope/filter picker, the 4 sort
//! options, per-group resolution actions, recoverable-space totals, in-app audition,
//! reveal-in-Explorer, `Auto-Resolve Quality`, and native delete confirms — over the real
//! `walkman_services::duplicate_finder` clustering built in task 11.1.
//!
//! ## Honest simplification
//! "Native confirmation dialogs" (R10.5) means the real OS dialog
//! (`platform::FileDialogs`/a Windows message box, task 6.6); this view only renders its own
//! in-app confirm sheet and emits the delete command on confirmation — swapping in a real
//! native dialog is a platform-layer wiring concern for task 18, not a UI-layer one.

use eframe::egui;

use walkman_domain::models::Track;
use walkman_services::duplicate_finder::{self, DuplicateGroup, DuplicateScope, DuplicateSortOption};

use crate::inspector::format_size;
use crate::theme::{button, color, font, glass, sizing};

/// A stable identity for a group across re-sorts/re-filters within a frame (groups
/// themselves carry no id) — every member's full path, joined, is unique to that exact set
/// of tracks regardless of display order.
pub fn group_key(group: &DuplicateGroup) -> String {
    let mut paths: Vec<&str> = group.tracks.iter().map(|t| t.full_path.as_str()).collect();
    paths.sort_unstable();
    paths.join("\u{1F}")
}

#[derive(Debug, Clone, PartialEq)]
pub enum DuplicatesCommand {
    /// Delete this one track (+ companions), after the confirm sheet.
    DeleteTrack(String),
    /// Keep a group's `best_quality_track`, delete the rest (the full paths listed).
    ResolveGroupKeepBest(Vec<String>),
    /// R10.4's one-click `Auto-Resolve Quality` across every visible group.
    AutoResolveQuality,
    Audition(String),
    RevealInExplorer(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum DuplicatesSheet {
    #[default]
    None,
    DeleteConfirm { track_id: String },
    ResolveGroupConfirm { group_key: String, track_ids: Vec<String> },
    AutoResolveConfirm,
}

#[derive(Debug, Clone)]
pub struct DuplicatesViewState {
    pub scope: DuplicateScope,
    pub sort: DuplicateSortOption,
    pub expanded: Vec<String>,
    pub sheet: DuplicatesSheet,
    pub commands: Vec<DuplicatesCommand>,
}

impl Default for DuplicatesViewState {
    fn default() -> Self {
        Self {
            scope: DuplicateScope::ExactSameAlbum,
            sort: DuplicateSortOption::SpaceRecoverable,
            expanded: Vec::new(),
            sheet: DuplicatesSheet::default(),
            commands: Vec::new(),
        }
    }
}

pub fn render(ui: &mut egui::Ui, tracks: &[Track], state: &mut DuplicatesViewState) {
    let mut groups = duplicate_finder::find_duplicates(tracks, state.scope);
    duplicate_finder::sort_groups(&mut groups, state.sort);

    render_toolbar(ui, &groups, state);
    ui.add_space(8.0);

    if groups.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(egui::RichText::new("No duplicates found").font(font::callout()).color(color::SILVER_MUTED));
        });
    } else {
        render_groups(ui, &groups, state);
    }

    render_sheet(ui, state);
}

fn render_toolbar(ui: &mut egui::Ui, groups: &[DuplicateGroup], state: &mut DuplicatesViewState) {
    ui.horizontal(|ui| {
        for (scope, label) in [(DuplicateScope::ExactSameAlbum, "Exact Same Album (Safe)"), (DuplicateScope::CrossAlbum, "Include Cross-Album (Loose)")] {
            let variant = if state.scope == scope { button::Variant::Capsule } else { button::Variant::Glass };
            if ui.add(button::Button::new(label, variant)).clicked() {
                state.scope = scope;
            }
        }

        ui.add_space(12.0);
        egui::ComboBox::from_id_salt("duplicates_sort")
            .selected_text(sort_label(state.sort))
            .show_ui(ui, |ui| {
                for option in [
                    DuplicateSortOption::SpaceRecoverable,
                    DuplicateSortOption::TitleAZ,
                    DuplicateSortOption::ArtistAZ,
                    DuplicateSortOption::MostCopies,
                ] {
                    ui.selectable_value(&mut state.sort, option, sort_label(option));
                }
            });

        ui.add_space(12.0);
        let total = duplicate_finder::total_recoverable_bytes(groups);
        ui.label(
            egui::RichText::new(format!("{} recoverable", format_size(total)))
                .font(font::caption_bold())
                .color(color::SILVER_MUTED),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(button::Button::new("Auto-Resolve Quality", button::Variant::Gold).enabled(!groups.is_empty())).clicked() {
                state.sheet = DuplicatesSheet::AutoResolveConfirm;
            }
        });
    });
}

fn sort_label(option: DuplicateSortOption) -> &'static str {
    match option {
        DuplicateSortOption::SpaceRecoverable => "Space Recoverable",
        DuplicateSortOption::TitleAZ => "Song Title A\u{2013}Z",
        DuplicateSortOption::ArtistAZ => "Artist A\u{2013}Z",
        DuplicateSortOption::MostCopies => "Most Copies",
    }
}

fn render_groups(ui: &mut egui::Ui, groups: &[DuplicateGroup], state: &mut DuplicatesViewState) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for group in groups {
            render_group(ui, group, state);
            ui.add_space(6.0);
        }
        ui.add_space(sizing::TABLE_BOTTOM_INSET);
    });
}

fn render_group(ui: &mut egui::Ui, group: &DuplicateGroup, state: &mut DuplicatesViewState) {
    let key = group_key(group);
    let expanded = state.expanded.contains(&key);
    let best_path = group.best_quality_track().full_path.clone();

    glass::liquid_glass(glass::State::Normal).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            let chevron = if expanded { "\u{25BE}" } else { "\u{25B8}" };
            if ui.add(button::Button::new(chevron, button::Variant::Glass)).clicked() {
                if expanded {
                    state.expanded.retain(|k| k != &key);
                } else {
                    state.expanded.push(key.clone());
                }
            }

            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(group.title()).font(font::body_medium()).color(color::SILVER_LIGHT));
                ui.label(egui::RichText::new(group.artist()).font(font::caption()).color(color::SILVER_MUTED));
            });

            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(format!("{} copies", group.tracks.len()))
                    .font(font::caption_bold())
                    .color(color::SILVER_MUTED),
            );
            if group.has_quality_diff {
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Quality Diff").font(font::caption_bold()).color(color::HIRES_GOLD));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format_size(group.recoverable_bytes()))
                        .font(font::caption())
                        .color(color::SILVER_MUTED),
                );
                ui.add_space(8.0);
                if ui.add(button::Button::new("Keep Best Quality", button::Variant::Glass)).clicked() {
                    let track_ids = group.tracks.iter().map(|t| t.full_path.clone()).filter(|p| p != &best_path).collect();
                    state.sheet = DuplicatesSheet::ResolveGroupConfirm { group_key: key.clone(), track_ids };
                }
            });
        });

        if expanded {
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);
            for track in &group.tracks {
                render_track_row(ui, track, &best_path, state);
            }
        }
    });
}

fn render_track_row(ui: &mut egui::Ui, track: &Track, best_path: &str, state: &mut DuplicatesViewState) {
    ui.horizontal(|ui| {
        if track.full_path == best_path {
            ui.label(egui::RichText::new("\u{2605}").font(font::caption_bold()).color(color::HIRES_GOLD));
        } else {
            ui.add_space(14.0);
        }
        ui.label(
            egui::RichText::new(format!(
                "{} \u{2022} {} kbps \u{2022} {:.1} kHz \u{2022} {}-bit \u{2022} {}",
                track.format,
                track.bitrate_kbps,
                track.sample_rate as f64 / 1000.0,
                track.bits_per_sample,
                format_size(track.size_bytes)
            ))
            .font(font::caption())
            .color(color::SILVER_MUTED),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(button::Button::new("Delete", button::Variant::Destructive)).clicked() {
                state.sheet = DuplicatesSheet::DeleteConfirm { track_id: track.full_path.clone() };
            }
            ui.add_space(6.0);
            if ui.add(button::Button::new("Reveal", button::Variant::Glass)).clicked() {
                state.commands.push(DuplicatesCommand::RevealInExplorer(track.full_path.clone()));
            }
            ui.add_space(6.0);
            if ui.add(button::Button::new("Audition", button::Variant::Glass)).clicked() {
                state.commands.push(DuplicatesCommand::Audition(track.full_path.clone()));
            }
        });
    });
}

fn render_sheet(ui: &mut egui::Ui, state: &mut DuplicatesViewState) {
    let ctx = ui.ctx().clone();
    match state.sheet.clone() {
        DuplicatesSheet::None => {}
        DuplicatesSheet::DeleteConfirm { track_id } => {
            crate::sheets::modal(&ctx, "duplicates_delete_confirm_sheet", egui::vec2(420.0, 200.0), |ui| {
                ui.label(egui::RichText::new("Delete Track?").font(font::title2()).color(color::SILVER_LIGHT));
                ui.label(
                    egui::RichText::new("This permanently deletes the audio file and any companion lyrics file.")
                        .font(font::callout())
                        .color(color::SILVER_MUTED),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(button::Button::new("Delete", button::Variant::Destructive)).clicked() {
                        state.commands.push(DuplicatesCommand::DeleteTrack(track_id.clone()));
                        state.sheet = DuplicatesSheet::None;
                    }
                    if ui.add(button::Button::new("Cancel", button::Variant::Glass)).clicked() {
                        state.sheet = DuplicatesSheet::None;
                    }
                });
            });
        }
        DuplicatesSheet::ResolveGroupConfirm { group_key: _, track_ids } => {
            crate::sheets::modal(&ctx, "duplicates_resolve_group_confirm_sheet", egui::vec2(440.0, 220.0), |ui| {
                ui.label(egui::RichText::new("Keep Best Quality?").font(font::title2()).color(color::SILVER_LIGHT));
                ui.label(
                    egui::RichText::new(format!("Deletes {} lower-quality cop{} in this group.", track_ids.len(), if track_ids.len() == 1 { "y" } else { "ies" }))
                        .font(font::callout())
                        .color(color::SILVER_MUTED),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(button::Button::new("Delete Lower-Quality Copies", button::Variant::Destructive)).clicked() {
                        state.commands.push(DuplicatesCommand::ResolveGroupKeepBest(track_ids.clone()));
                        state.sheet = DuplicatesSheet::None;
                    }
                    if ui.add(button::Button::new("Cancel", button::Variant::Glass)).clicked() {
                        state.sheet = DuplicatesSheet::None;
                    }
                });
            });
        }
        DuplicatesSheet::AutoResolveConfirm => {
            crate::sheets::modal(&ctx, "duplicates_auto_resolve_confirm_sheet", egui::vec2(440.0, 220.0), |ui| {
                ui.label(egui::RichText::new("Auto-Resolve Quality?").font(font::title2()).color(color::SILVER_LIGHT));
                ui.label(
                    egui::RichText::new("Keeps the highest-fidelity copy in every group and deletes the rest, plus their companion lyrics files.")
                        .font(font::callout())
                        .color(color::SILVER_MUTED),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(button::Button::new("Auto-Resolve", button::Variant::Destructive)).clicked() {
                        state.commands.push(DuplicatesCommand::AutoResolveQuality);
                        state.sheet = DuplicatesSheet::None;
                    }
                    if ui.add(button::Button::new("Cancel", button::Variant::Glass)).clicked() {
                        state.sheet = DuplicatesSheet::None;
                    }
                });
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[allow(clippy::too_many_arguments)]
    fn track(path: &str, title: &str, artist: &str, album: &str, duration: f64, bits: i32, rate: i32, size: i64) -> Track {
        Track {
            filename: path.into(),
            relative_path: path.into(),
            full_path: path.into(),
            format: "FLAC".into(),
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            track_num: "1".into(),
            year: "2024".into(),
            genre: String::new(),
            duration_seconds: duration,
            bitrate_kbps: 900,
            sample_rate: rate,
            bits_per_sample: bits,
            has_artwork: false,
            size_bytes: size,
            date_added: Utc::now(),
            is_senseme_analyzed: false,
            senseme_bpm: None,
            senseme_valence: None,
            senseme_arousal: None,
            senseme_channels: Vec::new(),
            senseme_highlight_sec: None,
        }
    }

    fn sample_group() -> DuplicateGroup {
        let a = track("a.flac", "Song", "Artist", "Album", 200.0, 16, 44_100, 10_000);
        let b = track("b.flac", "Song", "Artist", "Album", 200.0, 24, 96_000, 30_000);
        DuplicateGroup { tracks: vec![a, b], has_quality_diff: true }
    }

    // ---- group_key ----

    #[test]
    fn group_key_is_stable_regardless_of_member_order() {
        let mut group = sample_group();
        let key1 = group_key(&group);
        group.tracks.reverse();
        let key2 = group_key(&group);
        assert_eq!(key1, key2);
    }

    #[test]
    fn group_key_differs_for_different_groups() {
        let g1 = sample_group();
        let g2 = DuplicateGroup { tracks: vec![track("c.flac", "Other", "X", "Y", 1.0, 16, 44_100, 1)], has_quality_diff: false };
        assert_ne!(group_key(&g1), group_key(&g2));
    }

    // ---- Default state ----

    #[test]
    fn default_state_starts_on_exact_album_scope_sorted_by_recoverable_space() {
        let state = DuplicatesViewState::default();
        assert_eq!(state.scope, DuplicateScope::ExactSameAlbum);
        assert_eq!(state.sort, DuplicateSortOption::SpaceRecoverable);
        assert_eq!(state.sheet, DuplicatesSheet::None);
    }

    // ---- Headless render smoke test ----

    #[test]
    fn duplicates_view_renders_in_headless_frame_for_every_state() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);

        let tracks = vec![
            track("a.flac", "Song", "Artist", "Album", 200.0, 16, 44_100, 10_000),
            track("b.flac", "Song", "Artist", "Album", 200.5, 24, 96_000, 30_000),
        ];
        let group = duplicate_finder::find_duplicates(&tracks, DuplicateScope::ExactSameAlbum).into_iter().next().unwrap();
        let key = group_key(&group);

        let sheets = [
            DuplicatesSheet::None,
            DuplicatesSheet::DeleteConfirm { track_id: "a.flac".into() },
            DuplicatesSheet::ResolveGroupConfirm { group_key: key, track_ids: vec!["a.flac".into()] },
            DuplicatesSheet::AutoResolveConfirm,
        ];

        for (tracks, sheet, expanded) in
            [(Vec::new(), DuplicatesSheet::None, Vec::new())]
                .into_iter()
                .chain(sheets.into_iter().map(|s| (tracks.clone(), s, Vec::new())))
                .chain(std::iter::once((tracks.clone(), DuplicatesSheet::None, vec![group_key(&duplicate_finder::find_duplicates(&tracks, DuplicateScope::ExactSameAlbum)[0])])))
        {
            let mut state = DuplicatesViewState { sheet, expanded, ..DuplicatesViewState::default() };
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render(ui, &tracks, &mut state);
                });
            });
        }
    }
}
