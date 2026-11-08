//! `TransferQueueView`, ported from `Sources/WalkmanStudio/Views/TransferQueueView.swift`.
//!
//! Spec task **17.1** (requirements 9.1, 9.5, 9.6): the drag-and-drop batch queue's list UI
//! — status per row, expandable rows, a per-item editor (task 16.4), a preview context
//! menu, the duplicate-import guard sheet (Skip Existing/Overwrite/Keep Both), a clear-queue
//! confirm, and `TransferSyncSheet` progress — over the real `walkman_services::
//! transfer_queue`/`walkman_domain::models::TransferItem` built in task 10.2.
//!
//! ## Honest simplification
//! Drag-and-drop enqueueing isn't wired (egui has no native OS file-drop affordance this
//! crate uses yet, same caveat as every other import path since task 16.2); this view only
//! renders whatever `TransferItem`s it's handed and emits commands — actually starting a
//! transfer (`transfer_queue::transfer_batch_async`), running the duplicate guard
//! (`duplicate_guard::check_batch_for_duplicates`) first, and streaming `TransferProgress`
//! back into [`TransferQueueViewState::sheet`] is task 18's job.

use eframe::egui;

use walkman_domain::models::{TransferItem, TransferStatus, TransferTargetMode};
use walkman_services::duplicate_guard::DuplicateResolution;
use walkman_services::transcode::Preset;

use crate::selection;
use crate::theme::{button, color, font, glass, sizing};

/// The curated preset choices shown in the toolbar, in the order R9.3 lists them: the 4
/// "at minimum" presets first, then the rest the engine recognizes.
pub const PRESET_CHOICES: [Preset; 16] = [
    Preset::Copy,
    Preset::Flac16_44_1,
    Preset::Flac24_96,
    Preset::Mp3_320,
    Preset::Flac24_48,
    Preset::Flac24_192,
    Preset::Mp3_128,
    Preset::Mp3_192,
    Preset::Mp3_256,
    Preset::Mp3V0,
    Preset::Aac128,
    Preset::Aac256,
    Preset::Aac320,
    Preset::Alac16,
    Preset::Wav16,
    Preset::CopyOptimize,
];

/// Display label for a preset (R9.3's own naming), pure so the toolbar's combo box and
/// tests share one source of truth.
pub fn preset_label(preset: Preset) -> &'static str {
    match preset {
        Preset::Copy => "Bit-Perfect Copy",
        Preset::CopyOptimize => "Bit-Perfect Copy (Optimize Artwork)",
        Preset::CopyRaw => "Raw Copy",
        Preset::Mp3_128 => "MP3 128 kbps",
        Preset::Mp3_192 => "MP3 192 kbps",
        Preset::Mp3_256 => "MP3 256 kbps",
        Preset::Mp3_320 => "MP3 320 kbps",
        Preset::Mp3V0 => "MP3 V0",
        Preset::Aac128 => "AAC 128 kbps",
        Preset::Aac256 => "AAC 256 kbps",
        Preset::Aac320 => "AAC 320 kbps",
        Preset::Flac16_44_1 => "FLAC 16-bit / 44.1 kHz",
        Preset::Flac24_48 => "FLAC 24-bit / 48 kHz",
        Preset::Flac24_96 => "FLAC 24-bit / 96 kHz",
        Preset::Flac24_192 => "FLAC 24-bit / 192 kHz",
        Preset::Alac16 => "ALAC 16-bit",
        Preset::Wav16 => "WAV 16-bit",
    }
}

/// Short status label for a queue row, matching the per-item editor's own vocabulary
/// ([`crate::sheets::transfer_item::status_label`] — re-exported here as the same pure
/// function so the row list and the sheet never drift apart).
pub use crate::sheets::transfer_item::status_label;

#[derive(Debug, Clone, PartialEq)]
pub enum TransferQueueCommand {
    Remove(String),
    ClearAll,
    ToggleExpanded(String),
    OpenEditor(String),
    Preview(String),
    StartTransfer { item_ids: Vec<String>, target_mode: TransferTargetMode, preset: Preset, loudnorm: bool },
    ResolveDuplicates(DuplicateResolution),
    CancelTransfer,
}

