//! Trim & Cut sheet for the iced shell, ported from `crate::sheets::trim_cut`
//! (`TrimAndCutSheet.swift`). Reuses that module's pure types/logic unchanged —
//! `STEP_MS_*`/`GAIN_MIN_DB`/`GAIN_MAX_DB`, `TrimCutCommand`, [`TrimCutSheetState`],
//! `nudge_start`/`nudge_end`, `lock_start_to_playhead`/`lock_end_to_playhead`,
//! `ms_to_fraction`/`fraction_to_ms` — and only rewrites `render` as this module's
//! `view`/`update`. The waveform is the first real `iced::widget::canvas::Program` use
//! in this migration (everything else so far used declarative `Style`s).
//!
//! Documented simplifications: egui's version tracks continuous drag (every
//! `response.dragged()` frame moves the nearer handle smoothly); this canvas only
//! handles discrete click events (press anywhere on the waveform moves the nearer
//! handle to that point) — functionally equivalent for placing a handle, just not a
//! smooth drag. Fade in/out (`u64` milliseconds) use text inputs instead of egui's
//! `DragValue` spinner; gain (`f32` dB) uses iced's `slider`, matching egui's `Slider`.

use iced::mouse;
use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke};
use iced::widget::{button, checkbox, column, row, scrollable, slider, text, text_input};
use iced::{Element, Length, Point, Rectangle, Renderer, Theme};

use walkman_services::trim_cut::{ChannelMode, TrimCutMode};

use crate::iced_ui::theme::{color, font};
use crate::sheets::trim_cut::{
    fraction_to_ms, lock_end_to_playhead, lock_start_to_playhead, nudge_end, nudge_start, TrimCutCommand, TrimCutSheetState, GAIN_MAX_DB,
    GAIN_MIN_DB, STEP_MS_LARGE, STEP_MS_MEDIUM, STEP_MS_SMALL,
};

const WAVEFORM_HEIGHT: f32 = 100.0;

#[derive(Debug, Clone)]
pub enum Message {
    ModeSelected(TrimCutMode),
    PreviewClicked,
    StopClicked,
    WaveformClicked(f32),
    NudgeStart(i64),
    NudgeEnd(i64),
    LockInClicked,
    LockOutClicked,
    LockToPlayheadToggled(bool),
    ChannelModeSelected(ChannelMode),
    FadeInChanged(String),
    FadeOutChanged(String),
    GainChanged(f32),
    ApplyClicked,
    CancelClicked,
}

pub fn update(state: &mut TrimCutSheetState, message: Message) {
    match message {
        Message::ModeSelected(mode) => state.mode = mode,
        Message::PreviewClicked => {
            state.is_previewing = true;
            state.commands.push(TrimCutCommand::Preview);
        }
        Message::StopClicked => {
            state.is_previewing = false;
            state.commands.push(TrimCutCommand::StopPreview);
        }
        Message::WaveformClicked(fraction) => {
            let ms = fraction_to_ms(fraction, state.duration_ms);
            if ms.abs_diff(state.start_ms) <= ms.abs_diff(state.end_ms) {
                state.start_ms = ms.min(state.end_ms);
            } else {
                state.end_ms = ms.max(state.start_ms);
            }
        }
        Message::NudgeStart(delta) => nudge_start(state, delta),
        Message::NudgeEnd(delta) => nudge_end(state, delta),
        Message::LockInClicked => lock_start_to_playhead(state),
        Message::LockOutClicked => lock_end_to_playhead(state),
        Message::LockToPlayheadToggled(value) => state.lock_to_playhead = value,
        Message::ChannelModeSelected(mode) => state.channel_mode = mode,
        Message::FadeInChanged(v) => {
            if let Ok(ms) = v.parse() {
                state.fade_in_ms = ms;
            }
        }
        Message::FadeOutChanged(v) => {
            if let Ok(ms) = v.parse() {
                state.fade_out_ms = ms;
            }
        }
        Message::GainChanged(v) => state.gain_db = v,
        Message::ApplyClicked => {
            state.commands.push(TrimCutCommand::Apply { track_id: state.track_id.clone(), options: state.options() });
            state.close();
        }
        Message::CancelClicked => {
            state.commands.push(TrimCutCommand::Cancel);
            state.close();
        }
    }
}

pub fn view(state: &TrimCutSheetState) -> Option<Element<'_, Message>> {
    if !state.open {
        return None;
    }

    let content = column![
        text("Trim & Cut").size(font::TITLE2.size).color(color::SILVER_LIGHT),
        mode_toggle(state),
        waveform(state),
        scrollable(
            column![steppers(state), channel_routing(state), fades_and_gain(state)].spacing(10)
        )
        .height(Length::Fixed(260.0)),
        footer(),
    ]
    .spacing(10);

    Some(super::modal(content, 760.0, 600.0))
}

