//! Application shell, ported from `crate::shell` (egui) / `ContentView.swift`.
//!
//! Milestone 3 of the egui-to-iced migration plan: the three-column layout — a fixed
//! 260pt sidebar, a flexible central detail pane, and an optional 300pt trailing
//! inspector — plus the floating now-playing bar overlaid at the bottom center. Detail
//! panes are placeholders (just the section title) until the view-by-view milestone;
//! the sidebar uses the real, framework-agnostic [`crate::NavTab`] (no egui dependency)
//! so this already matches the real 13-section nav order/titles.

use iced::widget::{button, column, container, row, text, Row, Space};
use iced::{Border, Element, Length, Padding};

use walkman_domain::models::{LocalMusicFolder, Playlist, Track, TransferItem, WalkmanDevice};

use crate::iced_ui::theme::{color, font, sizing};
use crate::iced_ui::views::albums as albums_view;
use crate::iced_ui::views::artists as artists_view;
use crate::iced_ui::views::backup_restore as backup_restore_view;
use crate::iced_ui::views::device_overview as device_overview_view;
use crate::iced_ui::views::duplicates as duplicates_view;
use crate::iced_ui::views::hi_res as hi_res_view;
use crate::iced_ui::views::language_study as language_study_view;
use crate::iced_ui::views::library as library_view;
use crate::iced_ui::views::local_library as local_library_view;
use crate::iced_ui::views::playlists as playlists_view;
use crate::iced_ui::views::recently_added as recently_added_view;
use crate::iced_ui::views::senseme as senseme_view;
use crate::iced_ui::views::transfer_queue as transfer_queue_view;
use crate::iced_ui::widgets;
use crate::views::albums::AlbumsViewState;
use crate::views::artists::ArtistsViewState;
use crate::views::backup_restore::BackupRestoreViewState;
use crate::views::device_overview::{DeviceOverviewViewState, DeviceQuickStats};
use crate::views::duplicates::DuplicatesViewState;
use crate::views::hi_res::HiResViewState;
use crate::views::language_study::LanguageStudyViewState;
use crate::views::library::LibraryViewState;
use crate::views::local_library::LocalLibraryViewState;
use crate::views::playlists::PlaylistsViewState;
use crate::views::recently_added::RecentlyAddedViewState;
use crate::views::senseme::SenseMeViewState;
use crate::views::transfer_queue::TransferQueueViewState;
use crate::NavTab;

/// Shell-level state: which section is selected, whether the inspector is open, plus the
/// handful of real counts Milestone 4's lifecycle wiring now feeds in (sidebar badges
/// pending the view-by-view milestone). Grows as later milestones port more of the egui
/// `AppState` (device/track data, per-view/per-sheet state, player state, …) in.
#[derive(Debug)]
pub struct AppState {
    pub nav_tab: NavTab,
    pub show_inspector: bool,
    pub walkman_connected: bool,
    pub devices: Vec<WalkmanDevice>,
    pub local_tracks: Vec<Track>,
    pub local_folders: Vec<LocalMusicFolder>,
    pub walkman_tracks: Vec<Track>,
    pub local_library_view: LocalLibraryViewState,
    pub recently_added_view: RecentlyAddedViewState,
    pub library_view: LibraryViewState,
    pub artists_view: ArtistsViewState,
    pub albums_view: AlbumsViewState,
    pub device_overview_view: DeviceOverviewViewState,
    pub hi_res_view: HiResViewState,
    pub language_study_view: LanguageStudyViewState,
    pub senseme_view: SenseMeViewState,
    pub duplicates_view: DuplicatesViewState,
    pub transfer_queue_items: Vec<TransferItem>,
    pub transfer_queue_view: TransferQueueViewState,
    pub backup_restore_view: BackupRestoreViewState,
    pub playlists: Vec<Playlist>,
    pub playlists_view: PlaylistsViewState,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            // Defaults to Local Music Library on launch, matching the egui shell.
            nav_tab: NavTab::LocalLibrary,
            show_inspector: false,
            walkman_connected: false,
            devices: Vec::new(),
            local_tracks: Vec::new(),
            local_folders: Vec::new(),
            walkman_tracks: Vec::new(),
            local_library_view: LocalLibraryViewState::default(),
            recently_added_view: RecentlyAddedViewState::default(),
            library_view: LibraryViewState::default(),
            artists_view: ArtistsViewState::default(),
            albums_view: AlbumsViewState::default(),
            device_overview_view: DeviceOverviewViewState::default(),
            hi_res_view: HiResViewState::default(),
            language_study_view: LanguageStudyViewState::default(),
            senseme_view: SenseMeViewState::default(),
            duplicates_view: DuplicatesViewState::default(),
            transfer_queue_items: Vec::new(),
            transfer_queue_view: TransferQueueViewState::default(),
            backup_restore_view: BackupRestoreViewState::default(),
            playlists: Vec::new(),
            playlists_view: PlaylistsViewState::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    NavSelected(NavTab),
    ToggleInspector,
    LocalLibrary(local_library_view::Message),
    RecentlyAdded(recently_added_view::Message),
    Library(library_view::Message),
    Artists(artists_view::Message),
    Albums(albums_view::Message),
    DeviceOverview(device_overview_view::Message),
    HiRes(hi_res_view::Message),
    LanguageStudy(language_study_view::Message),
    SenseMe(senseme_view::Message),
    Duplicates(duplicates_view::Message),
    TransferQueue(transfer_queue_view::Message),
    BackupRestore(backup_restore_view::Message),
    Playlists(playlists_view::Message),
}

