//! Pure types/logic behind `CleanEjectSheet`, ported from
//! `Sources/WalkmanStudio/Views/CleanEjectSheet.swift`. The rendering itself lives in
//! `iced_ui::sheets::clean_eject` (the egui `render` this module used to carry was
//! removed once the iced migration reached parity — see Milestone 8 of the
//! egui-to-iced migration plan).

use walkman_services::clean_eject::CleanAndEjectResult;

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
}