/// Which modal is currently up over the queue. Populated/cleared by task 18 for
/// `DuplicateGuard`/`Progress` (this view only renders what it's told); `ClearConfirm` is
/// fully local since it needs no service round trip.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TransferQueueSheet {
    #[default]
    None,
    ClearConfirm,
    /// R9.5: the duplicate-import guard, shown when `duplicate_guard::
    /// check_batch_for_duplicates` found `duplicate_count` items already on the target.
    DuplicateGuard { duplicate_count: usize },
    /// R9.6's `TransferSyncSheet`, kept in sync with `transfer_queue::TransferProgress`.
    Progress { completed: usize, total: usize, current_title: String },
}

#[derive(Debug, Clone)]
pub struct TransferQueueViewState {
    pub target_mode: TransferTargetMode,
    pub preset: Preset,
    pub loudnorm: bool,
    pub selected: Vec<String>,
    pub selection_anchor: Option<usize>,
    pub sheet: TransferQueueSheet,
    pub commands: Vec<TransferQueueCommand>,
}

impl Default for TransferQueueViewState {
    fn default() -> Self {
        Self {
            target_mode: TransferTargetMode::Music,
            preset: Preset::Copy,
            loudnorm: false,
            selected: Vec::new(),
            selection_anchor: None,
            sheet: TransferQueueSheet::default(),
            commands: Vec::new(),
        }
    }
}

pub fn render(ui: &mut egui::Ui, items: &[TransferItem], state: &mut TransferQueueViewState) {
    render_toolbar(ui, items, state);
    ui.add_space(8.0);

    if items.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(egui::RichText::new("No items queued").font(font::callout()).color(color::SILVER_MUTED));
        });
    } else {
        render_list(ui, items, state);
    }

    render_sheet(ui, items, state);
}

fn render_toolbar(ui: &mut egui::Ui, items: &[TransferItem], state: &mut TransferQueueViewState) {
    ui.horizontal(|ui| {
        for mode in TransferTargetMode::ALL {
            let variant = if state.target_mode == mode { button::Variant::Capsule } else { button::Variant::Glass };
            if ui.add(button::Button::new(mode.title(), variant)).clicked() {
                state.target_mode = mode;
            }
        }

        ui.add_space(12.0);
        egui::ComboBox::from_id_salt("transfer_preset")
            .selected_text(preset_label(state.preset))
            .show_ui(ui, |ui| {
                for preset in PRESET_CHOICES {
                    ui.selectable_value(&mut state.preset, preset, preset_label(preset));
                }
            });

        ui.add_space(12.0);
        ui.checkbox(&mut state.loudnorm, "Loudness Normalize");

        ui.add_space(12.0);
        if ui.add(button::Button::new("Start Transfer", button::Variant::Prominent).enabled(!items.is_empty())).clicked() {
            state.commands.push(TransferQueueCommand::StartTransfer {
                item_ids: items.iter().map(|i| i.id.clone()).collect(),
                target_mode: state.target_mode,
                preset: state.preset,
                loudnorm: state.loudnorm,
            });
        }
        ui.add_space(8.0);
        if ui.add(button::Button::new("Clear Queue", button::Variant::Destructive).enabled(!items.is_empty())).clicked() {
            state.sheet = TransferQueueSheet::ClearConfirm;
        }
    });
}

fn render_list(ui: &mut egui::Ui, items: &[TransferItem], state: &mut TransferQueueViewState) {
    let all_ids: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (index, item) in items.iter().enumerate() {
            render_row(ui, index, item, &all_ids, state);
        }
        ui.add_space(sizing::TABLE_BOTTOM_INSET);
    });
}

