//! Walkman Studio for Windows — iced entry point (migration target for `main.rs`).
//!
//! Milestone 1 (scaffolding) of the egui-to-iced migration plan: an empty obsidian
//! window at the enforced size, nothing else wired up yet. `main.rs`'s egui app is the
//! shipping binary (`walkman-studio`) until this one reaches parity; this binary
//! (`walkman-studio-iced`) is a development-only target until then.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use iced::{Element, Size, Task, Theme};
use walkman_ui::iced_ui::shell;
use walkman_ui::iced_ui::theme::sizing;

fn main() -> iced::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    iced::application(App::title, App::update, App::view)
        .theme(App::theme)
        .window(iced::window::Settings {
            size: Size::new(sizing::WINDOW_DEFAULT.0, sizing::WINDOW_DEFAULT.1),
            min_size: Some(Size::new(sizing::WINDOW_MIN.0, sizing::WINDOW_MIN.1)),
            ..Default::default()
        })
        .run_with(App::new)
}

struct App {
    state: shell::AppState,
}

impl App {
    fn new() -> (Self, Task<shell::Message>) {
        (
            App {
                state: shell::AppState::default(),
            },
            Task::none(),
        )
    }

    fn title(&self) -> String {
        "Walkman Studio".to_string()
    }

    fn update(&mut self, message: shell::Message) -> Task<shell::Message> {
        shell::update(&mut self.state, message);
        Task::none()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn view(&self) -> Element<'_, shell::Message> {
        shell::view(&self.state)
    }
}
