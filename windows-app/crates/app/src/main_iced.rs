//! Walkman Studio for Windows — iced entry point (migration target for `main.rs`).
//!
//! Milestone 4 of the egui-to-iced migration plan wires real `AppServices` in and
//! replaces egui's "spawn a thread, stash its `Receiver`, poll every frame" lifecycle
//! pattern (`lifecycle.rs`) with iced's `Task`/`Subscription` model (`lifecycle_iced.rs`).
//! No per-view command enums exist yet (views are placeholders until Milestone 5), so the
//! top-level [`Message`] only wraps the shell's own messages and the lifecycle ones for
//! now; it grows a variant per view/sheet as they're ported.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod lifecycle_iced;
mod services;

use iced::{Element, Size, Task, Theme};
use walkman_ui::iced_ui::shell;
use walkman_ui::iced_ui::theme::sizing;

use services::AppServices;

fn main() -> iced::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Fallible service construction (the shared cache database) happens up front, exactly
    // like `main.rs`'s `eframe::run_native` closure propagating it via `?` — a corrupt
    // `library.sqlite3` is a startup-fatal condition either way.
    let services = AppServices::new().expect("failed to start Walkman Studio services");

    iced::application(App::title, App::update, App::view)
        .subscription(App::subscription)
        .theme(App::theme)
        .window(iced::window::Settings {
            size: Size::new(sizing::WINDOW_DEFAULT.0, sizing::WINDOW_DEFAULT.1),
            min_size: Some(Size::new(sizing::WINDOW_MIN.0, sizing::WINDOW_MIN.1)),
            ..Default::default()
        })
        .run_with(move || App::new(services))
}

#[derive(Debug, Clone)]
enum Message {
    Shell(shell::Message),
    Lifecycle(lifecycle_iced::Message),
}

struct App {
    state: shell::AppState,
    services: AppServices,
    /// Lifecycle bookkeeping that isn't UI-presentable state — ported from
    /// `lifecycle.rs::Lifecycle::scanned_mounts`'s same role (don't rescan a mount that
    /// hasn't changed).
    scanned_mounts: Vec<String>,
}

impl App {
    fn new(services: AppServices) -> (Self, Task<Message>) {
        let startup = lifecycle_iced::startup_task(&services).map(Message::Lifecycle);
        (
            App {
                state: shell::AppState::default(),
                services,
                scanned_mounts: Vec::new(),
            },
            startup,
        )
    }

    fn title(&self) -> String {
        "Walkman Studio".to_string()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Shell(m) => {
                shell::update(&mut self.state, m);
                Task::none()
            }
            Message::Lifecycle(lifecycle_iced::Message::LocalLibraryLoaded { folders, tracks }) => {
                self.state.local_folders = folders;
                self.state.local_tracks = tracks;
                Task::none()
            }
            Message::Lifecycle(lifecycle_iced::Message::DevicePollTick) => {
                let (devices, task) =
                    lifecycle_iced::refresh_devices(&self.services, &mut self.scanned_mounts);
                self.state.walkman_connected = !devices.is_empty();
                self.state.device_count = devices.len();
                if devices.is_empty() {
                    self.state.walkman_track_count = 0;
                }
                task.map_or(Task::none(), |t| t.map(Message::Lifecycle))
            }
            Message::Lifecycle(lifecycle_iced::Message::WalkmanMountScanned(tracks)) => {
                self.state.walkman_track_count = tracks.len();
                Task::none()
            }
        }
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        lifecycle_iced::subscription().map(Message::Lifecycle)
    }

    fn view(&self) -> Element<'_, Message> {
        shell::view(&self.state).map(Message::Shell)
    }
}