fn mode_toggle(state: &TrimCutSheetState) -> Element<'_, Message> {
    let modes = row![
        mode_chip(TrimCutMode::Trim, "Trim (Keep Selection)", state.mode),
        mode_chip(TrimCutMode::Cut, "Cut (Remove Selection)", state.mode),
    ]
    .spacing(6);

    let preview = if state.is_previewing {
        button(text("Stop").size(font::CALLOUT.size)).padding([6, 12]).on_press(Message::StopClicked).style(|_theme, s| destructive_style(s))
    } else {
        button(text("Preview").size(font::CALLOUT.size)).padding([6, 12]).on_press(Message::PreviewClicked).style(|_theme, s| prominent_style(s))
    };

    row![modes, iced::widget::Space::new(Length::Fill, Length::Shrink), preview].align_y(iced::alignment::Vertical::Center).into()
}

fn mode_chip<'a>(mode: TrimCutMode, label: &'static str, selected: TrimCutMode) -> Element<'a, Message> {
    let active = mode == selected;
    button(text(label).size(font::CALLOUT.size))
        .padding([6, 12])
        .on_press(Message::ModeSelected(mode))
        .style(move |_theme, status| chip_style(active, status))
        .into()
}

fn waveform(state: &TrimCutSheetState) -> Element<'_, Message> {
    Canvas::new(WaveformCanvas {
        waveform: &state.waveform,
        start_ms: state.start_ms,
        end_ms: state.end_ms,
        playhead_ms: state.playhead_ms,
        duration_ms: state.duration_ms,
    })
    .width(Length::Fill)
    .height(Length::Fixed(WAVEFORM_HEIGHT))
    .into()
}

struct WaveformCanvas<'a> {
    waveform: &'a [f32],
    start_ms: u64,
    end_ms: u64,
    playhead_ms: u64,
    duration_ms: u64,
}

impl<'a> canvas::Program<Message> for WaveformCanvas<'a> {
    type State = ();

    fn update(
        &self,
        _state: &mut (),
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (canvas::event::Status, Option<Message>) {
        if let canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
            if let Some(pos) = cursor.position_in(bounds) {
                let fraction = (pos.x / bounds.width).clamp(0.0, 1.0);
                return (canvas::event::Status::Captured, Some(Message::WaveformClicked(fraction)));
            }
        }
        (canvas::event::Status::Ignored, None)
    }

    fn draw(&self, _state: &(), renderer: &Renderer, _theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), color::INPUT);

        if !self.waveform.is_empty() {
            let n = self.waveform.len();
            let bar_width = (bounds.width / n as f32).max(1.0);
            for (i, peak) in self.waveform.iter().enumerate() {
                let x = i as f32 * bar_width;
                let half_height = (peak.clamp(0.0, 1.0) * bounds.height * 0.5).max(1.0);
                let y = bounds.height * 0.5 - half_height;
                frame.fill_rectangle(Point::new(x, y), iced::Size::new(bar_width.max(1.0), half_height * 2.0), color::with_alpha(iced::Color::WHITE, 0.5));
            }
        }

        let start_x = ms_to_fraction(self.start_ms, self.duration_ms) * bounds.width;
        let end_x = ms_to_fraction(self.end_ms, self.duration_ms) * bounds.width;
        frame.fill_rectangle(
            Point::new(start_x, 0.0),
            iced::Size::new((end_x - start_x).max(0.0), bounds.height),
            color::with_alpha(iced::Color::WHITE, 0.12),
        );
        frame.stroke(&Path::line(Point::new(start_x, 0.0), Point::new(start_x, bounds.height)), Stroke::default().with_color(color::HIRES_GOLD).with_width(2.0));
        frame.stroke(&Path::line(Point::new(end_x, 0.0), Point::new(end_x, bounds.height)), Stroke::default().with_color(color::HIRES_GOLD).with_width(2.0));

        let playhead_x = ms_to_fraction(self.playhead_ms, self.duration_ms) * bounds.width;
        frame.stroke(&Path::line(Point::new(playhead_x, 0.0), Point::new(playhead_x, bounds.height)), Stroke::default().with_color(color::SILVER_LIGHT).with_width(1.5));

        vec![frame.into_geometry()]
    }
}

fn ms_to_fraction(ms: u64, duration_ms: u64) -> f32 {
    crate::sheets::trim_cut::ms_to_fraction(ms, duration_ms)
}

fn steppers(state: &TrimCutSheetState) -> Element<'_, Message> {
    let step_row = |label_prefix: &'static str, current_ms: u64, on_nudge: fn(i64) -> Message, lock_message: Message| {
        let steps = [("-5s", STEP_MS_LARGE), ("-1s", STEP_MS_MEDIUM), ("-0.1s", STEP_MS_SMALL), ("+0.1s", STEP_MS_SMALL), ("+1s", STEP_MS_MEDIUM), ("+5s", STEP_MS_LARGE)]
            .into_iter()
            .fold(row![].spacing(4), |r, (label, step)| {
                let delta = if label.starts_with('-') { -(step as i64) } else { step as i64 };
                r.push(button(text(label).size(font::CAPTION.size)).padding([4, 8]).on_press(on_nudge(delta)).style(|_theme, s| chip_style(false, s)))
            });

        let mut lock_button = button(text(format!("Lock {label_prefix}\u{2026}")).size(font::CAPTION.size)).padding([4, 8]);
        if state.lock_to_playhead {
            lock_button = lock_button.on_press(lock_message);
        }

        row![
            text(label_prefix).size(font::CAPTION.size).color(color::SILVER_MUTED),
            text(format_ms(current_ms)).size(font::MONO_SMALL.size).color(color::SILVER_LIGHT),
            steps,
            lock_button.style(|_theme, s| chip_style(false, s)),
        ]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center)
    };

    column![
        step_row("In", state.start_ms, Message::NudgeStart, Message::LockInClicked),
        step_row("Out", state.end_ms, Message::NudgeEnd, Message::LockOutClicked),
        checkbox("Lock to Playhead", state.lock_to_playhead).on_toggle(Message::LockToPlayheadToggled),
    ]
    .spacing(6)
    .into()
}

