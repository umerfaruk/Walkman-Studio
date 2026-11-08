//! Typography scale, ported from `crate::theme::font` (itself ported from
//! `WalkmanFont`). egui resolves a face by named font *family* (one family per weight);
//! iced resolves a face by `iced::Font` (family name + `font::Weight`), so this scale is
//! expressed directly as `(size, weight)` pairs instead of needing per-weight family
//! aliases. Bundled-face loading (the egui port's `install_fonts`) is a later milestone;
//! until then these fall back to iced's default font at the given size/weight.

use iced::{font::Weight, Pixels};

/// A resolved `(size, weight)` text style. Apply via
/// `text("…").size(STYLE.size).font(Font { weight: STYLE.weight, ..Font::DEFAULT })`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub size: Pixels,
    pub weight: Weight,
}

const fn sans(size: f32, weight: Weight) -> Style {
    Style {
        size: Pixels(size),
        weight,
    }
}

// --- Large & section headings ---
/// 22pt bold.
pub const TITLE1: Style = sans(22.0, Weight::Bold);
/// 18pt bold.
pub const TITLE2: Style = sans(18.0, Weight::Bold);
/// Alias of [`TITLE2`] (`WalkmanFont.title`).
pub const TITLE: Style = TITLE2;
/// 15pt semibold.
pub const TITLE3: Style = sans(15.0, Weight::Semibold);
/// 14pt semibold.
pub const HEADLINE: Style = sans(14.0, Weight::Semibold);

// --- Primary body text (macOS standard 13pt) ---
/// 13pt regular.
pub const BODY: Style = sans(13.0, Weight::Normal);
/// 13pt medium.
pub const BODY_MEDIUM: Style = sans(13.0, Weight::Medium);
/// 13pt semibold.
pub const BODY_BOLD: Style = sans(13.0, Weight::Semibold);

// --- Secondary & callouts ---
/// 12pt regular.
pub const CALLOUT: Style = sans(12.0, Weight::Normal);
/// 12pt medium.
pub const CALLOUT_MEDIUM: Style = sans(12.0, Weight::Medium);
/// 12pt semibold.
pub const CALLOUT_BOLD: Style = sans(12.0, Weight::Semibold);

// --- Subheadlines & captions ---
/// 11.5pt regular.
pub const SUBHEADLINE: Style = sans(11.5, Weight::Normal);
/// 11.5pt medium.
pub const SUBHEADLINE_MEDIUM: Style = sans(11.5, Weight::Medium);
/// 11.5pt bold.
pub const SUBHEADLINE_BOLD: Style = sans(11.5, Weight::Bold);
/// 11pt regular.
pub const CAPTION: Style = sans(11.0, Weight::Normal);
/// 11pt bold.
pub const CAPTION_BOLD: Style = sans(11.0, Weight::Bold);
/// 11pt bold (section header).
pub const SECTION_HEADER: Style = sans(11.0, Weight::Bold);

// --- Monospaced audio metadata & timestamps (family applied via `Font::MONOSPACE`
// at the call site until a bundled mono face lands) ---
/// 11.5pt monospace.
pub const MONO: Style = sans(11.5, Weight::Normal);
/// 10.5pt monospace.
pub const MONO_SMALL: Style = sans(10.5, Weight::Normal);

// --- Compact badges & hardware indicators ---
/// 9.5pt bold monospace.
pub const BADGE: Style = sans(9.5, Weight::Bold);
/// 10pt bold rounded.
pub const PILL_COUNTER: Style = sans(10.0, Weight::Bold);
