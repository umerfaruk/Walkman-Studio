//! Pure types/logic behind `AboutWalkmanStudioView`, ported from
//! `Sources/WalkmanStudio/Views/AboutWalkmanStudioView.swift`. The rendering itself
//! lives in `iced_ui::sheets::about` (the egui `render` this module used to carry was
//! removed once the iced migration reached parity — see Milestone 8 of the
//! egui-to-iced migration plan).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AboutTab {
    #[default]
    Hardware,
    SupportedDaps,
    EngineRules,
    DeveloperSupport,
}

impl AboutTab {
    pub const ALL: [AboutTab; 4] = [AboutTab::Hardware, AboutTab::SupportedDaps, AboutTab::EngineRules, AboutTab::DeveloperSupport];

    pub fn title(self) -> &'static str {
        match self {
            AboutTab::Hardware => "Hardware",
            AboutTab::SupportedDaps => "Supported DAPs",
            AboutTab::EngineRules => "Engine Rules",
            AboutTab::DeveloperSupport => "Developer & Support",
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AboutSheetState {
    pub open: bool,
    pub tab: AboutTab,
}

impl AboutSheetState {
    pub fn open(&mut self) {
        *self = Self { open: true, tab: AboutTab::default() };
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_resets_to_the_hardware_tab() {
        let mut state = AboutSheetState { open: false, tab: AboutTab::DeveloperSupport };
        state.open();
        assert!(state.open);
        assert_eq!(state.tab, AboutTab::Hardware);
    }
}