fn format_ms(ms: u64) -> String {
    format!("{:.1}s", ms as f64 / 1000.0)
}

fn channel_routing(state: &TrimCutSheetState) -> Element<'_, Message> {
    let modes = [
        (ChannelMode::None, "None"),
        (ChannelMode::SwapLr, "Swap L/R"),
        (ChannelMode::Mono, "Mono"),
        (ChannelMode::Stereo, "Stereo"),
        (ChannelMode::IsolateLeft, "Isolate L"),
        (ChannelMode::IsolateRight, "Isolate R"),
        (ChannelMode::MuteLeft, "Mute L"),
        (ChannelMode::MuteRight, "Mute R"),
        (ChannelMode::PhaseFlip, "Phase Flip"),
    ]
    .into_iter()
    .fold(row![].spacing(4), |r, (mode, label)| {
        let active = state.channel_mode == mode;
        r.push(button(text(label).size(font::CAPTION.size)).padding([4, 8]).on_press(Message::ChannelModeSelected(mode)).style(move |_theme, s| chip_style(active, s)))
    });

    column![text("Channel Routing").size(font::CAPTION_BOLD.size).color(color::SILVER_MUTED), modes].spacing(6).into()
}

fn fades_and_gain(state: &TrimCutSheetState) -> Element<'_, Message> {
    column![
        row![
            text("Fade In (ms)").size(font::CAPTION.size).color(color::SILVER_MUTED),
            text_input("0", &state.fade_in_ms.to_string()).on_input(Message::FadeInChanged).padding(6).size(font::CAPTION.size).width(Length::Fixed(80.0)),
            text("Fade Out (ms)").size(font::CAPTION.size).color(color::SILVER_MUTED),
            text_input("0", &state.fade_out_ms.to_string()).on_input(Message::FadeOutChanged).padding(6).size(font::CAPTION.size).width(Length::Fixed(80.0)),
        ]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center),
        row![
            text("Gain (dB)").size(font::CAPTION.size).color(color::SILVER_MUTED),
            slider(GAIN_MIN_DB..=GAIN_MAX_DB, state.gain_db, Message::GainChanged).width(Length::Fixed(200.0)),
            text(format!("{:.1} dB", state.gain_db)).size(font::CAPTION.size).color(color::SILVER_MUTED),
        ]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center),
    ]
    .spacing(6)
    .into()
}

fn footer<'a>() -> Element<'a, Message> {
    row![
        button(text("Apply").size(font::CALLOUT.size)).padding([8, 16]).on_press(Message::ApplyClicked).style(|_theme, s| prominent_style(s)),
        button(text("Cancel").size(font::CALLOUT.size)).padding([8, 16]).on_press(Message::CancelClicked).style(|_theme, s| chip_style(false, s)),
    ]
    .spacing(8)
    .into()
}

fn chip_style(active: bool, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let fill = if active {
        color::with_alpha(iced::Color::WHITE, 0.14)
    } else if hovered {
        color::with_alpha(iced::Color::WHITE, 0.06)
    } else {
        color::with_alpha(color::CARD, 0.55)
    };
    button::Style {
        background: Some(iced::Background::Color(fill)),
        text_color: if active { color::SILVER_LIGHT } else { color::SILVER_MUTED },
        border: iced::Border {
            color: color::BORDER,
            width: 1.0,
            radius: crate::iced_ui::theme::sizing::radius::CAPSULE.into(),
        },
        ..button::Style::default()
    }
}

fn prominent_style(status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(iced::Background::Color(if hovered { color::lighten(color::SILVER, 0.08) } else { color::SILVER })),
        text_color: color::OBSIDIAN,
        border: iced::Border {
            color: color::with_alpha(iced::Color::WHITE, 0.25),
            width: 1.0,
            radius: crate::iced_ui::theme::sizing::radius::BUTTON.into(),
        },
        ..button::Style::default()
    }
}

fn destructive_style(status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(iced::Background::Color(color::with_alpha(color::DESTRUCTIVE, if hovered { 0.95 } else { 0.85 }))),
        text_color: color::SILVER_LIGHT,
        border: iced::Border {
            color: color::with_alpha(color::DESTRUCTIVE_LIGHT, 0.35),
            width: 1.0,
            radius: crate::iced_ui::theme::sizing::radius::BUTTON.into(),
        },
        ..button::Style::default()
    }
}
