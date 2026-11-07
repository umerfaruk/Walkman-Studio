//! Application shell, ported from `ContentView` + `SidebarView` + `WalkmanStudioApp`
//! (`Sources/WalkmanStudio/Views/ContentView.swift`).
//!
//! Spec task 4 builds the shell incrementally:
//! - **4.1 (this file):** the three-column layout — a fixed 260pt sidebar, a flexible
//!   central detail pane, and an optional 300pt trailing inspector (with a 1px divider) —
//!   and the shared [`AppState`] whose `nav_tab` defaults to `Local Music Library`.
//! - 4.2 renders the real `NavTab` sidebar sections (visibility rules, count badges).
//! - 4.3 adds the toolbar/menus and the inspector widen-on-open window resize.
//! - 4.4 adds the floating now-playing bar + fullscreen overlay.
//! - **4.5 (landed):** the detail router (`match nav_tab`) in [`crate::detail`] renders a
//!   per-section placeholder pane; the real section views land in tasks 14–17.
//!
//! The sidebar now renders the real `NavTab` sections and the detail pane routes through
//! [`crate::detail::render`]; the inspector still draws a minimal placeholder until task
//! 16.1.

use eframe::egui;

use crate::now_playing::{self, NowPlayingPlaceholder, PlayerRequests};
use crate::sidebar::{self, DriveRow};
use crate::theme::{color, sizing};
use crate::NavTab;

/// Hairline divider width between the detail pane and the trailing inspector (the
/// design's "+1px divider"; see `design.md` "Sizing baselines").
pub const DIVIDER_WIDTH: f32 = 1.0;

/// Shared application state, sketched in `design.md` ("App state"). Only the fields the
/// shell layout (task 4.1) needs are populated here; the remaining fields (drives,
/// devices, tracks, caches, sheet state, player, …) are introduced by the tasks that use
/// them (4.2–4.5 and the service-wiring tasks). Keeping the struct minimal avoids pulling
/// in types that are still being ported (task 5 `domain/models.rs`).
pub struct AppState {
    /// The selected navigation section. Defaults to `Local Music Library` on launch
    /// (requirement 1.6).
    pub nav_tab: NavTab,
    /// Whether the trailing 300pt inspector pane is shown (requirement 1.1 — the
    /// inspector is *optional*). The Ctrl+I toggle and widen-on-open resize land in 4.3.
    pub show_inspector: bool,
    /// Id of the track shown in the inspector, if any (full-path track id). Populated by
    /// the detail views in later tasks.
    pub selected_track_id: Option<String>,

    // --- Sidebar visibility drivers (task 4.2) ---
    //
    // These are MINIMAL scalar placeholders that drive the sidebar's visibility rules and
    // count badges (requirement 1.4). The richer domain types sketched in `design.md`
    // (`Vec<WalkmanDevice>`, `Vec<WalkmanDrive>`, `Vec<Track>`, the precomputed caches)
    // replace them when the model/service ports land (tasks 5 and 9); until then these
    // scalars stand in so the sidebar is fully exercisable.
    /// Whether a Walkman is currently connected. Drives the Device-section mode, the
    /// Walkman Library group's visibility, and whether `Backup & Restore` is enabled.
    pub walkman_connected: bool,
    /// Number of attached Walkman devices. Selects the Device-section header (0/1 →
    /// `Device`, >1 → `Connected Devices (n)`). Replaced by `Vec<WalkmanDevice>` (task 5).
    pub device_count: usize,
    /// Drive rows shown under the Device section (Internal, MicroSD). Replaced by
    /// `Vec<WalkmanDrive>` (task 5).
    pub drives: Vec<DriveRow>,

    // --- Sidebar count badges (task 4.2) ---
    /// Local Library track count badge.
    pub local_track_count: usize,
    /// `All Songs` (Walkman library) track count badge.
    pub track_count: usize,
    /// `Recently Added` count badge.
    pub recently_added_count: usize,
    /// `Albums` count badge.
    pub album_count: usize,
    /// `Artists` count badge.
    pub artist_count: usize,
    /// `Hi-Res Audio` count badge (rendered as the gold HR pill).
    pub hi_res_count: usize,
    /// `Language Study` count badge.
    pub language_study_count: usize,
    /// `Duplicates` count badge.
    pub duplicate_count: usize,
    /// `Transfer & Convert` queue-count badge.
    pub queue_count: usize,