pub fn update(state: &mut AppState, message: Message) {
    match message {
        Message::NavSelected(tab) => state.nav_tab = tab,
        Message::ToggleInspector => state.show_inspector = !state.show_inspector,
        Message::LocalLibrary(m) => local_library_view::update(&mut state.local_library_view, m),
        Message::RecentlyAdded(m) => recently_added_view::update(&mut state.recently_added_view, m),
        Message::Library(m) => library_view::update(&mut state.library_view, m),
        Message::Artists(m) => artists_view::update(&mut state.artists_view, m),
        Message::Albums(m) => albums_view::update(&mut state.albums_view, m),
        Message::DeviceOverview(m) => device_overview_view::update(&mut state.device_overview_view, m),
        Message::HiRes(m) => hi_res_view::update(&mut state.hi_res_view, m),
        Message::LanguageStudy(m) => language_study_view::update(&mut state.language_study_view, m),
        Message::SenseMe(m) => senseme_view::update(&state.walkman_tracks, &mut state.senseme_view, m),
        Message::Duplicates(m) => duplicates_view::update(&mut state.duplicates_view, m),
        Message::TransferQueue(m) => transfer_queue_view::update(&state.transfer_queue_items, &mut state.transfer_queue_view, m),
        Message::BackupRestore(m) => backup_restore_view::update(&state.walkman_tracks, &mut state.backup_restore_view, m),
        Message::Playlists(m) => playlists_view::update(&state.playlists, &mut state.playlists_view, m),
    }
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let mut body: Row<'_, Message> = row![sidebar(state), detail_pane(state)];
    if state.show_inspector {
        body = body.push(inspector_pane());
    }
    let body = body.width(Length::Fill).height(Length::Fill);

    iced::widget::stack![
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(iced::Background::Color(color::BACKGROUND)),
                ..container::Style::default()
            }),
        container(now_playing_bar())
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Bottom)
            .padding(Padding {
                top: 0.0,
                right: 0.0,
                bottom: 16.0,
                left: 0.0,
            }),
    ]
    .into()
}

fn sidebar(state: &AppState) -> Element<'_, Message> {
    let items = NavTab::ALL.iter().fold(column![].spacing(2), |col, &tab| {
        let selected = tab == state.nav_tab;
        col.push(
            button(text(tab.title()).size(font::BODY.size))
                .width(Length::Fill)
                .padding([8, 12])
                .on_press(Message::NavSelected(tab))
                .style(move |_theme, status| sidebar_item_style(selected, status)),
        )
    });

    let status_line = if state.walkman_connected {
        format!(
            "{} device(s) · {} local · {} on device",
            state.devices.len(),
            state.local_tracks.len(),
            state.walkman_tracks.len()
        )
    } else {
        format!("No device · {} local tracks", state.local_tracks.len())
    };

    container(column![
        container(column![
            text("Walkman Studio").size(font::TITLE3.size),
            text(status_line).size(font::CAPTION.size).color(color::SILVER_MUTED),
        ].spacing(2))
        .padding(Padding {
            top: 16.0,
            right: 16.0,
            bottom: 12.0,
            left: 16.0,
        }),
        iced::widget::scrollable(items.padding([0, 8])).height(Length::Fill),
    ])
    .width(Length::Fixed(sizing::SIDEBAR_WIDTH))
    .height(Length::Fill)
    .style(|_theme| container::Style {
        background: Some(iced::Background::Color(color::OBSIDIAN)),
        border: Border {
            color: color::BORDER,
            width: 1.0,
            ..Border::default()
        },
        ..container::Style::default()
    })
    .into()
}

