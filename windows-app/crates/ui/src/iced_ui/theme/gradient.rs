//! Linear gradients, ported from `crate::theme::gradient` (itself ported from
//! `WalkmanTheme`). Kept as CPU-side two-stop gradients (not an `iced::Background`
//! directly) so widgets can either sample a flat color out of them (cheap, used by
//! `widgets::gauge` for now) or build an `iced::gradient::Linear` from the same stops
//! once a widget actually needs a painted gradient fill.

use super::color;
use iced::Color;

/// Builds an opaque grey from SwiftUI's `Color(white:)` (0.0 black … 1.0 white).
const fn white(level: f32) -> Color {
    Color {
        r: level,
        g: level,
        b: level,
        a: 1.0,
    }
}

/// Direction of a two-stop linear gradient, matching the SwiftUI start/end points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    /// SwiftUI `.topLeading` → `.bottomTrailing` (diagonal).
    TopLeadingToBottomTrailing,
    /// SwiftUI `.leading` → `.trailing` (left → right).
    LeadingToTrailing,
    /// SwiftUI `.top` → `.bottom` (vertical).
    TopToBottom,
}

/// A two-stop linear gradient: `start` at t=0, `end` at t=1, along `direction`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearGradient {
    pub start: Color,
    pub end: Color,
    pub direction: Direction,
}

impl LinearGradient {
    /// Linearly interpolates between `start` and `end` at `t` (clamped to `0.0..=1.0`).
    pub fn sample(&self, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let lerp = |a: f32, b: f32| a + (b - a) * t;
        Color {
            r: lerp(self.start.r, self.end.r),
            g: lerp(self.start.g, self.end.g),
            b: lerp(self.start.b, self.end.b),
            a: lerp(self.start.a, self.end.a),
        }
    }
}

/// Hi-Res gold gradient: `hiResGoldLight` → `hiResGoldDark`, top-leading→bottom-trailing.
pub const HIRES_GOLD: LinearGradient = LinearGradient {
    start: color::HIRES_GOLD_LIGHT,
    end: color::HIRES_GOLD_DARK,
    direction: Direction::TopLeadingToBottomTrailing,
};

/// Brushed-silver gradient: white .90 → white .78, top-leading→bottom-trailing.
/// (`goldGradient` on the macOS side is an alias of this.)
pub const SILVER: LinearGradient = LinearGradient {
    start: white(0.90),
    end: white(0.78),
    direction: Direction::TopLeadingToBottomTrailing,
};

/// Storage-gauge gradient: white .60 → white .85, left→right.
pub const STORAGE: LinearGradient = LinearGradient {
    start: white(0.60),
    end: white(0.85),
    direction: Direction::LeadingToTrailing,
};

/// Card-header gradient: white .24 → white .16, top-leading→bottom-trailing.
pub const CARD_HEADER: LinearGradient = LinearGradient {
    start: white(0.24),
    end: white(0.16),
    direction: Direction::TopLeadingToBottomTrailing,
};