    /// Set when the bottom `About Walkman Studio` button is clicked, or when the About
    /// menu command fires (task 4.3). The real About sheet (`AboutWalkmanStudioView`) is
    /// opened off this one-shot flag in task 17.7.
    pub about_requested: bool,

    // --- Toolbar + menu command flags (task 4.3) ---
    //
    // These one-shot request flags are *raised* by the toolbar/menu/keyboard handlers and
    // *consumed* (reset) by the code that performs the real work in a later task. Keeping
    // them as plain flags here avoids pulling in the service layer before it is ported.
    /// Set when the toolbar `Refresh` button is pressed. Consumed by the device
    /// refresh/rescan wiring in task 18 (real drive detection + library reload).
    pub refresh_requested: bool,
    /// Set when the toolbar `Eject` button is pressed. Consumed by Clean & Eject
    /// (task 9.4 / 16) which flips [`is_ejecting`](AppState::is_ejecting) while it runs.
    pub eject_requested: bool,
    /// `true` while a Clean & Eject is in flight. Drives the toolbar spinner (replacing
    /// the Eject glyph) and disables the button. Set/cleared by the eject task (9.4/16);
    /// exposed here so the toolbar can reflect it.
    pub is_ejecting: bool,
    /// Set when `Manage Local Folders…` is invoked (menu command or Ctrl+Shift+L /
    /// Ctrl+Shift+O). Consumed by `ManageLocalFoldersSheet` in task 17.7.
    pub manage_folders_requested: bool,

    // --- Inspector widen-on-open resize bookkeeping (task 4.3) ---
    //
    // The window grows by the inspector width when the inspector opens and shrinks back
    // when it closes, replicating the macOS `handleInspectorVisibilityChanged` logic. The
    // resize must fire only on the *transition*, so we track the previous visibility and
    // the pre-open width to restore on close.
    /// [`show_inspector`](AppState::show_inspector) as of the previous frame, used to
    /// detect the open/close transition that triggers the one-shot window resize.
    pub prev_show_inspector: bool,
    /// The window width captured just before the inspector opened, restored on close.
    /// `None` when the inspector is closed (nothing to restore).
    pub saved_width_before_inspector: Option<f32>,

    // --- Now-playing bar + fullscreen overlay (task 4.4) ---
    //
    // The floating now-playing bar is always present (requirement 1.9) and shows the
    // controls/layout bound to this MINIMAL placeholder. The real player service
    // (`player.rs`) and the player-bound `TrackPreviewBar` / `FullScreenNowPlayingView`
    // land in tasks 13 and 15; this task renders the bar + a fullscreen overlay host
    // against a stand-in track with no audio playback.
    /// The track the now-playing bar displays, or `None` for the idle bar. Populated by
    /// the player binding in task 13; a placeholder for task 4.4.
    pub now_playing: Option<NowPlayingPlaceholder>,
    /// Whether the fullscreen Now Playing overlay is presented (`design.md` app-state
    /// sketch). Toggled by the bar's open-fullscreen button; the real
    /// `FullScreenNowPlayingView` content lands in task 15.2.
    pub is_fullscreen_player: bool,
    /// One-shot transport/scrubber request flags raised by the now-playing bar and
    /// consumed by the player wiring (task 13). No playback happens here.
    pub player_requests: PlayerRequests,
    /// In-progress scrubber drag target (seconds). Set while dragging the scrubber; the
    /// player consumes it on release (task 13). `None` when not seeking.
    pub player_seek_target: Option<f64>,
    /// The real playback engine (task 13), once bound. `None` until task 18 constructs one;
    /// with no player, the bar/fullscreen view fall back to the flag-only placeholder
    /// behavior from task 4.4 so existing request-flag tests keep passing.
    pub player: Option<std::sync::Arc<walkman_services::player::Player>>,
    /// Parsed `.lrc` lines for the currently playing track (task 15.2's karaoke lyrics),
    /// sorted by timestamp. Empty until task 18 loads a real `.lrc` file for the active
    /// track; an empty list simply renders no lyrics.
    pub lyrics_lines: Vec<walkman_services::walkman_engine::LrcLine>,
    /// Lead time (seconds) applied when picking the active lyric line (requirement 7.3),
    /// matching the macOS karaoke view's ~0.3s anticipation.
    pub lyrics_lead_seconds: f64,

