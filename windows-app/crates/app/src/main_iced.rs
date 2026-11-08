//! Walkman Studio for Windows — iced entry point (migration target for `main.rs`).
//!
//! Milestone 1 (scaffolding) of the egui-to-iced migration plan: an empty obsidian
//! window at the enforced size, nothing else wired up yet. `main.rs`'s egui app is the
//! shipping binary (`walkman-studio`) until this one reaches parity; this binary
//! (`walkman-studio-iced`) is a development-only target until then.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use iced::widget::container;
use iced::{Background, Element, Length, Size, Task, Theme};
use walkman_ui::iced_ui::theme::{color, sizing};

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

#[derive(Debug, Clone)]
enum Message {}

struct App;

impl App {
    fn new() -> (Self, Task<Message>) {
        (App, Task::none())
    }

    fn title(&self) -> String {
        "Walkman Studio".to_string()
    }

    fn update(&mut self, _message: Message) -> Task<Message> {
        Task::none()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn view(&self) -> Element<'_, Message> {
        container(iced::widget::text("Walkman Studio").size(22))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(Background::Color(color::BACKGROUND)),
                text_color: Some(color::SILVER_LIGHT),
                ..container::Style::default()
            })
            .into()
    }
}
