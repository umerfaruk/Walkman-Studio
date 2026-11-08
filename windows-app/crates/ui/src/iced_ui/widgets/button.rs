//! Button variants, ported from `crate::theme::button` (itself ported from
//! `LiquidGlassButtonStyle.Variant`). The egui version implements `egui::Widget` and
//! hand-paints hover/press scale + gradient fill + gloss band; iced's button widget
//! already handles press/hover state transitions declaratively via `Status`, so this
//! is a plain style resolver passed to `iced::widget::button(...).style(...)`.
//!
//! Scale-on-press/hover (macOS spring animation) isn't reproduced here — iced has no
//! built-in animation primitive for it, and it's cosmetic rather than structural; flag
//! for a later milestone if wanted; the color/border/shadow states already give clear
//! hover/press feedback.

use crate::iced_ui::theme::color::{self, lighten, with_alpha};
use crate::iced_ui::theme::sizing::radius;
use iced::widget::button::{Status, Style};
use iced::{Background, Border, Color, Shadow, Vector};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// Silver/platinum — the default prominent action.
    Prominent,
    /// Hi-Res gold — used for Hi-Res-related prominent actions.
    Gold,
    /// Translucent clear glass — low-emphasis chrome buttons (toolbar icons, etc.).
    Glass,
    /// Pill-shaped translucent glass — segmented-control-style controls.
    Capsule,
    /// Low-contrast frosted — secondary actions.
    Subtle,
    /// Red-tinted — destructive actions (delete, remove, eject-without-safety).
    Destructive,
}

struct Palette {
    fill: Color,
    text: Color,
    border: Color,
    pill_radius: bool,
}

fn palette(variant: Variant) -> Palette {
    match variant {
        Variant::Prominent => Palette {
            fill: color::SILVER,
            text: color::OBSIDIAN,
            border: with_alpha(Color::WHITE, 0.25),
            pill_radius: false,
        },
        Variant::Gold => Palette {
            fill: color::HIRES_GOLD,
            text: color::OBSIDIAN,
            border: with_alpha(Color::WHITE, 0.25),
            pill_radius: false,
        },
        Variant::Glass => Palette {
            fill: with_alpha(color::CARD, 0.55),
            text: color::SILVER_LIGHT,
            border: color::BORDER,
            pill_radius: false,
        },
        Variant::Capsule => Palette {
            fill: with_alpha(color::CARD, 0.55),
            text: color::SILVER_LIGHT,
            border: color::BORDER,
            pill_radius: true,
        },
        Variant::Subtle => Palette {
            fill: with_alpha(color::CARD_HOVER, 0.70),
            text: color::SILVER_MUTED,
            border: color::BORDER,
            pill_radius: false,
        },
        Variant::Destructive => Palette {
            fill: with_alpha(color::DESTRUCTIVE, 0.85),
            text: color::SILVER_LIGHT,
            border: with_alpha(color::DESTRUCTIVE_LIGHT, 0.35),
            pill_radius: false,
        },
    }
}

/// Resolves the `iced::widget::button` style for `variant` at the given interaction
/// `status`. Pass as `button(...).style(move |_theme, status| style(variant, status))`.
pub fn style(variant: Variant, status: Status) -> Style {
    let Palette {
        fill,
        text,
        border,
        pill_radius,
    } = palette(variant);

    let background = match status {
        Status::Active => fill,
        Status::Hovered => lighten(fill, 0.08),
        Status::Pressed => darken(fill, 0.06),
        Status::Disabled => with_alpha(fill, 0.35),
    };

    let radius = if pill_radius {
        radius::CAPSULE
    } else {
        radius::BUTTON
    };

    Style {
        background: Some(Background::Color(background)),
        text_color: if matches!(status, Status::Disabled) {
            with_alpha(text, 0.45)
        } else {
            text
        },
        border: Border {
            color: border,
            width: 1.0,
            radius: radius.into(),
        },
        shadow: Shadow {
            color: with_alpha(Color::BLACK, 0.30),
            offset: Vector::new(0.0, 1.0),
            blur_radius: 4.0,
        },
    }
}

/// Darkens `c` toward black by `amount` (0.0 = unchanged, 1.0 = black), alpha preserved.
fn darken(c: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    Color {
        r: c.r * (1.0 - amount),
        g: c.g * (1.0 - amount),
        b: c.b * (1.0 - amount),
        a: c.a,
    }
}
