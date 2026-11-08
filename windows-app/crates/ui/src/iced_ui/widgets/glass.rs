//! Glass surface styles, ported from `crate::theme::glass` (itself ported from
//! `LiquidGlassModifier` / `StudioGlassCardModifier` / `ModalGlassChassisModifier`).
//! No backdrop blur (iced has no native blur primitive, and the real macOS source only
//! uses `.ultraThinMaterial` for the modal chassis and the full-screen now-playing
//! background — both approximated here as flat translucent fills, see the migration
//! plan's "Design parity adjustments").

use crate::iced_ui::theme::color::{self, with_alpha};
use iced::widget::container::Style;
use iced::{Background, Border, Color, Shadow, Vector};

/// Base card glass (macOS `liquidGlass()`): card @78% (or @85% selected/hovered), 12pt
/// radius, soft black shadow.
pub fn liquid_glass(selected: bool) -> Style {
    let fill = if selected {
        with_alpha(color::CARD_HOVER, 0.85)
    } else {
        with_alpha(color::CARD, 0.78)
    };
    Style {
        background: Some(Background::Color(fill)),
        border: Border {
            color: color::BORDER,
            width: 1.0,
            radius: crate::iced_ui::theme::sizing::radius::LIQUID_GLASS.into(),
        },
        shadow: Shadow {
            color: with_alpha(Color::BLACK, 0.35),
            offset: Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        },
        ..Style::default()
    }
}

/// Elevated studio card (macOS `studioGlassCard()`): card @88%, 14pt radius, stronger
/// shadow and border than [`liquid_glass`].
pub fn studio_glass_card() -> Style {
    Style {
        background: Some(Background::Color(with_alpha(color::CARD, 0.88))),
        border: Border {
            color: with_alpha(Color::WHITE, 0.10),
            width: 1.0,
            radius: crate::iced_ui::theme::sizing::radius::STUDIO_CARD.into(),
        },
        shadow: Shadow {
            color: with_alpha(Color::BLACK, 0.40),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 10.0,
        },
        ..Style::default()
    }
}

/// Modal chassis (macOS `modalGlassChassis()`): card @92%, 16pt radius, heaviest shadow
/// of the three surfaces — used for every sheet/modal's outer panel.
pub fn modal_glass_chassis() -> Style {
    Style {
        background: Some(Background::Color(with_alpha(color::CARD, 0.92))),
        border: Border {
            color: with_alpha(Color::WHITE, 0.12),
            width: 1.5,
            radius: crate::iced_ui::theme::sizing::radius::MODAL_CHASSIS.into(),
        },
        shadow: Shadow {
            color: with_alpha(Color::BLACK, 0.55),
            offset: Vector::new(0.0, 12.0),
            blur_radius: 28.0,
        },
        ..Style::default()
    }
}

/// Dimmed full-window backdrop shown behind a modal chassis.
pub fn modal_backdrop() -> Style {
    Style {
        background: Some(Background::Color(with_alpha(Color::BLACK, 0.55))),
        ..Style::default()
    }
}
