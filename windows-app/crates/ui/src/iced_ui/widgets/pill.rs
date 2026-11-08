//! Pill/badge container styles, ported from `crate::theme::widgets` (`pill_style`,
//! `hi_res_badge`, `paint_icon_badge`). Flat fill + border, no gloss painting (see the
//! module doc on `super::glass` for why).

use crate::iced_ui::theme::color::{self, with_alpha};
use crate::iced_ui::theme::sizing::radius;
use iced::widget::container::Style;
use iced::{Background, Border, Color};

/// Capsule status pill (sidebar nav badges, filter chips). `active` selects the
/// brighter fill used for the currently-selected pill/tab.
pub fn pill(active: bool) -> Style {
    Style {
        background: Some(Background::Color(if active {
            with_alpha(Color::WHITE, 0.14)
        } else {
            with_alpha(Color::WHITE, 0.06)
        })),
        border: Border {
            color: color::BORDER,
            width: 1.0,
            radius: radius::CAPSULE.into(),
        },
        ..Style::default()
    }
}

/// Gold "HR" Hi-Res badge chip.
pub fn hi_res_badge() -> Style {
    Style {
        background: Some(Background::Color(color::HIRES_GOLD)),
        border: Border {
            color: with_alpha(color::HIRES_GOLD_LIGHT, 0.6),
            width: 1.0,
            radius: radius::HIRES_BADGE.into(),
        },
        text_color: Some(color::OBSIDIAN),
        ..Style::default()
    }
}

/// Square glass icon chip (sidebar/toolbar icon backgrounds).
pub fn icon_badge() -> Style {
    Style {
        background: Some(Background::Color(with_alpha(color::CARD_HOVER, 0.70))),
        border: Border {
            color: color::BORDER,
            width: 1.0,
            radius: radius::ICON_BADGE.into(),
        },
        ..Style::default()
    }
}