fn sidebar_item_style(selected: bool, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let fill = if selected {
        color::with_alpha(iced::Color::WHITE, 0.14)
    } else if hovered {
        color::with_alpha(iced::Color::WHITE, 0.06)
    } else {
        iced::Color::TRANSPARENT
    };
    button::Style {
        background: Some(iced::Background::Color(fill)),
        text_color: if selected {
            color::SILVER_LIGHT
        } else {
            color::SILVER_MUTED
        },
        border: Border {
            radius: 8.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn detail_pane(state: &AppState) -> Element<'_, Message> {
    if state.nav_tab == NavTab::Overview {
        let stats = walkman_services::local_library::precompute(state.walkman_tracks.clone()).stats;
        let quick_stats = DeviceQuickStats {
            album_count: stats.album_count,
            artist_count: stats.artist_count,
            hi_res_count: stats.hi_res_count,
        };
        return device_overview_view::view(&state.devices, quick_stats, &state.device_overview_view)
            .map(Message::DeviceOverview);
    }
    if state.nav_tab == NavTab::LocalLibrary {
        return local_library_view::view(&state.local_tracks, &state.local_folders, &state.local_library_view)
            .map(Message::LocalLibrary);
    }
    if state.nav_tab == NavTab::RecentlyAdded {
        return recently_added_view::view(&state.walkman_tracks, &state.recently_added_view)
            .map(Message::RecentlyAdded);
    }
    if state.nav_tab == NavTab::Library {
        return library_view::view(&state.walkman_tracks, &state.library_view).map(Message::Library);
    }
    if state.nav_tab == NavTab::Artists {
        return artists_view::view(&state.walkman_tracks, &state.artists_view).map(Message::Artists);
    }
    if state.nav_tab == NavTab::Albums {
        return albums_view::view(&state.walkman_tracks, &state.albums_view).map(Message::Albums);
    }
    if state.nav_tab == NavTab::HiRes {
        return hi_res_view::view(&state.walkman_tracks, &state.hi_res_view).map(Message::HiRes);
    }
    if state.nav_tab == NavTab::LanguageStudy {
        return language_study_view::view(&state.walkman_tracks, &state.language_study_view).map(Message::LanguageStudy);
    }
    if state.nav_tab == NavTab::SenseMe {
        return senseme_view::view(&state.walkman_tracks, &state.senseme_view).map(Message::SenseMe);
    }
    if state.nav_tab == NavTab::Duplicates {
        return duplicates_view::view(&state.walkman_tracks, &state.duplicates_view).map(Message::Duplicates);
    }
    if state.nav_tab == NavTab::Transfer {
        return transfer_queue_view::view(&state.transfer_queue_items, &state.transfer_queue_view).map(Message::TransferQueue);
    }
    if state.nav_tab == NavTab::BackupRestore {
        return backup_restore_view::view(&state.walkman_tracks, &state.backup_restore_view).map(Message::BackupRestore);
    }
    if state.nav_tab == NavTab::Playlists {
        return playlists_view::view(&state.playlists, &state.playlists_view).map(Message::Playlists);
    }

    container(column![
        text(state.nav_tab.title()).size(font::TITLE1.size),
        Space::new(Length::Fill, Length::Fixed(8.0)),
        text("View not ported yet.")
            .size(font::BODY.size)
            .color(color::SILVER_MUTED),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding {
        top: 24.0,
        right: 24.0,
        bottom: sizing::TABLE_BOTTOM_INSET,
        left: 24.0,
    })
    .into()
}

fn inspector_pane<'a, Message: 'a>() -> Element<'a, Message> {
    container(column![
        text("Inspector").size(font::HEADLINE.size),
        Space::new(Length::Fill, Length::Fixed(8.0)),
        text("Select a track to inspect.")
            .size(font::BODY.size)
            .color(color::SILVER_MUTED),
    ])
    .padding(16)
    .width(Length::Fixed(sizing::INSPECTOR_WIDTH))
    .height(Length::Fill)
    .style(|_theme| container::Style {
        background: Some(iced::Background::Color(color::CARD)),
        border: Border {
            color: color::BORDER,
            width: 1.0,
            ..Border::default()
        },
        ..container::Style::default()
    })
    .into()
}

fn now_playing_bar<'a, Message: 'a>() -> Element<'a, Message> {
    container(
        row![text("Not Playing")
            .size(font::BODY.size)
            .color(color::SILVER_MUTED)]
        .padding([10, 16]),
    )
    .width(Length::Fixed(sizing::PLAYER_MAX_WIDTH))
    .style(|_theme| widgets::glass::studio_glass_card())
    .into()
}
