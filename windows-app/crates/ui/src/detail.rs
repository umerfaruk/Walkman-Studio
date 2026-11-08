//! The central detail router, ported from the `switch selectedTab { … }` in `ContentView`
//! (`Sources/WalkmanStudio/Views/ContentView.swift`).
//!
//! Spec task **4.5**: a `match nav_tab` that renders the view for the selected section
//! (requirement 1.4). Each section's real view lands in tasks 14–17; until then this
//! router renders a distinct, clearly-labeled placeholder pane per section so the shell
//! composes end-to-end and the routing skeleton is verifiable.
//!
//! ## Exhaustive match (parity guarantee)
//!
//! [`render`] and [`section_note`] match over **every** [`NavTab`] variant with **no**
//! catch-all `_` arm. When a new section is added to [`NavTab`], the compiler forces this
//! router (and its per-section note) to be updated too — the same compile-time parity
//! guarantee the parity-checklist test enforces in task 19.4.
//!
//! ## Pure logic vs. rendering
//!
//! The per-section title/note strings are factored into the `Ui`-free [`section_title`]
//! and [`section_note`] helpers so they unit-test without a live egui context.
//! [`render`] walks the active tab and paints the placeholder; [`section_placeholder`]
//! draws a single heading + sub-note within the obsidian theme.

use eframe::egui;

use crate::shell::AppState;
use crate::theme::{color, font};
use crate::NavTab;

/// The section title shown at the top of the detail pane. Reuses [`NavTab::title`] so the
/// detail heading stays in lockstep with the sidebar label (requirement 1.4).
pub fn section_title(nav_tab: NavTab) -> &'static str {
    nav_tab.title()
}

/// A short note pointing to the task that implements the real view for `nav_tab`.
///
/// Each note names the implementing task from `tasks.md` so the placeholder is
/// self-documenting. The match is exhaustive (no `_` arm) so a new [`NavTab`] cannot be
/// added without giving it a note here.
pub fn section_note(nav_tab: NavTab) -> &'static str {
    match nav_tab {
        // --- Local Music Library group ---
        NavTab::LocalLibrary => "Local library view — LocalLibraryView (task 14.1).",
        NavTab::SenseMe => "SenseMe\u{2122} channels carousel — SenseMeChannelsView (task 15.3).",
        // --- Walkman Library group ---
        NavTab::Library => "All songs table — LibraryView (task 14.2).",
        NavTab::RecentlyAdded => "Recently added, by time scope — RecentlyAddedView (task 14.3).",
        NavTab::Albums => "Album cover grid — AlbumsView (task 14.4).",
        NavTab::Artists => "Artist grid with avatars — ArtistsView (task 14.5).",
        NavTab::HiRes => "Hi-Res audio browser — HiResAudioView (task 14.6).",
        NavTab::LanguageStudy => "Language study browser — LanguageStudyView (task 14.7).",
        NavTab::Playlists => "Playlist editor — PlaylistsView (task 17.3, done).",
        // --- Device ---
        NavTab::Overview => "Device overview & storage — DeviceOverviewView (task 14.8).",
        // --- Tools ---
        NavTab::Duplicates => "Duplicate finder — DuplicatesView (task 17.2, done).",
        NavTab::Transfer => "Transfer & convert queue — TransferQueueView (task 17.1, done).",
        NavTab::BackupRestore => "Backup & restore — BackupRestoreView (task 17.4).",
    }
}

