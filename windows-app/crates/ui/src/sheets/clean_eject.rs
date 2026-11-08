//! `CleanEjectSheet`, ported from `Sources/WalkmanStudio/Views/CleanEjectSheet.swift`.
//!
//! Spec task **17.7** (requirement 16.3): a confirm step, then a progress display while the
//! real junk-removal + eject sequence (`walkman_services::clean_eject::clean_and_eject`,
//! task 9.4) runs, then a result summary (files/folders cleaned, or the eject error).
//!
//! This sheet only renders state and emits `StartCleanAndEject`/`Close`; running the actual
//! sequence (which needs a real `Ejector`, task 6.5) and writing the result back into
//! [`CleanEjectSheetState::result`] is task 18's job.

use eframe::egui;

use walkman_services::clean_eject::CleanAndEjectResult;

use crate::theme::{button, color, font};

#[derive(Debug, Clone, PartialEq)]
pub enum CleanEjectCommand {
    StartCleanAndEject { mount_path: String },
    Close,
}

#[derive(Debug, Clone, Default)]
pub struct CleanEjectSheetState {
    pub open: bool,
    pub mount_path: String,
    pub drive_label: String,
    pub is_running: bool,
    pub result: Option<Result<CleanAndEjectResult, String>>,
    pub commands: Vec<CleanEjectCommand>,
}

impl CleanEjectSheetState {
    pub fn open_for(&mut self, mount_path: &str, drive_label: &str) {
        *self = Self { open: true, mount_path: mount_path.to_owned(), drive_label: drive_label.to_owned(), ..Self::default() };
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}

pub fn render(ctx: &egui::Context, state: &mut CleanEjectSheetState) {
    if !state.open {
        return;
    }
    crate::sheets::modal(ctx, "clean_eject_sheet", egui::vec2(420.0, 240.0), |ui| {
        ui.label(egui::RichText::new("Clean & Eject").font(font::title2()).color(color::SILVER_LIGHT));
        ui.add_space(10.0);

        match &state.result {
            None if state.is_running => {
                ui.label(egui::RichText::new(format!("Cleaning and ejecting {}\u{2026}", state.drive_label)).font(font::callout()).color(color::SILVER_MUTED));
                ui.add_space(10.0);
                ui.add(egui::widgets::Spinner::new());
            }
            None => {
                ui.label(
                    egui::RichText::new(format!("Remove junk files and safely eject {}?", state.drive_label))
                        .font(font::callout())
                        .color(color::SILVER_MUTED),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(button::Button::new("Clean & Eject", button::Variant::Prominent)).clicked() {
                        state.is_running = true;
                        state.commands.push(CleanEjectCommand::StartCleanAndEject { mount_path: state.mount_path.clone() });
                    }
                    if ui.add(button::Button::new("Cancel", button::Variant::Glass)).clicked() {
                        state.commands.push(CleanEjectCommand::Close);
                        state.close();
                    }
                });
            }
            Some(Ok(result)) => {
                ui.label(egui::RichText::new("Safe to unplug.").font(font::callout()).color(color::SILVER_LIGHT));
                ui.label(
                    egui::RichText::new(format!("Removed {} junk file(s), {} folder(s).", result.cleaned.files_removed, result.cleaned.folders_removed))
                        .font(font::caption())
                        .color(color::SILVER_MUTED),
                );
                ui.add_space(12.0);
                if ui.add(button::Button::new("Done", button::Variant::Prominent)).clicked() {
                    state.commands.push(CleanEjectCommand::Close);
                    state.close();
                }
            }
            Some(Err(message)) => {
                ui.label(egui::RichText::new("Eject Failed").font(font::callout()).color(color::HIRES_GOLD));
                ui.label(egui::RichText::new(message).font(font::caption()).color(color::SILVER_MUTED));
                ui.add_space(12.0);
                if ui.add(button::Button::new("Close", button::Variant::Glass)).clicked() {
                    state.commands.push(CleanEjectCommand::Close);
                    state.close();
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_for_resets_a_previous_runs_result() {
        let mut state = CleanEjectSheetState::default();
        state.result = Some(Err("stale".into()));
        state.is_running = true;

        state.open_for("E:\\", "Walkman");

        assert!(state.open);
        assert_eq!(state.mount_path, "E:\\");
        assert_eq!(state.drive_label, "Walkman");
        assert!(state.result.is_none());
        assert!(!state.is_running);
    }

    #[test]
    fn clean_eject_sheet_renders_in_headless_frame_for_every_state() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);

        let mut closed = CleanEjectSheetState::default();
        let mut confirming = CleanEjectSheetState::default();
        confirming.open_for("E:\\", "Walkman");
        let mut running = CleanEjectSheetState::default();
        running.open_for("E:\\", "Walkman");
        running.is_running = true;
        let mut succeeded = CleanEjectSheetState::default();
        succeeded.open_for("E:\\", "Walkman");
        succeeded.result = Some(Ok(CleanAndEjectResult { cleaned: walkman_services::clean_eject::CleanSummary { files_removed: 3, folders_removed: 1 } }));
        let mut failed = CleanEjectSheetState::default();
        failed.open_for("E:\\", "Walkman");
        failed.result = Some(Err("volume is busy or in use".into()));

        for state in [&mut closed, &mut confirming, &mut running, &mut succeeded, &mut failed] {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |_ui| {});
                render(ctx, state);
            });
        }
    }
}