fn render_row(ui: &mut egui::Ui, index: usize, item: &TransferItem, all_ids: &[String], state: &mut TransferQueueViewState) {
    let selected = state.selected.contains(&item.id);
    let glass_state = if selected { glass::State::Selected } else { glass::State::Normal };

    let response = glass::liquid_glass(glass_state).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            let chevron = if item.is_expanded { "\u{25BE}" } else { "\u{25B8}" };
            if ui.add(button::Button::new(chevron, button::Variant::Glass)).clicked() {
                state.commands.push(TransferQueueCommand::ToggleExpanded(item.id.clone()));
            }

            ui.add_space(8.0);
            ui.label(egui::RichText::new(status_label(&item.status)).font(font::caption_bold()).color(color::SILVER_MUTED));

            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(&item.title).font(font::body_medium()).color(color::SILVER_LIGHT));
                ui.label(egui::RichText::new(&item.artist).font(font::caption()).color(color::SILVER_MUTED));
            });

            if item.is_duplicate_on_walkman {
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Duplicate").font(font::caption_bold()).color(color::HIRES_GOLD));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(button::Button::new("Remove", button::Variant::Glass)).clicked() {
                    state.commands.push(TransferQueueCommand::Remove(item.id.clone()));
                }
                ui.add_space(6.0);
                if ui.add(button::Button::new("Edit\u{2026}", button::Variant::Glass)).clicked() {
                    state.commands.push(TransferQueueCommand::OpenEditor(item.id.clone()));
                }
            });
        });

        if item.is_expanded {
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!(
                    "{} \u{2022} {} kbps \u{2022} {:.1} kHz \u{2022} {}-bit",
                    item.format,
                    item.bitrate_kbps,
                    item.sample_rate as f64 / 1000.0,
                    item.bits_per_sample
                ))
                .font(font::caption())
                .color(color::SILVER_MUTED),
            );
            if let Some(info) = &item.duplicate_match_info {
                ui.label(egui::RichText::new(info).font(font::caption()).color(color::SILVER_MUTED));
            }
        }
    });

    let row_response = response.response.interact(egui::Sense::click());
    if row_response.clicked() {
        let modifiers = ui.input(|i| i.modifiers);
        selection::handle_click(&mut state.selected, &mut state.selection_anchor, all_ids, index, &item.id, modifiers.shift, modifiers.command || modifiers.ctrl);
    }
    row_response.context_menu(|ui| {
        if ui.button("Preview").clicked() {
            state.commands.push(TransferQueueCommand::Preview(item.id.clone()));
            ui.close_menu();
        }
        if ui.button("Edit\u{2026}").clicked() {
            state.commands.push(TransferQueueCommand::OpenEditor(item.id.clone()));
            ui.close_menu();
        }
        if ui.button("Remove").clicked() {
            state.commands.push(TransferQueueCommand::Remove(item.id.clone()));
            ui.close_menu();
        }
    });
}

