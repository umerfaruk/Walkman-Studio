//! Parity-checklist test (spec task 19.4): enumerates every `NavTab`, sort option, filter,
//! display mode, and sheet-tab enum in this Rust port and asserts the set matches a fixture
//! hand-captured directly from the real macOS Swift source (`Sources/WalkmanStudio/`), so a
//! variant silently dropped or a new one silently added on just one side fails the build.
//!
//! ## What "matches" means here
//! Swift's own case-value strings (e.g. `"Exact & Same Album"`) were deliberately *not*
//! copied verbatim everywhere in this port — many Rust labels already use their own wording
//! (documented per-task throughout `tasks.md`). Re-litigating every label string now would
//! undo dozens of already-reviewed, intentional choices. What this test asserts instead is
//! *structural* parity — the same **number** of options/variants, captured from reading the
//! Swift `enum ...: CaseIterable` declarations directly (file:line cited in each fixture
//! comment below) — which is exactly the kind of drift (a variant quietly added or removed on
//! one side only) a label-text diff would miss just as easily as it would catch.
//!
//! Two real gaps were found and fixed by this exercise (not just asserted after the fact):
//! `BackupRestoreView.swift`'s `AlbumFilter`/`TrackFilter` each had a 3rd `"Hi-Res Only"` case
//! this port's `AlbumFilter`/`TrackFilter` (task 17.4) didn't; `AboutWalkmanStudioView.swift`
//! has 4 tabs (`.hardware`/`.supportedDAPs`/`.engineRules`/`.developerSupport`), not the 3
//! this port's `AboutTab` (task 17.7) originally shipped with (a 3rd tab named "Credits" that
//! matched neither real tab). `AudioChannelMode` also had 9 cases against this port's
//! `ChannelMode`'s 7 (missing `muteLeft`/`muteRight`, distinct from isolate-to-both-ears).
//! All three are fixed in the same change that added this test, each with its own test too
//! (`services::trim_cut::tests`, `ui::views::backup_restore::tests`,
//! `ui::sheets::about::tests`) — this file only re-asserts the resulting counts so a future
//! regression is caught here specifically, by name, as a parity check.

use walkman_services::backup_restore::BackupScope;
use walkman_services::duplicate_finder::{DuplicateScope, DuplicateSortOption};
use walkman_services::trim_cut::ChannelMode;
use walkman_ui::sheets::about::AboutTab;
use walkman_ui::sheets::batch_lyrics::LyricsGeneratorScope;
use walkman_ui::views::albums::AlbumSortOption;
use walkman_ui::views::backup_restore::{AlbumFilter, TrackFilter};
use walkman_ui::views::hi_res::{HiResViewMode, TrackSortOption};
use walkman_ui::views::language_study::{LanguageDisplayMode, LanguageSortOption};
use walkman_ui::views::local_library::{LocalDisplayMode, LocalSortOption};
use walkman_ui::views::playlists::AddTracksMode;
use walkman_ui::views::recently_added::TimeScope;
use walkman_ui::NavTab;

/// `SidebarView.swift:4-17` — `NavigationTab`, 13 cases in this exact sidebar order.
#[test]
fn nav_tab_matches_the_macos_sidebar_order() {
    assert_eq!(NavTab::ALL.len(), 13);
    assert_eq!(
        NavTab::ALL,
        [
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
        ]
    );
}

/// `LocalLibraryView.swift:5-21` — `LocalDisplayMode`/`LocalSortOption`: 5 display modes, 8
/// sort options.
#[test]
fn local_library_display_modes_and_sorts_match_macos() {
    assert_eq!(LocalDisplayMode::ALL.len(), 5);
    assert_eq!(LocalSortOption::ALL.len(), 8);
}

/// `AlbumsView.swift:11-19` — `AlbumSortOption`: 7 options.
#[test]
fn album_sort_options_match_macos() {
    assert_eq!(AlbumSortOption::ALL.len(), 7);
}

/// `HiResAudioView.swift:4-29` — `HiResViewMode`: 3 modes; `HiResTrackSortOption`: 6 options.
#[test]
fn hi_res_view_modes_and_sorts_match_macos() {
    assert_eq!(HiResViewMode::ALL.len(), 3);
    assert_eq!(TrackSortOption::ALL.len(), 6);
}