/// Renders the detail pane for the selected section into `ui`.
///
/// The `match` is exhaustive over every [`NavTab`] (no catch-all) so adding a section is a
/// compile error until it is routed here, mirroring the `switch selectedTab` in
/// `ContentView`. Each arm currently draws a placeholder keyed off [`section_title`] /
/// [`section_note`]; the real section views replace these arms in tasks 14–17, each
/// receiving the inspect/edit/trim/delete/transfer callbacks that mutate shared app state.
pub fn render(ui: &mut egui::Ui, state: &mut AppState) {
    let tab = state.nav_tab;
    match tab {
        NavTab::LocalLibrary => {
            crate::views::local_library::render(
                ui,
                &state.local_tracks,
                &state.local_folders,
                &mut state.local_library_view,
            );
        }
        NavTab::Library => {
            crate::views::library::render(ui, &state.walkman_tracks, &mut state.library_view);
        }
        NavTab::RecentlyAdded => {
            crate::views::recently_added::render(
                ui,
                &state.walkman_tracks,
                chrono::Utc::now(),
                &mut state.recently_added_view,
            );
        }
        NavTab::Albums => {
            crate::views::albums::render(ui, &state.walkman_tracks, &mut state.albums_view);
        }
        NavTab::Artists => {
            crate::views::artists::render(ui, &state.walkman_tracks, &mut state.artists_view);
        }
        NavTab::HiRes => {
            crate::views::hi_res::render(ui, &state.walkman_tracks, &mut state.hi_res_view);
        }
        NavTab::LanguageStudy => {
            crate::views::language_study::render(ui, &state.walkman_tracks, &mut state.language_study_view);
        }
        NavTab::Overview => {
            crate::views::device_overview::render(
                ui,
                &state.devices,
                state.device_quick_stats,
                &mut state.device_overview_view,
            );
        }
        NavTab::SenseMe => {
            crate::views::senseme::render(ui, &state.walkman_tracks, &mut state.senseme_view);
        }
        NavTab::Transfer => {
            crate::views::transfer_queue::render(ui, &state.transfer_queue_items, &mut state.transfer_queue_view);
        }
        NavTab::Duplicates => {
            crate::views::duplicates::render(ui, &state.walkman_tracks, &mut state.duplicates_view);
        }
        NavTab::Playlists => {
            crate::views::playlists::render(ui, &state.playlists, &state.walkman_tracks, &mut state.playlists_view);
        }
        NavTab::BackupRestore => {
            section_placeholder(ui, section_title(tab), section_note(tab));
        }
    }
}

/// Draws a single section placeholder: the section `title` as a heading and `note` as a
/// muted sub-line, centered within the flat obsidian detail pane. Reuses the theme fonts
/// and silver palette so the placeholder matches the design system (no new widget).
pub fn section_placeholder(ui: &mut egui::Ui, title: &str, note: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(24.0);
        ui.label(
            egui::RichText::new(title)
                .font(font::title1())
                .color(color::SILVER_LIGHT),
        );
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(note)
                .font(font::callout())
                .color(color::SILVER_MUTED),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn title_matches_nav_tab_label() {
        // The detail heading reuses the sidebar label verbatim (requirement 1.4).
        for tab in NavTab::ALL {
            assert_eq!(section_title(tab), tab.title());
        }
    }

    #[test]
    fn every_nav_tab_has_a_nonempty_note() {
        // The router covers all 13 sections; each note is non-empty.
        for tab in NavTab::ALL {
            assert!(
                !section_note(tab).is_empty(),
                "{tab:?} has an empty placeholder note"
            );
        }
    }

    #[test]
    fn section_notes_are_distinct() {
        // Each section gets its own distinct note so placeholders are not confusable.
        let mut seen = HashSet::new();
        for tab in NavTab::ALL {
            assert!(
                seen.insert(section_note(tab)),
                "{tab:?} shares its note with another section"
            );
        }
        assert_eq!(seen.len(), NavTab::ALL.len());
    }

    /// Drive the detail router through every one of the 13 sections in a headless egui
    /// frame. egui lays out and paints during `Context::run`, so an out-of-range rect, an
    /// unknown font family, or a bad shape index would panic here. This verifies the
    /// router renders each section's placeholder without a live window.
    #[test]
    fn detail_router_renders_in_headless_frame_for_every_tab() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);

        for tab in NavTab::ALL {
            let mut state = AppState::default();
            state.nav_tab = tab;
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render(ui, &mut state);
                });
            });
        }
    }
}