fn render_sheet(ui: &mut egui::Ui, items: &[TransferItem], state: &mut TransferQueueViewState) {
    let ctx = ui.ctx().clone();
    match state.sheet.clone() {
        TransferQueueSheet::None => {}
        TransferQueueSheet::ClearConfirm => {
            crate::sheets::modal(&ctx, "transfer_clear_confirm_sheet", egui::vec2(420.0, 200.0), |ui| {
                ui.label(egui::RichText::new("Clear Queue?").font(font::title2()).color(color::SILVER_LIGHT));
                ui.label(
                    egui::RichText::new(format!("Removes all {} queued item(s). This can't be undone.", items.len()))
                        .font(font::callout())
                        .color(color::SILVER_MUTED),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(button::Button::new("Clear Queue", button::Variant::Destructive)).clicked() {
                        state.commands.push(TransferQueueCommand::ClearAll);
                        state.sheet = TransferQueueSheet::None;
                    }
                    if ui.add(button::Button::new("Cancel", button::Variant::Glass)).clicked() {
                        state.sheet = TransferQueueSheet::None;
                    }
                });
            });
        }
        TransferQueueSheet::DuplicateGuard { duplicate_count } => {
            crate::sheets::modal(&ctx, "transfer_duplicate_guard_sheet", egui::vec2(460.0, 240.0), |ui| {
                ui.label(egui::RichText::new("Duplicates Found").font(font::title2()).color(color::SILVER_LIGHT));
                ui.label(
                    egui::RichText::new(format!("{duplicate_count} item(s) already exist on the target. Choose how to handle them."))
                        .font(font::callout())
                        .color(color::SILVER_MUTED),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(button::Button::new("Skip Existing", button::Variant::Glass)).clicked() {
                        state.commands.push(TransferQueueCommand::ResolveDuplicates(DuplicateResolution::SkipExisting));
                        state.sheet = TransferQueueSheet::None;
                    }
                    if ui.add(button::Button::new("Overwrite", button::Variant::Destructive)).clicked() {
                        state.commands.push(TransferQueueCommand::ResolveDuplicates(DuplicateResolution::Overwrite));
                        state.sheet = TransferQueueSheet::None;
                    }
                    if ui.add(button::Button::new("Keep Both", button::Variant::Prominent)).clicked() {
                        state.commands.push(TransferQueueCommand::ResolveDuplicates(DuplicateResolution::KeepBoth));
                        state.sheet = TransferQueueSheet::None;
                    }
                });
            });
        }
        TransferQueueSheet::Progress { completed, total, current_title } => {
            crate::sheets::modal(&ctx, "transfer_sync_sheet", egui::vec2(460.0, 200.0), |ui| {
                ui.label(egui::RichText::new("Transferring\u{2026}").font(font::title2()).color(color::SILVER_LIGHT));
                ui.label(egui::RichText::new(&current_title).font(font::callout()).color(color::SILVER_MUTED));
                ui.add_space(10.0);
                let fraction = if total == 0 { 0.0 } else { completed as f32 / total as f32 };
                ui.add(egui::ProgressBar::new(fraction).text(format!("{completed} / {total}")));
                ui.add_space(12.0);
                if ui.add(button::Button::new("Cancel", button::Variant::Glass)).clicked() {
                    state.commands.push(TransferQueueCommand::CancelTransfer);
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn sample_item(id: &str, expanded: bool) -> TransferItem {
        TransferItem {
            id: id.into(),
            source_path: format!("D:/Downloads/{id}.flac"),
            status: TransferStatus::Pending,
            title: format!("Title {id}"),
            artist: "Artist".into(),
            album: "Album".into(),
            track_number: "1".into(),
            year: "2024".into(),
            genre: "Pop".into(),
            duration_seconds: 180.0,
            bitrate_kbps: 900,
            sample_rate: 44_100,
            bits_per_sample: 16,
            format: "FLAC".into(),
            size_bytes: 1,
            artwork_data: None,
            is_artwork_optimized: false,
            lyrics: None,
            is_lyrics_synced: false,
            is_fetching_online: false,
            is_optimizing: false,
            is_expanded: expanded,
            is_duplicate_on_walkman: false,
            duplicate_match_info: None,
            duplicate_matched_track: None,
        }
    }

    // ---- Pure helpers ----

    #[test]
    fn preset_label_covers_every_choice_distinctly() {
        let mut seen = HashSet::new();
        for preset in PRESET_CHOICES {
            assert!(seen.insert(preset_label(preset)), "duplicate label for {preset:?}");
        }
    }

    #[test]
    fn status_label_matches_the_sheets_module() {
        // Re-exported from `sheets::transfer_item` so the row list and the editor sheet
        // never drift; this just pins that the re-export is live.
        assert_eq!(status_label(&TransferStatus::Success), "Success");
    }

    // ---- Default state ----

    #[test]
    fn default_state_starts_on_music_target_with_bit_perfect_copy() {
        let state = TransferQueueViewState::default();
        assert_eq!(state.target_mode, TransferTargetMode::Music);
        assert_eq!(state.preset, Preset::Copy);
        assert!(!state.loudnorm);
        assert_eq!(state.sheet, TransferQueueSheet::None);
    }

    // ---- Headless render smoke test ----

    /// Drives the view through a headless egui frame for an empty queue, a populated queue
    /// with one expanded row, and every sheet variant — egui lays out and paints during
    /// `Context::run`, so a bad rect/font/widget would panic here.
    #[test]
    fn transfer_queue_renders_in_headless_frame_for_every_state() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);

        let items = vec![sample_item("a", false), sample_item("b", true)];
        let sheets = [
            TransferQueueSheet::None,
            TransferQueueSheet::ClearConfirm,
            TransferQueueSheet::DuplicateGuard { duplicate_count: 2 },
            TransferQueueSheet::Progress { completed: 1, total: 2, current_title: "Title b".into() },
        ];

        for (items, sheet) in [(Vec::new(), TransferQueueSheet::None)].into_iter().chain(sheets.into_iter().map(|s| (items.clone(), s))) {
            let mut state = TransferQueueViewState { sheet, ..TransferQueueViewState::default() };
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render(ui, &items, &mut state);
                });
            });
        }
    }
}