    // --- Local Library view data + state (task 14.1) ---
    //
    // The real `walkman_domain`/`walkman_services` types now exist (tasks 5/7), so this
    // view's data is plumbed as plain fields rather than scalar stand-ins — but *nothing
    // populates them yet*: real scanning/service wiring is task 18's job. `LocalLibraryView`
    // itself emits `LocalLibraryCommand`s into `local_library_view.commands` for task 18 to
    // drain and act on; nothing drains them here yet either.
    /// Every track across every registered local folder. Empty until task 18 wires
    /// `LocalLibrary::tracks()` in.
    pub local_tracks: Vec<walkman_domain::models::Track>,
    /// Every registered local folder. Empty until task 18 wires `LocalLibrary::list_folders()`
    /// in.
    pub local_folders: Vec<walkman_domain::models::LocalMusicFolder>,
    /// `LocalLibraryView`'s own UI state (display mode, sort, search, selection, open sheet,
    /// emitted commands).
    pub local_library_view: crate::views::local_library::LocalLibraryViewState,

    // --- Walkman "All Songs" view data + state (task 14.2) ---
    /// Every track on the connected Walkman's active drive. Empty until task 18 wires the real
    /// scan/cache load in.
    pub walkman_tracks: Vec<walkman_domain::models::Track>,
    /// `LibraryView`'s own UI state (sort column/direction, search, selection, emitted
    /// commands).
    pub library_view: crate::views::library::LibraryViewState,
    /// `RecentlyAddedView`'s own UI state (time scope, selection, emitted commands). Reads
    /// from the same `walkman_tracks` as [`AppState::library_view`].
    pub recently_added_view: crate::views::recently_added::RecentlyAddedViewState,
    /// `AlbumsView`'s own UI state (sort/format filter/Hi-Res toggle/search/drill-down/
    /// selection/emitted commands). Reads from the same `walkman_tracks`.
    pub albums_view: crate::views::albums::AlbumsViewState,
    /// `ArtistsView`'s own UI state (search/drill-down/emitted commands).
    pub artists_view: crate::views::artists::ArtistsViewState,
    /// `HiResAudioView`'s own UI state (view mode/sorts/filter/search/selection/emitted
    /// commands).
    pub hi_res_view: crate::views::hi_res::HiResViewState,
    /// `LanguageStudyView`'s own UI state (display mode/sort/selection/emitted commands).
    pub language_study_view: crate::views::language_study::LanguageStudyViewState,

    // --- Device Overview data + state (task 14.8) ---
    /// Every currently-connected Walkman device. Empty until task 18 wires real USB/volume
    /// detection in (task 9).
    pub devices: Vec<walkman_domain::models::WalkmanDevice>,
    /// Quick stats (albums/artists/hi-res) for the active device's library.
    pub device_quick_stats: crate::views::device_overview::DeviceQuickStats,
    /// `DeviceOverviewView`'s own UI state (selected device/MTP help sheet/emitted commands).
    pub device_overview_view: crate::views::device_overview::DeviceOverviewViewState,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            // Matches the macOS app's launch section (requirement 1.6).
            nav_tab: NavTab::LocalLibrary,
            // The inspector is hidden until the user opens it (requirement 1.1).
            show_inspector: false,
            selected_track_id: None,

            // No Walkman attached until a device event fires (task 9); the sidebar opens
            // in its disconnected state.
            walkman_connected: false,
            device_count: 0,
            drives: Vec::new(),

            local_track_count: 0,
            track_count: 0,
            recently_added_count: 0,
            album_count: 0,
            artist_count: 0,
            hi_res_count: 0,
            language_study_count: 0,
            duplicate_count: 0,
            queue_count: 0,

            about_requested: false,

            refresh_requested: false,
            eject_requested: false,
            is_ejecting: false,
            manage_folders_requested: false,

            // The inspector starts closed, so there is nothing to restore and no pending
            // transition on the first frame.
            prev_show_inspector: false,
            saved_width_before_inspector: None,

            // Nothing is playing on launch; the bar renders its idle state (the bar is
            // always present, requirement 1.9). The player binding (task 13) populates it.
            now_playing: None,
            is_fullscreen_player: false,
            player_requests: PlayerRequests::default(),
            player_seek_target: None,
            player: None,
            lyrics_lines: Vec::new(),
            lyrics_lead_seconds: 0.3,

