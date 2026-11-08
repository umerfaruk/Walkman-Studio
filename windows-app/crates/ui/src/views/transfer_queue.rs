//! Pure types/logic behind `TransferQueueView`, ported from
//! `Sources/WalkmanStudio/Views/TransferQueueView.swift`. The rendering itself lives in
//! `iced_ui::views::transfer_queue` (the egui `render` this module used to carry was
//! removed once the iced migration reached parity — see Milestone 8 of the
//! egui-to-iced migration plan).

use walkman_domain::models::TransferTargetMode;
use walkman_services::duplicate_guard::DuplicateResolution;
use walkman_services::transcode::Preset;

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

/// Display label for a preset (R9.3's own naming), pure so the toolbar's picker and
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

/// Which modal is currently up over the queue. `ClearConfirm` is fully local since it
/// needs no service round trip.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

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
        assert_eq!(status_label(&walkman_domain::models::TransferStatus::Success), "Success");
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
}
