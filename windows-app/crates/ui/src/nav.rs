//! Navigation sections. Ported from `SidebarView.NavigationTab`
//! (`Sources/WalkmanStudio/Views/SidebarView.swift`).

/// The 13 top-level navigation sections, in sidebar order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavTab {
    Overview,
    LocalLibrary,
    SenseMe,
    Library,
    RecentlyAdded,
    Albums,
    Artists,
    HiRes,
    LanguageStudy,
    Playlists,
    Duplicates,
    Transfer,
    BackupRestore,
}

impl NavTab {
    pub const ALL: [NavTab; 13] = [
        NavTab::Overview,
        NavTab::LocalLibrary,
        NavTab::SenseMe,
        NavTab::Library,
        NavTab::RecentlyAdded,
        NavTab::Albums,
        NavTab::Artists,
        NavTab::HiRes,
        NavTab::LanguageStudy,
        NavTab::Playlists,
        NavTab::Duplicates,
        NavTab::Transfer,
        NavTab::BackupRestore,
    ];

    /// Section title, matching the macOS `rawValue`.
    pub fn title(self) -> &'static str {
        match self {
            NavTab::Overview => "Device Overview",
            NavTab::LocalLibrary => "Local Music Library",
            NavTab::SenseMe => "SenseMe\u{2122} Channels",
            NavTab::Library => "All Songs",
            NavTab::RecentlyAdded => "Recently Added",
            NavTab::Albums => "Albums",
            NavTab::Artists => "Artists",
            NavTab::HiRes => "Hi-Res Audio",
            NavTab::LanguageStudy => "Language Study",
            NavTab::Playlists => "Playlists",
            NavTab::Duplicates => "Duplicates",
            NavTab::Transfer => "Transfer & Convert",
            NavTab::BackupRestore => "Backup & Restore",
        }
    }

    /// The macOS SF Symbol name. The UI maps this to a bundled glyph (task 3.4).
    pub fn sf_symbol(self) -> &'static str {
        match self {
            NavTab::Overview => "hifispeaker.fill",
            NavTab::LocalLibrary => "folder.badge.gearshape",
            NavTab::SenseMe => "waveform.path.ecg.rectangle.fill",
            NavTab::Library => "music.note.list",
            NavTab::RecentlyAdded => "clock.arrow.circlepath",
            NavTab::Albums => "square.stack.fill",
            NavTab::Artists => "music.mic",
            NavTab::HiRes => "waveform.badge.magnifyingglass",
            NavTab::LanguageStudy => "character.book.closed.fill",
            NavTab::Playlists => "list.bullet.rectangle.portrait.fill",
            NavTab::Duplicates => "doc.on.doc.fill",
            NavTab::Transfer => "arrow.up.arrow.down.circle.fill",
            NavTab::BackupRestore => "arrow.triangle.2.circlepath.circle.fill",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thirteen_sections() {
        assert_eq!(NavTab::ALL.len(), 13);
        assert_eq!(NavTab::SenseMe.title(), "SenseMe\u{2122} Channels");
        assert_eq!(NavTab::Transfer.title(), "Transfer & Convert");
    }
}