            local_tracks: Vec::new(),
            local_folders: Vec::new(),
            local_library_view: crate::views::local_library::LocalLibraryViewState::default(),

            walkman_tracks: Vec::new(),
            library_view: crate::views::library::LibraryViewState::default(),
            recently_added_view: crate::views::recently_added::RecentlyAddedViewState::default(),
            albums_view: crate::views::albums::AlbumsViewState::default(),
            artists_view: crate::views::artists::ArtistsViewState::default(),
            hi_res_view: crate::views::hi_res::HiResViewState::default(),
            language_study_view: crate::views::language_study::LanguageStudyViewState::default(),

            devices: Vec::new(),
            device_quick_stats: crate::views::device_overview::DeviceQuickStats::default(),
            device_overview_view: crate::views::device_overview::DeviceOverviewViewState::default(),
        }
    }
}

impl AppState {
    /// Fresh launch state. Equivalent to [`AppState::default`]; named for intent at call
    /// sites (`AppState::new_on_launch()`).
    pub fn new_on_launch() -> Self {
        Self::default()
    }
}

/// Lays out the three-column shell into the current egui context.
///
/// Column order and sizing (requirements 1.1, 1.3; `design.md` "Layout"):
/// 1. a **fixed, non-resizable 260pt** left sidebar ([`sizing::SIDEBAR_WIDTH`]);
/// 2. an optional **300pt** trailing inspector ([`sizing::INSPECTOR_WIDTH`]) with a
///    [`DIVIDER_WIDTH`] (1px) separator on its leading edge, shown only when
///    [`AppState::show_inspector`] is set; and
/// 3. a **flexible** central detail pane that fills the remaining width.
///
/// The side panels are laid out first so the central pane receives exactly the leftover
/// space. Each region currently draws a placeholder; the real sidebar/detail/inspector
/// content is filled in by tasks 4.2–4.5.
pub fn layout_shell(ctx: &egui::Context, state: &mut AppState) {
    // Refresh `now_playing` from the real player (task 13), if one is bound, and let it
    // auto-advance (R7.5). A no-op while `state.player` is `None` (task 18 constructs one).
    now_playing::sync_now_playing_from_player(state);

    // --- Top: toolbar + menu bar (requirements 1.7, 1.8). ---
    //
    // Added first so the `TopBottomPanel::top` spans the full width above the three
    // columns, matching the macOS unified toolbar. It also handles the Ctrl+I /
    // Ctrl+Shift+L / Ctrl+Shift+O shortcuts and raises the command request flags (task 4.3).
    crate::toolbar::render(ctx, state);

    // The inspector widen-on-open window resize runs once per frame off the inspector
    // open/close transition (task 4.3). Driven here rather than inside the toolbar panel
    // so it fires exactly once regardless of panel ordering.
    crate::toolbar::apply_inspector_resize(ctx, state);

    // --- Left: fixed 260pt, non-collapsible sidebar (requirement 1.3). ---
    egui::SidePanel::left("walkman_sidebar")
        .exact_width(sizing::SIDEBAR_WIDTH)
        .resizable(false)
        .frame(egui::Frame::none().fill(color::BACKGROUND))
        .show(ctx, |ui| {
            // Real sidebar: NavTab section groups, visibility rules, count badges, and the
            // bottom About button (task 4.2).
            sidebar::render(ui, state);
        });

    // --- Right: optional 300pt inspector with a 1px leading divider (requirement 1.1). ---
    if state.show_inspector {
        egui::SidePanel::right("walkman_inspector")
            // The panel reserves the inspector width plus the hairline divider so the
            // central pane keeps its full flexible width regardless of the divider.
            .exact_width(sizing::INSPECTOR_WIDTH + DIVIDER_WIDTH)
            .resizable(false)
            .frame(egui::Frame::none().fill(color::BACKGROUND))
            .show(ctx, |ui| {
                // Crisp 1px border divider on the leading (left) edge of the inspector.
                let full = ui.max_rect();
                let divider = egui::Rect::from_min_max(
                    full.min,
                    egui::pos2(full.min.x + DIVIDER_WIDTH, full.max.y),
                );
                ui.painter().rect_filled(divider, 0.0, color::BORDER);

                // Inspector content sits to the right of the divider.
                let mut content = ui.new_child(
                    egui::UiBuilder::new().max_rect(egui::Rect::from_min_max(
                        egui::pos2(full.min.x + DIVIDER_WIDTH, full.min.y),
                        full.max,
                    )),
                );
                inspector_placeholder(&mut content, state);
            });
    }

    // --- Center: flexible detail pane filling the remaining width. ---
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(color::BACKGROUND))
        .show(ctx, |ui| {
            // Detail router (task 4.5): `match nav_tab` renders the selected section's
            // view. Each section currently draws a per-section placeholder; the real
            // views land in tasks 14–17.
            crate::detail::render(ui, state);
        });

    // --- Always-present floating now-playing bar (requirement 1.9, task 4.4). ---
    //
    // Rendered after the panels so it floats above the detail content (as a foreground
    // `Area`, it overlays rather than reserving layout space). The 88pt table bottom inset
    // (task 3.4) keeps table rows clear of the bar.
    now_playing::render_now_playing_bar(ctx, state);

    // --- Fullscreen Now Playing overlay host (task 4.4; content in task 15.2). ---
    // Drawn last so the overlay covers everything, including the now-playing bar, when the
    // user enlarges the player.
    now_playing::render_fullscreen_overlay(ctx, state);
}