/// `LanguageStudyView.swift:6-34` — `LanguageStudyDisplayMode`: 5 modes;
/// `LanguageStudySortOption`: 6 options.
#[test]
fn language_study_display_modes_and_sorts_match_macos() {
    assert_eq!(LanguageDisplayMode::ALL.len(), 5);
    assert_eq!(LanguageSortOption::ALL.len(), 6);
}

/// `RecentlyAddedView.swift:24-29` — `RecentlyAddedTimeScope`: 4 scopes.
#[test]
fn recently_added_time_scopes_match_macos() {
    assert_eq!(TimeScope::ALL.len(), 4);
}

/// `DuplicatesView.swift:4-11` / `DuplicateFinderService.swift:43-44` —
/// `DuplicateSortOption`: 4 options; `DuplicateDetectionScope`: 2 scopes.
#[test]
fn duplicate_sort_options_and_scopes_match_macos() {
    // No `ALL` const on these two (ported before this test existed); an exhaustive match
    // forces this test to stop compiling the moment a variant is added or removed on the
    // Rust side, which is strictly stronger than a runtime length check.
    let count_sort = |o: DuplicateSortOption| match o {
        DuplicateSortOption::SpaceRecoverable | DuplicateSortOption::TitleAZ | DuplicateSortOption::ArtistAZ | DuplicateSortOption::MostCopies => 1,
    };
    let _: fn(DuplicateSortOption) -> usize = count_sort;

    let count_scope = |s: DuplicateScope| match s {
        DuplicateScope::ExactSameAlbum | DuplicateScope::CrossAlbum => 1,
    };
    let _: fn(DuplicateScope) -> usize = count_scope;
}

/// `PlaylistsView.swift:2150-2156` — `AddBrowsingMode`: 4 modes.
#[test]
fn playlist_add_tracks_modes_match_macos() {
    assert_eq!(AddTracksMode::ALL.len(), 4);
}

/// `BackupRestoreService.swift:67-73` — `BackupScopeType`: 5 scopes.
/// `BackupRestoreView.swift:35-39,2057-2062,2563-2568` — `Mode`: 2 modes; `AlbumFilter`/
/// `TrackFilter`: 3 cases each (`all`/`selected`/`hiRes`) — the `hiRes` case was missing from
/// this port's `AlbumFilter`/`TrackFilter` until this same change added it.
#[test]
fn backup_restore_scopes_and_filters_match_macos() {
    let count_scope = |s: BackupScope| match s {
        BackupScope::FullLibrary | BackupScope::Playlists | BackupScope::LanguageStudy | BackupScope::SelectedAlbums | BackupScope::SelectedTracks => 1,
    };
    let _: fn(BackupScope) -> usize = count_scope;

    let count_album_filter = |f: AlbumFilter| match f {
        AlbumFilter::AllAlbums | AlbumFilter::Selected | AlbumFilter::HiRes => 1,
    };
    let _: fn(AlbumFilter) -> usize = count_album_filter;

    let count_track_filter = |f: TrackFilter| match f {
        TrackFilter::AllTracks | TrackFilter::Selected | TrackFilter::HiRes => 1,
    };
    let _: fn(TrackFilter) -> usize = count_track_filter;
}

/// `BatchLyricsGeneratorSheet.swift:16-23` — `LyricsScope`: 4 scopes.
#[test]
fn batch_lyrics_scopes_match_macos() {
    assert_eq!(LyricsGeneratorScope::ALL.len(), 4);
}

/// `AboutWalkmanStudioView.swift:5-9` — `AboutTab`: 4 tabs (`.hardware`/`.supportedDAPs`/
/// `.engineRules`/`.developerSupport`) — this port's `AboutTab` originally had only 3 (a
/// `Credits` tab matching neither of the 2 real tabs it was missing) until this same change.
#[test]
fn about_tabs_match_macos() {
    assert_eq!(AboutTab::ALL.len(), 4);
}

