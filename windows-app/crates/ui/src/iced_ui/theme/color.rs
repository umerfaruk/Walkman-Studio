//! "Obsidian & Silver" color palette. Values match `WalkmanTheme` (Swift) and
//! `crate::theme::color` (the egui port) — same hex values, `iced::Color` instead of
//! `egui::Color32`.

use iced::Color;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

const fn white_alpha(a: f32) -> Color {
    Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a,
    }
}

// --- Obsidian & slate surfaces (flat, non-gradient) ---
/// #101114 — deep matte body / app background.
pub const OBSIDIAN: Color = rgb(16, 17, 20);
pub const BACKGROUND: Color = OBSIDIAN;
/// #18191D — minimal card / surface.
pub const CARD: Color = rgb(24, 25, 29);
pub const SURFACE: Color = CARD;
/// #212126 — card hover.
pub const CARD_HOVER: Color = rgb(33, 33, 38);
/// #141519 — input field fill.
pub const INPUT: Color = rgb(20, 21, 25);
/// Border = white @ 8% opacity (per `Theme.swift`; the egui port's `BORDER` constant
/// mis-ported this as an opaque dark grey — fixed here).
pub const BORDER: Color = white_alpha(0.08);

// --- Silver / platinum palette ---
/// #DCE0E8 — minimal crisp silver.
pub const SILVER: Color = rgb(220, 224, 232);
/// #F2F4F8 — clean platinum.
pub const SILVER_LIGHT: Color = rgb(242, 244, 248);
/// #9BA0AC — brushed aluminum grey.
pub const SILVER_MUTED: Color = rgb(155, 160, 172);
/// #5F6470 — graphite accent.
pub const SILVER_DARK: Color = rgb(95, 100, 112);

// --- Hi-Res gold palette ---
/// #E8B944 — classic Sony Hi-Res gold.
pub const HIRES_GOLD: Color = rgb(232, 185, 68);
/// #F8D470 — gold highlight.
pub const HIRES_GOLD_LIGHT: Color = rgb(248, 212, 112);
/// #C49426 — gold shadow.
pub const HIRES_GOLD_DARK: Color = rgb(196, 148, 38);

// --- Destructive accent (not in the original palette; `LiquidGlassButtonStyle` on the
// macOS side ties `.destructive` to the system red, which has no documented hex here) ---
pub const DESTRUCTIVE: Color = rgb(224, 76, 72);
pub const DESTRUCTIVE_LIGHT: Color = rgb(240, 110, 104);

/// Returns `c` with its alpha replaced by `a`.
pub fn with_alpha(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

/// Lightens `c` toward white by `amount` (0.0 = unchanged, 1.0 = white), alpha preserved.
pub fn lighten(c: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    Color {
        r: c.r + (1.0 - c.r) * amount,
        g: c.g + (1.0 - c.g) * amount,
        b: c.b + (1.0 - c.b) * amount,
        a: c.a,
    }
}