/// Placeholder inspector pane. The real `MetadataStudioView` inspector lands in task 16.1.
fn inspector_placeholder(ui: &mut egui::Ui, _state: &AppState) {
    ui.add_space(12.0);
    ui.label("Inspector");
    ui.label("(MetadataStudioView: task 16.1)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_nav_tab_is_local_music_library() {
        // Requirement 1.6: the default selected section on launch is Local Music Library.
        let state = AppState::default();
        assert_eq!(state.nav_tab, NavTab::LocalLibrary);
        assert_eq!(state.nav_tab.title(), "Local Music Library");

        // `new_on_launch` is the intent-named alias of `default`.
        assert_eq!(AppState::new_on_launch().nav_tab, NavTab::LocalLibrary);
    }

    #[test]
    fn inspector_hidden_on_launch() {
        // Requirement 1.1: the trailing inspector is optional (hidden until opened).
        let state = AppState::default();
        assert!(!state.show_inspector);
        assert!(state.selected_track_id.is_none());
    }

    #[test]
    fn layout_sizing_constants_match_design() {
        // Requirement 1.3 / design.md "Sizing baselines": fixed 260pt sidebar, 300pt
        // inspector, and a 1px divider between the detail pane and the inspector.
        assert_eq!(sizing::SIDEBAR_WIDTH, 260.0);
        assert_eq!(sizing::INSPECTOR_WIDTH, 300.0);
        assert_eq!(DIVIDER_WIDTH, 1.0);
    }

    /// Compose the whole shell (toolbar + sidebar + detail router + inspector +
    /// now-playing bar + fullscreen overlay host) in a headless egui frame across a few
    /// representative states. egui lays out and paints during `Context::run`, so a panel
    /// overflow, an out-of-range rect, or a bad shape index would panic here. This is the
    /// end-of-task-4 sanity check that all the pieces compose together.
    #[test]
    fn shell_composes_in_headless_frame_for_representative_states() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);

        // (walkman_connected, device_count, show_inspector, is_fullscreen_player)
        let cases = [
            (false, 0usize, false, false), // disconnected, inspector closed
            (true, 1, true, false),        // connected, inspector open
            (true, 2, false, true),        // multi-device, fullscreen overlay up
        ];

        for (connected, device_count, show_inspector, fullscreen) in cases {
            let mut state = AppState::default();
            state.walkman_connected = connected;
            state.device_count = device_count;
            state.show_inspector = show_inspector;
            state.is_fullscreen_player = fullscreen;
            if connected {
                state.drives = vec![DriveRow {
                    name: "Internal Storage".into(),
                    is_current: true,
                    is_sd: false,
                }];
                state.now_playing = Some(NowPlayingPlaceholder::sample());
            }

            // Render every section so the detail router is exercised in a fully composed
            // shell, not just in isolation.
            for tab in NavTab::ALL {
                state.nav_tab = tab;
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    layout_shell(ctx, &mut state);
                });
            }
        }
    }
}