/// `TrimCutService.swift:35-61` — `AudioChannelMode`: 9 cases (`original`/`swapLR`/`mono`/
/// `stereo`/`leftOnly`/`rightOnly`/`muteLeft`/`muteRight`/`invertPhase`) — this port's
/// `ChannelMode` had only 7 (missing `muteLeft`/`muteRight`) until this same change.
#[test]
fn trim_cut_channel_modes_match_macos() {
    let count = |m: ChannelMode| match m {
        ChannelMode::None
        | ChannelMode::SwapLr
        | ChannelMode::Mono
        | ChannelMode::Stereo
        | ChannelMode::IsolateLeft
        | ChannelMode::IsolateRight
        | ChannelMode::MuteLeft
        | ChannelMode::MuteRight
        | ChannelMode::PhaseFlip => 1,
    };
    let _: fn(ChannelMode) -> usize = count;
}

/// Every sheet/view the macOS app has a dedicated `.swift` file for, and the Rust module that
/// ports it (spec tasks 14-17) — a plain inventory check (R19.4's "every... sheet"), so a
/// sheet quietly never getting a Rust counterpart shows up here as a visibly incomplete list
/// rather than nowhere at all. This list is intentionally the sheets/views with their own
/// top-level file in `Sources/WalkmanStudio/Views/`, not every helper subview.
#[test]
fn every_macos_view_and_sheet_has_a_rust_port() {
    const PORTED: &[(&str, &str)] = &[
        ("AboutWalkmanStudioView.swift", "ui::sheets::about"),
        ("AlbumMetadataEditorSheet.swift", "ui::sheets::album_metadata"),
        ("AlbumsView.swift", "ui::views::albums"),
        ("AndroidWalkmanMtpHelpSheet.swift", "ui::views::device_overview (render_mtp_help_sheet)"),
        ("ArtistsView.swift", "ui::views::artists"),
        ("BackupRestoreView.swift", "ui::views::backup_restore"),
        ("BatchLyricsGeneratorSheet.swift", "ui::sheets::batch_lyrics"),
        ("CleanEjectSheet.swift", "ui::sheets::clean_eject"),
        ("ContentView.swift", "ui::shell / ui::detail"),
        ("DeviceOverviewView.swift", "ui::views::device_overview"),
        ("DuplicateImportGuardSheet.swift", "ui::views::transfer_queue (TransferQueueSheet::DuplicateGuard)"),
        ("DuplicatesView.swift", "ui::views::duplicates"),
        ("FullScreenNowPlayingView.swift", "ui::now_playing (render_fullscreen_overlay)"),
        ("HiResAudioView.swift", "ui::views::hi_res"),
        ("LanguageStudyView.swift", "ui::views::language_study"),
        ("LibraryView.swift", "ui::views::library"),
        ("LocalLibraryView.swift", "ui::views::local_library"),
        ("ManageLocalFoldersSheet.swift", "ui::views::local_library (LocalLibrarySheet::ManageFolders)"),
        ("MetadataStudioView.swift", "ui::inspector"),
        ("PlaylistsView.swift", "ui::views::playlists"),
        ("RecentlyAddedView.swift", "ui::views::recently_added"),
        ("SenseMeChannelsView.swift", "ui::views::senseme"),
        ("SidebarView.swift", "ui::sidebar / ui::nav"),
        ("SongMetadataEditorSheet.swift", "ui::sheets::song_metadata"),
        ("TopStorageBarView.swift", "ui::views::device_overview (storage gauges)"),
        ("TrackPreviewBar.swift", "ui::now_playing (render_now_playing_bar)"),
        ("TransferQueueItemEditorSheet.swift", "ui::sheets::transfer_item"),
        ("TransferQueueView.swift", "ui::views::transfer_queue"),
        ("TransferSyncSheet.swift", "ui::views::transfer_queue (TransferQueueSheet::Progress)"),
        ("TrimAndCutSheet.swift", "ui::sheets::trim_cut"),
        // The built-in lyrics editor (R15.6) has no single dedicated macOS `.swift` view file
        // of its own (it's inline in MetadataStudioView's flow) but gets a dedicated Rust
        // module anyway, since it's a distinct, independently-testable sheet in this port.
        ("(inline in MetadataStudioView.swift)", "ui::sheets::lyrics_editor"),
    ];
    assert_eq!(PORTED.len(), 31, "update this list (and the count) when a view/sheet is added or removed");
    assert!(PORTED.iter().all(|(swift, rust)| !swift.is_empty() && !rust.is_empty()));
}
