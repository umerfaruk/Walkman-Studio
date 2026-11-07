//! Design system tokens, ported from `Sources/WalkmanStudio/Theme.swift`.
//!
//! Spec tasks 3.1 (colors + sizing + gradients + typography + font loading) and 3.2
//! (the three reusable glass surfaces — see the [`glass`] module). Button variants and
//! the smaller widgets are added in task 3.3. Colors are expressed as egui [`Color32`]
//! values from the same sRGB hex/opacity used by `WalkmanTheme`.

use egui::{Color32, FontFamily, FontId};

/// "Optical Liquid Glass & Obsidian" color palette. Values match `WalkmanTheme`.
pub mod color {
    use super::Color32;

    // --- Obsidian & slate surfaces (flat, non-gradient) ---
    /// #101114 — deep matte body / app background.
    pub const OBSIDIAN: Color32 = Color32::from_rgb(16, 17, 20);
    pub const BACKGROUND: Color32 = OBSIDIAN;
    /// #18191D — minimal card / surface.
    pub const CARD: Color32 = Color32::from_rgb(24, 25, 29);
    pub const SURFACE: Color32 = CARD;
    /// #212126 — card hover.
    pub const CARD_HOVER: Color32 = Color32::from_rgb(33, 33, 38);
    /// #141519 — input field fill.
    pub const INPUT: Color32 = Color32::from_rgb(20, 21, 25);
    /// Border = white @ 8% opacity.
    pub const BORDER: Color32 = Color32::from_rgba_premultiplied(20, 20, 20, 20);

    // --- Silver / platinum palette ---
    /// #DCE0E8 — minimal crisp silver.
    pub const SILVER: Color32 = Color32::from_rgb(220, 224, 232);
    /// #F2F4F8 — clean platinum.
    pub const SILVER_LIGHT: Color32 = Color32::from_rgb(242, 244, 248);
    /// #9BA0AC — brushed aluminum grey.
    pub const SILVER_MUTED: Color32 = Color32::from_rgb(155, 160, 172);
    /// #5F6470 — graphite accent.
    pub const SILVER_DARK: Color32 = Color32::from_rgb(95, 100, 112);

    // --- Hi-Res gold palette ---
    /// #E8B944 — classic Sony Hi-Res gold.
    pub const HIRES_GOLD: Color32 = Color32::from_rgb(232, 185, 68);
    /// #F8D470 — gold highlight.
    pub const HIRES_GOLD_LIGHT: Color32 = Color32::from_rgb(248, 212, 112);
    /// #C49426 — gold shadow.
    pub const HIRES_GOLD_DARK: Color32 = Color32::from_rgb(196, 148, 38);
}

/// Sizing baselines, ported from the macOS window/layout constants.
pub mod sizing {
    /// Minimum window size (width, height).
    pub const WINDOW_MIN: (f32, f32) = (1320.0, 720.0);
    /// Default window size (width, height).
    pub const WINDOW_DEFAULT: (f32, f32) = (1440.0, 880.0);
    /// Fixed sidebar width.
    pub const SIDEBAR_WIDTH: f32 = 260.0;
    /// Trailing inspector width.
    pub const INSPECTOR_WIDTH: f32 = 300.0;
    /// Max width of the floating now-playing bar.
    pub const PLAYER_MAX_WIDTH: f32 = 1040.0;
    /// Bottom inset reserved on tables so rows clear the floating player.
    pub const TABLE_BOTTOM_INSET: f32 = 88.0;
    /// Shared control-height baseline for toolbar controls / search / sort / buttons.
    pub const CONTROL_BASELINE: f32 = 38.0;
}

/// Linear gradients, ported from `WalkmanTheme`.
///
/// egui has no first-class gradient type, so a gradient is modelled as two endpoint
/// colors plus a direction. Widgets (tasks 3.2–3.3) sample / paint these; a caller can
/// interpolate with [`LinearGradient::sample`] or read the raw endpoints.
///
/// Per the design system, gradients are used **only** on specific controls (buttons,
/// storage bars, specular sheens) — never as surface backgrounds. Surfaces stay flat
/// obsidian.
pub mod gradient {
    use super::Color32;

    /// Direction of a two-stop linear gradient, matching the SwiftUI start/end points.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Direction {
        /// SwiftUI `.topLeading` → `.bottomTrailing` (diagonal).
        TopLeadingToBottomTrailing,
        /// SwiftUI `.leading` → `.trailing` (left → right).
        LeadingToTrailing,
        /// SwiftUI `.top` → `.bottom` (vertical).
        TopToBottom,
    }

    /// A two-stop linear gradient: `start` at t=0, `end` at t=1, along `direction`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LinearGradient {
        pub start: Color32,
        pub end: Color32,
        pub direction: Direction,
    }

    impl LinearGradient {
        /// Linearly interpolates between `start` and `end` at `t` (clamped to `0.0..=1.0`),
        /// matching SwiftUI's component-wise interpolation (including alpha).
        pub fn sample(&self, t: f32) -> Color32 {
            let t = t.clamp(0.0, 1.0);
            let lerp = |a: u8, b: u8| -> u8 {
                (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8
            };
            let [sr, sg, sb, sa] = self.start.to_array();
            let [er, eg, eb, ea] = self.end.to_array();
            Color32::from_rgba_unmultiplied(
                lerp(sr, er),
                lerp(sg, eg),
                lerp(sb, eb),
                lerp(sa, ea),
            )
        }
    }

    /// Builds an opaque grey from SwiftUI's `Color(white:)` (0.0 black … 1.0 white).
    const fn white(level: f32) -> Color32 {
        let v = (level * 255.0 + 0.5) as u8;
        Color32::from_rgb(v, v, v)
    }

    /// Hi-Res gold gradient: `hiResGoldLight` → `hiResGoldDark`, top-leading→bottom-trailing.
    /// (SwiftUI `WalkmanTheme.hiResGoldGradient`.)
    pub const HIRES_GOLD: LinearGradient = LinearGradient {
        start: super::color::HIRES_GOLD_LIGHT,
        end: super::color::HIRES_GOLD_DARK,
        direction: Direction::TopLeadingToBottomTrailing,
    };

    /// Brushed-silver gradient: white .90 → white .78, top-leading→bottom-trailing.
    /// (`WalkmanTheme.silverGradient`; `goldGradient` is an alias of this.)
    pub const SILVER: LinearGradient = LinearGradient {
        start: white(0.90),
        end: white(0.78),
        direction: Direction::TopLeadingToBottomTrailing,
    };

    /// Storage-gauge gradient: white .60 → white .85, left→right.
    /// (`WalkmanTheme.storageGradient`.)
    pub const STORAGE: LinearGradient = LinearGradient {
        start: white(0.60),
        end: white(0.85),
        direction: Direction::LeadingToTrailing,
    };

    /// Card-header gradient: white .24 → white .16, top-leading→bottom-trailing.
    /// (`WalkmanTheme.cardHeaderGradient`.)
    pub const CARD_HEADER: LinearGradient = LinearGradient {
        start: white(0.24),
        end: white(0.16),
        direction: Direction::TopLeadingToBottomTrailing,
    };
}

/// Named font families registered into the egui font registry by [`install_fonts`].
///
/// egui selects a glyph face via [`FontFamily`], not a numeric weight, so each weight we
/// use is registered as its own named family. When a bundled face for a given weight is
/// missing, [`install_fonts`] aliases it to the nearest available face so text still
/// renders.
pub mod family {
    /// Regular-weight proportional sans (Segoe UI Regular when bundled).
    pub const SANS: &str = "walkman-sans";
    /// Medium-weight proportional sans.
    pub const SANS_MEDIUM: &str = "walkman-sans-medium";
    /// Semibold-weight proportional sans.
    pub const SANS_SEMIBOLD: &str = "walkman-sans-semibold";
    /// Bold-weight proportional sans.
    pub const SANS_BOLD: &str = "walkman-sans-bold";
    /// Monospace face (metadata, timestamps, badges).
    pub const MONO: &str = "walkman-mono";
    /// Bold monospace (compact badges).
    pub const MONO_BOLD: &str = "walkman-mono-bold";
    /// Rounded face used for pill counters.
    pub const ROUNDED_BOLD: &str = "walkman-rounded-bold";
}

/// Typography scale, ported from `WalkmanFont` (sizes/weights per requirement 2.3).
///
/// Each constant is an egui [`FontId`] pairing a point size with a named [`FontFamily`]
/// (see [`family`]). Weight is expressed through the chosen family, since egui resolves a
/// face by family name rather than a numeric weight.
pub mod font {
    use super::{family, FontFamily, FontId};

    fn face_sans(size: f32) -> FontId {
        FontId::new(size, FontFamily::Name(family::SANS.into()))
    }
    fn face_sans_medium(size: f32) -> FontId {
        FontId::new(size, FontFamily::Name(family::SANS_MEDIUM.into()))
    }
    fn face_sans_semibold(size: f32) -> FontId {
        FontId::new(size, FontFamily::Name(family::SANS_SEMIBOLD.into()))
    }
    fn face_sans_bold(size: f32) -> FontId {
        FontId::new(size, FontFamily::Name(family::SANS_BOLD.into()))
    }
    fn face_mono(size: f32) -> FontId {
        FontId::new(size, FontFamily::Name(family::MONO.into()))
    }
    fn face_mono_bold(size: f32) -> FontId {
        FontId::new(size, FontFamily::Name(family::MONO_BOLD.into()))
    }
    fn face_rounded_bold(size: f32) -> FontId {
        FontId::new(size, FontFamily::Name(family::ROUNDED_BOLD.into()))
    }

    // --- Large & section headings ---
    /// 22pt bold.
    pub fn title1() -> FontId {
        face_sans_bold(22.0)
    }
    /// 18pt bold.
    pub fn title2() -> FontId {
        face_sans_bold(18.0)
    }
    /// Alias of [`title2`] (`WalkmanFont.title`).
    pub fn title() -> FontId {
        title2()
    }
    /// 15pt semibold.
    pub fn title3() -> FontId {
        face_sans_semibold(15.0)
    }
    /// 14pt semibold.
    pub fn headline() -> FontId {
        face_sans_semibold(14.0)
    }

    // --- Primary body text (macOS standard 13pt) ---
    /// 13pt regular.
    pub fn body() -> FontId {
        face_sans(13.0)
    }
    /// 13pt medium.
    pub fn body_medium() -> FontId {
        face_sans_medium(13.0)
    }
    /// 13pt semibold.
    pub fn body_bold() -> FontId {
        face_sans_semibold(13.0)
    }

    // --- Secondary & callouts ---
    /// 12pt regular.
    pub fn callout() -> FontId {
        face_sans(12.0)
    }
    /// 12pt medium.
    pub fn callout_medium() -> FontId {
        face_sans_medium(12.0)
    }
    /// 12pt semibold.
    pub fn callout_bold() -> FontId {
        face_sans_semibold(12.0)
    }

    // --- Subheadlines & captions ---
    /// 11.5pt regular.
    pub fn subheadline() -> FontId {
        face_sans(11.5)
    }
    /// 11.5pt medium.
    pub fn subheadline_medium() -> FontId {
        face_sans_medium(11.5)
    }
    /// 11.5pt bold.
    pub fn subheadline_bold() -> FontId {
        face_sans_bold(11.5)
    }
    /// 11pt regular.
    pub fn caption() -> FontId {
        face_sans(11.0)
    }
    /// 11pt bold.
    pub fn caption_bold() -> FontId {
        face_sans_bold(11.0)
    }
    /// 11pt bold (section header).
    pub fn section_header() -> FontId {
        face_sans_bold(11.0)
    }

    // --- Monospaced audio metadata & timestamps ---
    /// 11.5pt monospace.
    pub fn mono() -> FontId {
        face_mono(11.5)
    }
    /// 10.5pt monospace.
    pub fn mono_small() -> FontId {
        face_mono(10.5)
    }

    // --- Compact badges & hardware indicators ---
    /// 9.5pt bold monospace.
    pub fn badge() -> FontId {
        face_mono_bold(9.5)
    }
    /// 10pt bold rounded.
    pub fn pill_counter() -> FontId {
        face_rounded_bold(10.0)
    }
}

/// Installs the bundled fonts into the egui font registry and registers the named
/// [`family`] entries used by the [`font`] scale.
///
/// ## Bundled faces
///
/// Segoe UI is the Windows default sans and the closest visual match to the macOS system
/// font at these sizes; a monospace face covers metadata/timestamps/badges. Font files
/// are loaded from `windows-app/assets/fonts/` at build time via [`include_bytes!`] *when
/// present* (the slot is documented in `assets/README.md`). Segoe UI ships with Windows
/// but its license does not allow redistribution, so the files are git-ignored and absent
/// on non-Windows dev hosts.
///
/// ## Graceful fallback
///
/// When a bundled face is missing, its named family is aliased to the nearest available
/// face: bundled sans/mono if present, otherwise egui's built-in proportional/monospace
/// fonts (enabled by the `default_fonts` eframe feature). This keeps every [`font`]
/// constant renderable on any host, so the app never launches with empty text.
pub fn install_fonts(ctx: &egui::Context) {
    use egui::{FontData, FontDefinitions};
    use std::sync::Arc;

    let mut fonts = FontDefinitions::default();

    // Try to load bundled faces. `OPTIONAL_*` are `Option<&[u8]>` so a missing file
    // (non-Windows dev host) compiles to `None` and we fall back below.
    let bundled_sans = OPTIONAL_SANS_REGULAR.map(|b| register(&mut fonts, "walkman-sans-regular-face", b));
    let bundled_sans_medium =
        OPTIONAL_SANS_MEDIUM.map(|b| register(&mut fonts, "walkman-sans-medium-face", b));
    let bundled_sans_semibold =
        OPTIONAL_SANS_SEMIBOLD.map(|b| register(&mut fonts, "walkman-sans-semibold-face", b));
    let bundled_sans_bold =
        OPTIONAL_SANS_BOLD.map(|b| register(&mut fonts, "walkman-sans-bold-face", b));
    let bundled_mono = OPTIONAL_MONO.map(|b| register(&mut fonts, "walkman-mono-face", b));

    // egui's built-in fallbacks (present via the `default_fonts` feature).
    const BUILTIN_SANS: &str = "Ubuntu-Light";
    const BUILTIN_MONO: &str = "Hack";

    let sans_regular = bundled_sans.clone().unwrap_or_else(|| BUILTIN_SANS.to_owned());
    let sans_medium = bundled_sans_medium
        .clone()
        .or_else(|| bundled_sans_bold.clone())
        .unwrap_or_else(|| sans_regular.clone());
    let sans_semibold = bundled_sans_semibold
        .clone()
        .or_else(|| bundled_sans_bold.clone())
        .unwrap_or_else(|| sans_medium.clone());
    let sans_bold = bundled_sans_bold
        .clone()
        .or_else(|| bundled_sans_semibold.clone())
        .unwrap_or_else(|| sans_regular.clone());
    let mono_regular = bundled_mono.clone().unwrap_or_else(|| BUILTIN_MONO.to_owned());

    // Map each named family to its resolved face key.
    alias(&mut fonts, family::SANS, &sans_regular);
    alias(&mut fonts, family::SANS_MEDIUM, &sans_medium);
    alias(&mut fonts, family::SANS_SEMIBOLD, &sans_semibold);
    alias(&mut fonts, family::SANS_BOLD, &sans_bold);
    alias(&mut fonts, family::MONO, &mono_regular);
    alias(&mut fonts, family::MONO_BOLD, &mono_regular);
    // No bundled rounded face; the rounded pill counter falls back to the bold sans.
    alias(&mut fonts, family::ROUNDED_BOLD, &sans_bold);

    // Also make the bundled sans/mono the default for egui's built-in Proportional and
    // Monospace families so untyped widgets inherit the Walkman look.
    if let Some(key) = bundled_sans.as_ref() {
        prepend_default(&mut fonts, FontFamily::Proportional, key);
    }
    if let Some(key) = bundled_mono.as_ref() {
        prepend_default(&mut fonts, FontFamily::Monospace, key);
    }

    ctx.set_fonts(fonts);
    return;

    // --- local helpers ---
    fn register(fonts: &mut FontDefinitions, key: &str, bytes: &'static [u8]) -> String {
        fonts
            .font_data
            .insert(key.to_owned(), Arc::new(FontData::from_static(bytes)));
        key.to_owned()
    }
    fn alias(fonts: &mut FontDefinitions, family_name: &str, face_key: &str) {
        fonts
            .families
            .insert(FontFamily::Name(family_name.into()), vec![face_key.to_owned()]);
    }
    fn prepend_default(fonts: &mut FontDefinitions, fam: FontFamily, face_key: &str) {
        fonts
            .families
            .entry(fam)
            .or_default()
            .insert(0, face_key.to_owned());
    }
}

// Bundled font slots. These resolve to `Some(bytes)` only when the files exist under
// `windows-app/assets/fonts/` at build time (see `assets/README.md`); otherwise `None`,
// which drives the graceful fallback in `install_fonts`.
//
// `include_bytes!` requires the file to exist at compile time, so each slot is guarded by
// a build-time `cfg` that a future packaging step (task 19.1) can flip on once the
// Windows font files are vendored. On a non-Windows dev host the files are absent, so the
// slots stay `None` and the build still succeeds.
#[cfg(walkman_bundled_fonts)]
const OPTIONAL_SANS_REGULAR: Option<&[u8]> =
    Some(include_bytes!("../../../assets/fonts/segoeui.ttf"));
#[cfg(not(walkman_bundled_fonts))]
const OPTIONAL_SANS_REGULAR: Option<&[u8]> = None;

#[cfg(walkman_bundled_fonts)]
const OPTIONAL_SANS_MEDIUM: Option<&[u8]> =
    Some(include_bytes!("../../../assets/fonts/segoeui-medium.ttf"));
#[cfg(not(walkman_bundled_fonts))]
const OPTIONAL_SANS_MEDIUM: Option<&[u8]> = None;

#[cfg(walkman_bundled_fonts)]
const OPTIONAL_SANS_SEMIBOLD: Option<&[u8]> =
    Some(include_bytes!("../../../assets/fonts/segoeui-semibold.ttf"));
#[cfg(not(walkman_bundled_fonts))]
const OPTIONAL_SANS_SEMIBOLD: Option<&[u8]> = None;

#[cfg(walkman_bundled_fonts)]
const OPTIONAL_SANS_BOLD: Option<&[u8]> =
    Some(include_bytes!("../../../assets/fonts/segoeui-bold.ttf"));
#[cfg(not(walkman_bundled_fonts))]
const OPTIONAL_SANS_BOLD: Option<&[u8]> = None;

#[cfg(walkman_bundled_fonts)]
const OPTIONAL_MONO: Option<&[u8]> =
    Some(include_bytes!("../../../assets/fonts/CascadiaMono.ttf"));
#[cfg(not(walkman_bundled_fonts))]
const OPTIONAL_MONO: Option<&[u8]> = None;

/// Reusable "glass" surfaces, ported from the SwiftUI view modifiers in
/// `Theme.swift` (`LiquidGlassModifier`, `StudioGlassCardModifier`,
/// `ModalGlassChassisModifier`).
///
/// ## Design rules (requirement 2.6)
///
/// These are **flat** surfaces. Depth comes *only* from neutral black drop shadows and
/// crisp 1px (white-tinted) borders — there is **zero glow/bloom** and **zero background
/// gradient on the surface itself**. The only gradients used are the thin specular
/// *sheens* along the top edge of [`studio_glass_card`] / [`modal_glass_chassis`]; the
/// surface fill underneath them is always a flat [`color::CARD`] at a fixed opacity.
///
/// ## egui mapping
///
/// Each surface is produced as an [`egui::Frame`] carrying the fill, corner [`Rounding`],
/// border [`Stroke`], and drop [`Shadow`]. A caller renders content with
/// `frame.show(ui, |ui| { … })`. The sheen (which egui's `Frame` cannot express, since it
/// has no gradient fill) is a separate overlay: [`studio_glass_card`] /
/// [`modal_glass_chassis`] return the frame, and [`paint_top_sheen`] paints the matching
/// specular gradient onto the top edge of a rect after the frame is laid out. A
/// convenience [`StudioSurface`] bundles the two so views can draw a sheened card in one
/// call.
///
/// Opacities use SwiftUI semantics: `Color.white.opacity(x)` → [`white_alpha`],
/// `Color.black.opacity(x)` → [`Color32::from_black_alpha`]. egui's [`Shadow::blur`] is
/// the fuzzy penumbra width, so it carries the SwiftUI `shadow(radius:)` value.
pub mod glass {
    use super::{color, Color32};
    use egui::{Frame, Rounding, Shadow, Stroke, Vec2};

    /// Corner radius of a [`liquid_glass`] surface (SwiftUI default `cornerRadius: 12`).
    pub const LIQUID_RADIUS: f32 = 12.0;
    /// Corner radius of a [`studio_glass_card`] surface (`cornerRadius: 14`).
    pub const STUDIO_RADIUS: f32 = 14.0;
    /// Corner radius of a [`modal_glass_chassis`] surface (`cornerRadius: 16`).
    pub const MODAL_RADIUS: f32 = 16.0;

    /// Surface fill opacity of [`liquid_glass`] (`WalkmanTheme.card.opacity(0.78)`).
    pub const LIQUID_FILL: f32 = 0.78;
    /// Surface fill opacity of [`studio_glass_card`] (`card.opacity(0.88)`).
    pub const STUDIO_FILL: f32 = 0.88;
    /// Surface fill opacity of [`modal_glass_chassis`] (`card.opacity(0.92)`).
    pub const MODAL_FILL: f32 = 0.92;

    /// Height of the top specular sheen on [`studio_glass_card`] (SwiftUI `frame(height: 8)`).
    pub const STUDIO_SHEEN_HEIGHT: f32 = 8.0;
    /// Height of the top specular sheen on [`modal_glass_chassis`] (`frame(height: 10)`).
    pub const MODAL_SHEEN_HEIGHT: f32 = 10.0;

    /// `base * borderOpacity` for the default `liquid_glass` `borderOpacity` of `0.35`.
    /// Resting (unhovered, unselected) border: `0.35 * 0.20`.
    pub const LIQUID_BORDER_REST: f32 = 0.35 * 0.20;
    /// Hovered border: `0.35 * 0.35`.
    pub const LIQUID_BORDER_HOVER: f32 = 0.35 * 0.35;
    /// Selected border (silver, not plain white): `0.35 * 0.95`.
    pub const LIQUID_BORDER_SELECTED: f32 = 0.35 * 0.95;

    /// Resting border line width for `liquid_glass`.
    pub const LIQUID_STROKE_WIDTH: f32 = 1.0;
    /// Selected border line width for `liquid_glass`.
    pub const LIQUID_STROKE_WIDTH_SELECTED: f32 = 1.5;

    /// Drop-shadow blur radius of `liquid_glass` at rest (`shadow(radius: 6)`).
    pub const LIQUID_SHADOW_RADIUS: f32 = 6.0;
    /// Drop-shadow blur radius of `liquid_glass` on hover (`radius: 10`).
    pub const LIQUID_SHADOW_RADIUS_HOVER: f32 = 10.0;
    /// Neutral black shadow opacity for `liquid_glass` (`Color.black.opacity(0.24)`).
    pub const LIQUID_SHADOW_ALPHA: f32 = 0.24;

    /// `studio_glass_card` shadow radius / opacity (`radius: 8`, `black.opacity(0.22)`).
    pub const STUDIO_SHADOW_RADIUS: f32 = 8.0;
    pub const STUDIO_SHADOW_ALPHA: f32 = 0.22;
    /// `modal_glass_chassis` shadow radius / opacity (`radius: 28`, `black.opacity(0.45)`).
    pub const MODAL_SHADOW_RADIUS: f32 = 28.0;
    pub const MODAL_SHADOW_ALPHA: f32 = 0.45;

    /// Border stroke width for the studio card / modal chassis rims (`lineWidth: 0.85`).
    pub const RIM_STROKE_WIDTH: f32 = 0.85;

    /// `Color.white.opacity(x)` → a non-premultiplied white [`Color32`] with alpha `x`.
    ///
    /// Matches SwiftUI's gamma-space opacity so `white_alpha(0.08)` is exactly the
    /// design's "white @ 8%".
    pub fn white_alpha(opacity: f32) -> Color32 {
        let a = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        Color32::from_white_alpha(a)
    }

    /// `Color.black.opacity(x)` → a black [`Color32`] with alpha `x` (used for shadows).
    fn black_alpha(opacity: f32) -> Color32 {
        let a = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        Color32::from_black_alpha(a)
    }

    /// Interaction state for [`liquid_glass`]. Mirrors the SwiftUI `isHovered` /
    /// `isSelected` flags, which drive border opacity/width and shadow radius.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum State {
        /// Resting surface: faint white rim, soft shadow.
        #[default]
        Normal,
        /// Pointer hovering: brighter rim, deeper shadow.
        Hovered,
        /// Selected: crisp silver rim at 1.5px (shadow stays at the resting radius, as
        /// in the SwiftUI modifier, which keys shadow radius off `isHovered` only).
        Selected,
    }

    /// The flat obsidian card at [`LIQUID_FILL`] opacity.
    fn liquid_fill() -> Color32 {
        color::CARD.gamma_multiply(LIQUID_FILL)
    }

    /// `liquid_glass(cornerRadius: 12)` — the workhorse card surface.
    ///
    /// Flat [`color::CARD`] @ ~78% fill, a 1px white rim whose opacity tracks `state`
    /// (and whose color/width switch to crisp silver @ 1.5px when selected), and a
    /// neutral black drop shadow (blur 6 at rest, 10 on hover). No glow, no gradient.
    pub fn liquid_glass(state: State) -> Frame {
        let (stroke, shadow_radius, shadow_offset_y) = match state {
            State::Normal => (
                Stroke::new(LIQUID_STROKE_WIDTH, white_alpha(LIQUID_BORDER_REST)),
                LIQUID_SHADOW_RADIUS,
                2.0,
            ),
            State::Hovered => (
                Stroke::new(LIQUID_STROKE_WIDTH, white_alpha(LIQUID_BORDER_HOVER)),
                LIQUID_SHADOW_RADIUS_HOVER,
                4.0,
            ),
            State::Selected => (
                Stroke::new(
                    LIQUID_STROKE_WIDTH_SELECTED,
                    silver_alpha(LIQUID_BORDER_SELECTED),
                ),
                LIQUID_SHADOW_RADIUS,
                2.0,
            ),
        };

        Frame::none()
            .fill(liquid_fill())
            .rounding(Rounding::same(LIQUID_RADIUS))
            .stroke(stroke)
            .shadow(Shadow {
                offset: Vec2::new(0.0, shadow_offset_y),
                blur: shadow_radius,
                spread: 0.0,
                color: black_alpha(LIQUID_SHADOW_ALPHA),
            })
    }

    /// Crisp silver rim color used for the selected `liquid_glass` state
    /// (`WalkmanTheme.silverLight.opacity(..)`).
    fn silver_alpha(opacity: f32) -> Color32 {
        let a = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        Color32::from_rgba_unmultiplied(
            color::SILVER_LIGHT.r(),
            color::SILVER_LIGHT.g(),
            color::SILVER_LIGHT.b(),
            a,
        )
    }

    /// `studio_glass_card(cornerRadius: 14)` — a studio panel surface.
    ///
    /// Flat [`color::CARD`] @ ~88% fill, an 8px top specular sheen (painted separately by
    /// [`paint_top_sheen`] / [`StudioSurface`]), a thin white rim (0.85px), and a neutral
    /// black drop shadow (blur 8). The returned [`Frame`] carries everything except the
    /// sheen gradient, which egui's `Frame` cannot express.
    pub fn studio_glass_card() -> Frame {
        Frame::none()
            .fill(color::CARD.gamma_multiply(STUDIO_FILL))
            .rounding(Rounding::same(STUDIO_RADIUS))
            .stroke(Stroke::new(RIM_STROKE_WIDTH, white_alpha(0.24)))
            .shadow(Shadow {
                offset: Vec2::new(0.0, 3.0),
                blur: STUDIO_SHADOW_RADIUS,
                spread: 0.0,
                color: black_alpha(STUDIO_SHADOW_ALPHA),
            })
    }

    /// `modal_glass_chassis(cornerRadius: 16)` — the chassis behind modal sheets.
    ///
    /// Flat [`color::CARD`] @ ~92% fill over a translucent/acrylic backing (the window's
    /// blurred backdrop on the host), a 10px top specular sheen (painted by
    /// [`paint_top_sheen`] / [`StudioSurface`]), a brighter white rim (0.85px), and a
    /// strong neutral black drop shadow (blur 28). No glow.
    pub fn modal_glass_chassis() -> Frame {
        Frame::none()
            .fill(color::CARD.gamma_multiply(MODAL_FILL))
            .rounding(Rounding::same(MODAL_RADIUS))
            .stroke(Stroke::new(RIM_STROKE_WIDTH, white_alpha(0.35)))
            .shadow(Shadow {
                offset: Vec2::new(0.0, 12.0),
                blur: MODAL_SHADOW_RADIUS,
                spread: 0.0,
                color: black_alpha(MODAL_SHADOW_ALPHA),
            })
    }

    /// Which specular sheen to paint along the top edge of a surface.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Sheen {
        /// 8px sheen for [`studio_glass_card`]: white .16 → .03 → clear, top→bottom.
        Studio,
        /// 10px sheen for [`modal_glass_chassis`]: white .24 → .05 → clear, top→bottom.
        Modal,
    }

    impl Sheen {
        /// Sheen band height in points.
        pub fn height(self) -> f32 {
            match self {
                Sheen::Studio => STUDIO_SHEEN_HEIGHT,
                Sheen::Modal => MODAL_SHEEN_HEIGHT,
            }
        }

        /// The three vertical gradient stops `(location, white-opacity)`, top → bottom.
        /// The final stop is fully clear (alpha 0) so the sheen fades into the flat fill.
        pub fn stops(self) -> [(f32, f32); 3] {
            match self {
                Sheen::Studio => [(0.0, 0.16), (0.5, 0.03), (1.0, 0.0)],
                Sheen::Modal => [(0.0, 0.24), (0.5, 0.05), (1.0, 0.0)],
            }
        }
    }

    /// Paints the top specular [`Sheen`] onto `rect` (the laid-out surface rect), clipped
    /// to the surface corner `rounding` at the top. The gradient runs vertically from the
    /// top edge down `sheen.height()` points, fading white → faint white → clear, exactly
    /// as the SwiftUI modifiers overlay it. This is the *only* gradient on a glass
    /// surface; the fill underneath stays flat.
    ///
    /// Approximates the continuous gradient with stacked 1px horizontal slices (egui has
    /// no native vertical-gradient fill). Call *after* the surface frame is drawn.
    pub fn paint_top_sheen(painter: &egui::Painter, rect: egui::Rect, sheen: Sheen, rounding: f32) {
        use egui::Rect;

        let height = sheen.height().min(rect.height());
        if height <= 0.0 || rect.width() <= 0.0 {
            return;
        }
        let stops = sheen.stops();

        // Linear interpolation of white-opacity across the stop list at parameter `t`.
        let alpha_at = |t: f32| -> f32 {
            let t = t.clamp(0.0, 1.0);
            let mut prev = stops[0];
            for &next in &stops[1..] {
                if t <= next.0 {
                    let span = (next.0 - prev.0).max(f32::EPSILON);
                    let k = (t - prev.0) / span;
                    return prev.1 + (next.1 - prev.1) * k;
                }
                prev = next;
            }
            stops[stops.len() - 1].1
        };

        let slices = height.ceil() as usize;
        for i in 0..slices {
            let y0 = rect.top() + i as f32;
            let y1 = (y0 + 1.0).min(rect.top() + height);
            if y1 <= y0 {
                break;
            }
            let t = (i as f32 + 0.5) / height;
            let alpha = alpha_at(t);
            if alpha <= 0.0 {
                continue;
            }
            // Round the top corners of the first slice so the sheen hugs the surface.
            let slice = Rect::from_min_max(
                egui::pos2(rect.left(), y0),
                egui::pos2(rect.right(), y1),
            );
            let slice_rounding = if i == 0 {
                Rounding {
                    nw: rounding,
                    ne: rounding,
                    sw: 0.0,
                    se: 0.0,
                }
            } else {
                Rounding::ZERO
            };
            painter.rect_filled(slice, slice_rounding, white_alpha(alpha));
        }
    }

    /// A sheened studio/modal surface: the [`Frame`] plus the [`Sheen`] and corner radius
    /// needed to paint the specular highlight. Lets a view draw the full surface in one
    /// place without re-deriving the sheen kind.
    #[derive(Debug, Clone, Copy)]
    pub struct StudioSurface {
        pub frame: Frame,
        pub sheen: Sheen,
        pub rounding: f32,
    }

    impl StudioSurface {
        /// A [`studio_glass_card`] bundled with its 8px [`Sheen::Studio`].
        pub fn studio() -> Self {
            Self {
                frame: studio_glass_card(),
                sheen: Sheen::Studio,
                rounding: STUDIO_RADIUS,
            }
        }

        /// A [`modal_glass_chassis`] bundled with its 10px [`Sheen::Modal`].
        pub fn modal() -> Self {
            Self {
                frame: modal_glass_chassis(),
                sheen: Sheen::Modal,
                rounding: MODAL_RADIUS,
            }
        }

        /// Shows `add_contents` inside the frame, then paints the top sheen over the
        /// laid-out surface. A one-call equivalent of the SwiftUI card modifiers.
        pub fn show<R>(
            self,
            ui: &mut egui::Ui,
            add_contents: impl FnOnce(&mut egui::Ui) -> R,
        ) -> egui::InnerResponse<R> {
            let inner = self.frame.show(ui, add_contents);
            paint_top_sheen(ui.painter(), inner.response.rect, self.sheen, self.rounding);
            inner
        }
    }
}

/// The six liquid-glass button variants and the smaller interactive widgets
/// (active/hover pills, the Hi-Res "HR" badge, liquid-glass icon badges, storage
/// gauges), ported from `LiquidGlassButtonStyle`, `LiquidGlassActivePill`,
/// `LiquidGlassHoverPill`, `HiResBadge`, `LiquidGlassIconBadge`, and the
/// `storageGradient` usage in `Theme.swift`.
///
/// ## Design rules (requirement 2.6)
///
/// These are the controls where gradients *are* allowed — button fills, the top
/// specular gloss, and the storage gauge — matching the source. Surfaces elsewhere
/// stay flat obsidian. There is still **zero glow/bloom**: depth comes only from
/// neutral black drop shadows and crisp borders.
///
/// ## egui mapping
///
/// egui draws immediately rather than retaining a view tree, so each widget is a
/// value that paints itself. [`Button`] implements [`egui::Widget`] so callers write
/// `ui.add(theme::button::Button::new("Save", Variant::Prominent))`; the pills,
/// badges, and gauge expose both a pure [`style`] helper (returns the colors/metrics
/// a frame would use — unit-testable without a live `Ui`) and a `show`/`paint`
/// method that renders into a real `Ui`/`Painter`.
pub mod button {
    use super::{color, font, gradient, glass::white_alpha, sizing};
    use egui::{
        Color32, FontId, Rect, Response, Rounding, Sense, Stroke, Ui, Vec2, Widget,
    };

    /// Hover scale factor (SwiftUI `scaleEffect(... 1.025 ...)`).
    pub const HOVER_SCALE: f32 = 1.025;
    /// Press scale factor (SwiftUI `scaleEffect(configuration.isPressed ... 0.97 ...)`).
    pub const PRESS_SCALE: f32 = 0.97;
    /// Default button corner radius (`LiquidGlassButtonStyle(cornerRadius: 8)`).
    pub const CORNER_RADIUS: f32 = 8.0;
    /// Height of the top specular gloss band (`frame(height: 12)`).
    pub const GLOSS_HEIGHT: f32 = 12.0;

    /// The six button variants (`LiquidGlassButtonStyle.Variant`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Variant {
        /// Crisp silver/platinum glass (the default, prominent action).
        Prominent,
        /// Authentic Hi-Res gold glass.
        Gold,
        /// Translucent clear liquid glass.
        Glass,
        /// Pill-shaped translucent glass (same fill as [`Glass`](Variant::Glass),
        /// rendered with a fully-rounded capsule).
        Capsule,
        /// Subtle low-contrast frosted glass.
        Subtle,
        /// Red-tinted warning/delete liquid glass.
        Destructive,
    }

    /// Pointer interaction state driving fill/border/shadow/scale.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum State {
        /// Resting.
        #[default]
        Normal,
        /// Pointer hovering (brighter fill/border, deeper shadow, scale 1.025).
        Hovered,
        /// Pointer pressed (dimmed label, scale 0.97).
        Pressed,
        /// Disabled (dimmed fill/label, no hover/press response).
        Disabled,
    }

    impl State {
        fn is_enabled(self) -> bool {
            !matches!(self, State::Disabled)
        }
        fn is_hovered(self) -> bool {
            matches!(self, State::Hovered)
        }
        fn is_pressed(self) -> bool {
            matches!(self, State::Pressed)
        }
    }

    /// The resolved paint recipe for a button in a given variant + state: the fill
    /// (either a flat tint or a two-stop vertical gradient), the top gloss peak
    /// opacity, the border stroke, the drop shadow, the label opacity, and the scale.
    ///
    /// This mirrors exactly what `LiquidGlassButtonStyle` computes, so it can be
    /// asserted in unit tests without a live `Ui`.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Style {
        /// Top fill color (t=0). For flat fills this equals [`fill_bottom`](Style::fill_bottom).
        pub fill_top: Color32,
        /// Bottom fill color (t=1).
        pub fill_bottom: Color32,
        /// Peak white opacity of the top specular gloss band.
        pub gloss_peak: f32,
        /// Border stroke (top color; the source border is itself a faint gradient, so
        /// this carries the brightest top stop which is what reads at this size).
        pub border: Stroke,
        /// Drop-shadow blur radius.
        pub shadow_blur: f32,
        /// Drop-shadow vertical offset.
        pub shadow_offset_y: f32,
        /// Neutral black drop-shadow opacity.
        pub shadow_alpha: f32,
        /// Label opacity (dimmed when pressed / disabled).
        pub label_opacity: f32,
        /// Scale factor applied to the control.
        pub scale: f32,
    }

    /// `Color.black.opacity(x)` → a black [`Color32`] with alpha `x`.
    fn black_alpha(opacity: f32) -> Color32 {
        Color32::from_black_alpha((opacity.clamp(0.0, 1.0) * 255.0).round() as u8)
    }

    /// `Color.red.opacity(x)` → SwiftUI's system red (≈ `#FF3B30`) at opacity `x`.
    fn red_alpha(opacity: f32) -> Color32 {
        Color32::from_rgba_unmultiplied(255, 59, 48, (opacity.clamp(0.0, 1.0) * 255.0).round() as u8)
    }

    /// Resolve the paint recipe for `variant` in `state`.
    pub fn style(variant: Variant, state: State) -> Style {
        let enabled = state.is_enabled();
        let hovered = state.is_hovered();
        let pressed = state.is_pressed();

        // --- Fill (the one place gradients are allowed on a control) ---
        let (fill_top, fill_bottom) = match variant {
            Variant::Prominent => {
                // Silver/platinum vertical gradient #F5F7FC → #D7DCE6, dimmed when disabled.
                let top = Color32::from_rgb(245, 247, 252);
                let bottom = Color32::from_rgb(215, 220, 230);
                if enabled {
                    (top, bottom)
                } else {
                    (top.gamma_multiply(0.35), bottom.gamma_multiply(0.35))
                }
            }
            Variant::Gold => {
                let g = gradient::HIRES_GOLD;
                if enabled {
                    (g.start, g.end)
                } else {
                    (g.start.gamma_multiply(0.35), g.end.gamma_multiply(0.35))
                }
            }
            Variant::Glass | Variant::Capsule => {
                let a = if hovered && enabled {
                    0.14
                } else if enabled {
                    0.08
                } else {
                    0.04
                };
                let c = white_alpha(a);
                (c, c)
            }
            Variant::Subtle => {
                let a = if hovered && enabled {
                    0.10
                } else if enabled {
                    0.05
                } else {
                    0.025
                };
                let c = white_alpha(a);
                (c, c)
            }
            Variant::Destructive => {
                let a = if hovered && enabled {
                    0.22
                } else if enabled {
                    0.12
                } else {
                    0.06
                };
                let c = red_alpha(a);
                (c, c)
            }
        };

        // --- Top specular gloss peak opacity ---
        let gloss_base = match variant {
            Variant::Prominent => 0.65,
            Variant::Destructive => 0.28,
            _ => 0.35,
        };
        let gloss_peak = gloss_base * if enabled { 1.0 } else { 0.30 };

        // --- Border (a faint white/red gradient in the source; we carry the top stop) ---
        let border_color = if matches!(variant, Variant::Destructive) {
            if hovered && enabled {
                red_alpha(0.55)
            } else {
                red_alpha(0.30)
            }
        } else if hovered && enabled {
            white_alpha(0.60)
        } else if enabled {
            white_alpha(0.38)
        } else {
            white_alpha(0.15)
        };
        let border = Stroke::new(1.0, border_color);

        // --- Shadow (neutral black, deepens on hover; no glow) ---
        let (shadow_alpha, shadow_blur, shadow_offset_y) = if !enabled {
            (0.06, 3.0, 1.5)
        } else if hovered {
            (0.25, 6.0, 3.0)
        } else {
            (0.14, 3.0, 1.5)
        };

        // --- Label opacity + scale ---
        let label_opacity = if !enabled {
            0.40
        } else if pressed {
            0.85
        } else {
            1.0
        };
        let scale = if enabled && pressed {
            super::button::PRESS_SCALE
        } else if enabled && hovered {
            super::button::HOVER_SCALE
        } else {
            1.0
        };

        Style {
            fill_top,
            fill_bottom,
            gloss_peak,
            border,
            shadow_blur,
            shadow_offset_y,
            shadow_alpha,
            label_opacity,
            scale,
        }
    }

    /// A liquid-glass button. Implements [`egui::Widget`], so render it with
    /// `ui.add(Button::new("Save", Variant::Prominent))` and read the returned
    /// [`Response`] (`.clicked()` etc.). Hovering shows a pointer cursor and the
    /// 1.025 scale; pressing shows the 0.97 scale and a dimmed label.
    #[derive(Debug, Clone)]
    pub struct Button {
        text: String,
        variant: Variant,
        corner_radius: f32,
        enabled: bool,
        min_height: f32,
        font: FontId,
    }

    impl Button {
        /// A button with the given label and variant (default corner radius 8, the
        /// 38pt control-height baseline, and the semibold callout label font).
        pub fn new(text: impl Into<String>, variant: Variant) -> Self {
            Self {
                text: text.into(),
                variant,
                corner_radius: CORNER_RADIUS,
                enabled: true,
                min_height: sizing::CONTROL_BASELINE,
                font: font::callout_bold(),
            }
        }

        /// Override the corner radius.
        pub fn corner_radius(mut self, r: f32) -> Self {
            self.corner_radius = r;
            self
        }

        /// Enable/disable the button (disabled buttons dim and ignore pointer input).
        pub fn enabled(mut self, enabled: bool) -> Self {
            self.enabled = enabled;
            self
        }

        /// Override the minimum control height (defaults to the 38pt baseline).
        pub fn min_height(mut self, h: f32) -> Self {
            self.min_height = h;
            self
        }

        /// Effective corner radius: a [`Variant::Capsule`] is fully rounded.
        fn effective_radius(&self, height: f32) -> f32 {
            if matches!(self.variant, Variant::Capsule) {
                height * 0.5
            } else {
                self.corner_radius
            }
        }

        /// Resolve the current pointer [`State`] from an egui [`Response`].
        fn state_from(&self, response: &Response) -> State {
            if !self.enabled {
                State::Disabled
            } else if response.is_pointer_button_down_on() {
                State::Pressed
            } else if response.hovered() {
                State::Hovered
            } else {
                State::Normal
            }
        }
    }

    impl Widget for Button {
        fn ui(self, ui: &mut Ui) -> Response {
            // Measure the label and size the control: label + horizontal padding,
            // clamped to at least the control-height baseline.
            let galley = ui.painter().layout_no_wrap(
                self.text.clone(),
                self.font.clone(),
                color::OBSIDIAN, // placeholder; actual text color chosen below
            );
            let pad_x = 14.0;
            let desired = Vec2::new(
                galley.size().x + pad_x * 2.0,
                self.min_height.max(galley.size().y + 12.0),
            );

            let sense = if self.enabled {
                Sense::click()
            } else {
                Sense::hover()
            };
            let (rect, response) = ui.allocate_at_least(desired, sense);

            let state = self.state_from(&response);
            let st = style(self.variant, state);

            // Pointer cursor on hover (SwiftUI `NSCursor.pointingHand`).
            if self.enabled && response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            if ui.is_rect_visible(rect) {
                paint(ui, rect, self.effective_radius(rect.height()), &st);

                // Label, centered, with the variant's foreground + state opacity.
                let text_color = label_color(self.variant).gamma_multiply(st.label_opacity);
                let text_pos = rect.center() - galley.size() * 0.5;
                let galley = ui.painter().layout_no_wrap(self.text, self.font, text_color);
                ui.painter().galley(text_pos, galley, text_color);
            }

            response
        }
    }

    /// Foreground (label) color per variant. Prominent/Gold ride on bright fills so
    /// use near-black ink; the translucent variants use platinum text; destructive is
    /// a bright red.
    fn label_color(variant: Variant) -> Color32 {
        match variant {
            Variant::Prominent => Color32::from_rgb(20, 21, 25),
            Variant::Gold => Color32::from_rgb(28, 20, 0),
            Variant::Destructive => Color32::from_rgb(255, 120, 112),
            _ => color::SILVER_LIGHT,
        }
    }

    /// Paints the button body (fill gradient, top gloss, border, drop shadow) into
    /// `rect` with the given corner `radius` (callers pass the capsule radius for
    /// [`Variant::Capsule`]). Public so views that need a bare button surface (e.g. an
    /// icon-only control) can reuse the exact look.
    pub fn paint(ui: &Ui, rect: Rect, radius: f32, st: &Style) {
        let painter = ui.painter();
        let rounding = Rounding::same(radius);

        // Drop shadow first (neutral black; a simple offset rect so there is zero glow).
        if st.shadow_alpha > 0.0 {
            let shadow_rect = rect.translate(Vec2::new(0.0, st.shadow_offset_y));
            painter.rect_filled(
                shadow_rect.expand(st.shadow_blur * 0.25),
                rounding,
                black_alpha(st.shadow_alpha),
            );
        }

        // Fill: vertical two-stop gradient (or flat when top == bottom).
        paint_vertical_fill(painter, rect, rounding, st.fill_top, st.fill_bottom);

        // Top specular gloss band: white peak → clear over GLOSS_HEIGHT.
        paint_top_gloss(painter, rect, radius, st.gloss_peak);

        // Crisp 1px rim.
        painter.rect_stroke(rect, rounding, st.border);
    }

    /// Fills `rect` with a vertical two-stop gradient (top → bottom). When the two
    /// stops are equal this is a flat fill. Approximated with 1px horizontal slices
    /// since egui has no native gradient fill.
    fn paint_vertical_fill(
        painter: &egui::Painter,
        rect: Rect,
        rounding: Rounding,
        top: Color32,
        bottom: Color32,
    ) {
        if top == bottom {
            painter.rect_filled(rect, rounding, top);
            return;
        }
        let g = gradient::LinearGradient {
            start: top,
            end: bottom,
            direction: gradient::Direction::TopToBottom,
        };
        let h = rect.height();
        let slices = h.ceil().max(1.0) as usize;
        for i in 0..slices {
            let y0 = rect.top() + i as f32;
            let y1 = (y0 + 1.0).min(rect.bottom());
            if y1 <= y0 {
                break;
            }
            let t = (i as f32 + 0.5) / h;
            let c = g.sample(t);
            let slice = Rect::from_min_max(
                egui::pos2(rect.left(), y0),
                egui::pos2(rect.right(), y1),
            );
            let slice_rounding = if i == 0 {
                Rounding {
                    nw: rounding.nw,
                    ne: rounding.ne,
                    sw: 0.0,
                    se: 0.0,
                }
            } else if y1 >= rect.bottom() {
                Rounding {
                    nw: 0.0,
                    ne: 0.0,
                    sw: rounding.sw,
                    se: rounding.se,
                }
            } else {
                Rounding::ZERO
            };
            painter.rect_filled(slice, slice_rounding, c);
        }
    }

    /// Paints the top specular gloss (white `peak` → clear over [`GLOSS_HEIGHT`]).
    fn paint_top_gloss(painter: &egui::Painter, rect: Rect, radius: f32, peak: f32) {
        if peak <= 0.0 {
            return;
        }
        let height = GLOSS_HEIGHT.min(rect.height());
        let slices = height.ceil().max(1.0) as usize;
        for i in 0..slices {
            let y0 = rect.top() + i as f32;
            let y1 = (y0 + 1.0).min(rect.top() + height);
            if y1 <= y0 {
                break;
            }
            let t = (i as f32 + 0.5) / height;
            let alpha = peak * (1.0 - t); // linear peak → 0
            if alpha <= 0.0 {
                continue;
            }
            let slice = Rect::from_min_max(
                egui::pos2(rect.left(), y0),
                egui::pos2(rect.right(), y1),
            );
            let slice_rounding = if i == 0 {
                Rounding {
                    nw: radius,
                    ne: radius,
                    sw: 0.0,
                    se: 0.0,
                }
            } else {
                Rounding::ZERO
            };
            painter.rect_filled(slice, slice_rounding, white_alpha(alpha));
        }
    }
}

/// The small interactive widgets: active/hover capsule pills, the Hi-Res "HR" gold
/// serif badge, liquid-glass icon badges, and per-drive storage gauges. Ported from
/// `LiquidGlassActivePill`, `LiquidGlassHoverPill`, `HiResBadge`,
/// `LiquidGlassIconBadge`, and the `storageGradient` usage in `Theme.swift`.
pub mod widgets {
    use super::{color, font, gradient, glass::white_alpha, FontId};
    use egui::{Color32, Rect, Response, Rounding, Sense, Stroke, Ui, Vec2};

    // ===================== Pills =====================

    /// Which segmented capsule pill to draw.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Pill {
        /// The *active* (selected) segment. `hi_res` swaps the rim to Hi-Res gold and
        /// softens the body fill, as in `LiquidGlassActivePill(isHiRes:)`.
        Active { hi_res: bool, hovered: bool },
        /// The *hover* affordance behind an unselected segment (`LiquidGlassHoverPill`).
        Hover,
    }

    /// Resolved pill paint recipe (fill, top sheen peak, rim stroke, shadow).
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct PillStyle {
        pub fill: Color32,
        pub sheen_peak: f32,
        pub sheen_height: f32,
        pub rim: Stroke,
        pub shadow_alpha: f32,
    }

    /// Resolve a [`Pill`]'s paint recipe.
    pub fn pill_style(pill: Pill) -> PillStyle {
        match pill {
            Pill::Active { hi_res, hovered } => PillStyle {
                fill: white_alpha(if hi_res { 0.13 } else { 0.16 }),
                sheen_peak: if hovered { 0.42 } else { 0.35 },
                sheen_height: 6.0,
                rim: if hi_res {
                    Stroke::new(0.85, gold_alpha(0.55))
                } else {
                    Stroke::new(0.85, white_alpha(if hovered { 0.45 } else { 0.34 }))
                },
                shadow_alpha: 0.25,
            },
            Pill::Hover => PillStyle {
                fill: white_alpha(0.09),
                sheen_peak: 0.20,
                sheen_height: 5.5,
                rim: Stroke::new(0.85, white_alpha(0.22)),
                shadow_alpha: 0.15,
            },
        }
    }

    /// Hi-Res gold at opacity `x` (`WalkmanTheme.hiResGold.opacity(x)`).
    fn gold_alpha(opacity: f32) -> Color32 {
        Color32::from_rgba_unmultiplied(
            color::HIRES_GOLD.r(),
            color::HIRES_GOLD.g(),
            color::HIRES_GOLD.b(),
            (opacity.clamp(0.0, 1.0) * 255.0).round() as u8,
        )
    }

    /// Paints a capsule [`Pill`] into `rect` (fill, top sheen, rim, soft shadow). The
    /// capsule corner radius is half the rect height.
    pub fn paint_pill(painter: &egui::Painter, rect: Rect, pill: Pill) {
        let st = pill_style(pill);
        let radius = rect.height() * 0.5;
        let rounding = Rounding::same(radius);

        // Soft neutral shadow.
        if st.shadow_alpha > 0.0 {
            painter.rect_filled(
                rect.translate(Vec2::new(0.0, 1.0)),
                rounding,
                Color32::from_black_alpha((st.shadow_alpha * 255.0).round() as u8),
            );
        }
        // Flat translucent body.
        painter.rect_filled(rect, rounding, st.fill);
        // Top specular sheen.
        paint_capsule_sheen(painter, rect, radius, st.sheen_peak, st.sheen_height);
        // Rim.
        painter.rect_stroke(rect, rounding, st.rim);
    }

    fn paint_capsule_sheen(
        painter: &egui::Painter,
        rect: Rect,
        radius: f32,
        peak: f32,
        height: f32,
    ) {
        if peak <= 0.0 {
            return;
        }
        let height = height.min(rect.height());
        let slices = height.ceil().max(1.0) as usize;
        for i in 0..slices {
            let y0 = rect.top() + i as f32;
            let y1 = (y0 + 1.0).min(rect.top() + height);
            if y1 <= y0 {
                break;
            }
            let t = (i as f32 + 0.5) / height;
            let alpha = peak * (1.0 - t);
            if alpha <= 0.0 {
                continue;
            }
            let slice = Rect::from_min_max(
                egui::pos2(rect.left(), y0),
                egui::pos2(rect.right(), y1),
            );
            let slice_rounding = if i == 0 {
                Rounding {
                    nw: radius,
                    ne: radius,
                    sw: 0.0,
                    se: 0.0,
                }
            } else {
                Rounding::ZERO
            };
            painter.rect_filled(slice, slice_rounding, white_alpha(alpha));
        }
    }

    // ===================== Hi-Res "HR" badge =====================

    /// Corner radius of the HR badge (`RoundedRectangle(cornerRadius: 3)`).
    pub const HR_BADGE_RADIUS: f32 = 3.0;
    /// Font size of the "HR" glyphs (`.system(size: 8.5, weight: .black, design: .serif)`).
    pub const HR_BADGE_FONT_SIZE: f32 = 8.5;

    /// The Hi-Res "HR" gold serif badge (`HiResBadge`). Draws "HR" in near-black ink
    /// over the Hi-Res gold gradient, in a 3pt-rounded chip. Allocates its own size
    /// (label + 4pt horizontal / 1.5pt vertical padding) and returns the [`Response`].
    pub fn hi_res_badge(ui: &mut Ui) -> Response {
        // Serif isn't a bundled face; the mono-bold face is the closest tactile match
        // to the compact black serif at this tiny size.
        let f = FontId::new(HR_BADGE_FONT_SIZE, font::badge().family);
        let galley = ui
            .painter()
            .layout_no_wrap("HR".to_owned(), f.clone(), Color32::BLACK);
        let pad = Vec2::new(4.0, 1.5);
        let desired = galley.size() + pad * 2.0;
        let (rect, response) = ui.allocate_at_least(desired, Sense::hover());

        if ui.is_rect_visible(rect) {
            paint_hi_res_badge(ui, rect, &f);
        }
        response
    }

    /// Paints the HR badge into an arbitrary `rect` (gold gradient fill + centered
    /// near-black "HR"). Lets table/grid rows place the badge inline.
    pub fn paint_hi_res_badge(ui: &Ui, rect: Rect, f: &FontId) {
        let painter = ui.painter();
        // Gold gradient, top-leading→bottom-trailing — approximated vertically here
        // (the chip is only a few px tall so the diagonal reads as a vertical ramp).
        let g = gradient::HIRES_GOLD;
        let h = rect.height().ceil().max(1.0) as usize;
        for i in 0..h {
            let y0 = rect.top() + i as f32;
            let y1 = (y0 + 1.0).min(rect.bottom());
            if y1 <= y0 {
                break;
            }
            let t = (i as f32 + 0.5) / rect.height();
            let slice = Rect::from_min_max(
                egui::pos2(rect.left(), y0),
                egui::pos2(rect.right(), y1),
            );
            let r = if i == 0 {
                Rounding { nw: HR_BADGE_RADIUS, ne: HR_BADGE_RADIUS, sw: 0.0, se: 0.0 }
            } else if y1 >= rect.bottom() {
                Rounding { nw: 0.0, ne: 0.0, sw: HR_BADGE_RADIUS, se: HR_BADGE_RADIUS }
            } else {
                Rounding::ZERO
            };
            painter.rect_filled(slice, r, g.sample(t));
        }
        let ink = Color32::from_rgb(10, 8, 0);
        let galley = painter.layout_no_wrap("HR".to_owned(), f.clone(), ink);
        let pos = rect.center() - galley.size() * 0.5;
        painter.galley(pos, galley, ink);
    }

    // ===================== Liquid-glass icon badge =====================

    /// A liquid-glass icon badge (`LiquidGlassIconBadge`): a rounded translucent chip
    /// with a top specular sheen, a faint rim, and a soft shadow, holding a centered
    /// glyph in platinum silver. Defaults: 32pt square, 9pt corner radius.
    #[derive(Debug, Clone)]
    pub struct IconBadge {
        glyph: String,
        size: f32,
        corner_radius: f32,
    }

    impl IconBadge {
        /// Default 32pt badge with a 9pt corner radius.
        pub fn new(glyph: impl Into<String>) -> Self {
            Self { glyph: glyph.into(), size: 32.0, corner_radius: 9.0 }
        }
        /// Override the square size.
        pub fn size(mut self, size: f32) -> Self {
            self.size = size;
            self
        }
        /// Override the corner radius.
        pub fn corner_radius(mut self, r: f32) -> Self {
            self.corner_radius = r;
            self
        }

        /// Allocates `size`×`size` and paints the badge; returns the [`Response`].
        pub fn show(self, ui: &mut Ui) -> Response {
            let (rect, response) =
                ui.allocate_at_least(Vec2::splat(self.size), Sense::hover());
            if ui.is_rect_visible(rect) {
                paint_icon_badge(ui, rect, self.corner_radius, &self.glyph, self.size * 0.42);
            }
            response
        }
    }

    /// Paints a liquid-glass icon badge into `rect`: white @8% body, a top sheen band
    /// (`height = size*0.32`, peak white 0.28), a faint rim, a soft neutral shadow,
    /// and the `glyph` centered in platinum silver at `glyph_size`.
    pub fn paint_icon_badge(ui: &Ui, rect: Rect, corner_radius: f32, glyph: &str, glyph_size: f32) {
        let painter = ui.painter();
        let rounding = Rounding::same(corner_radius);

        // Soft shadow (black @25%, radius ~3, y 1.5) — neutral, no glow.
        painter.rect_filled(
            rect.translate(Vec2::new(0.0, 1.5)),
            rounding,
            Color32::from_black_alpha((0.25_f32 * 255.0).round() as u8),
        );
        // Flat white @8% body.
        painter.rect_filled(rect, rounding, white_alpha(0.08));
        // Top sheen: white .28 → clear over size*0.32.
        let sheen_h = rect.height() * 0.32;
        let slices = sheen_h.ceil().max(1.0) as usize;
        for i in 0..slices {
            let y0 = rect.top() + i as f32;
            let y1 = (y0 + 1.0).min(rect.top() + sheen_h);
            if y1 <= y0 {
                break;
            }
            let t = (i as f32 + 0.5) / sheen_h;
            let alpha = 0.28 * (1.0 - t);
            if alpha <= 0.0 {
                continue;
            }
            let slice = Rect::from_min_max(
                egui::pos2(rect.left(), y0),
                egui::pos2(rect.right(), y1),
            );
            let r = if i == 0 {
                Rounding { nw: corner_radius, ne: corner_radius, sw: 0.0, se: 0.0 }
            } else {
                Rounding::ZERO
            };
            painter.rect_filled(slice, r, white_alpha(alpha));
        }
        // Faint rim (white 0.38 top stop).
        painter.rect_stroke(rect, rounding, Stroke::new(0.85, white_alpha(0.38)));
        // Centered glyph in platinum silver.
        if !glyph.is_empty() {
            let f = FontId::new(glyph_size, font::body().family);
            let galley = painter.layout_no_wrap(glyph.to_owned(), f, color::SILVER_LIGHT);
            let pos = rect.center() - galley.size() * 0.5;
            painter.galley(pos, galley, color::SILVER_LIGHT);
        }
    }

    // ===================== Storage gauge =====================

    /// Default height of a storage gauge track (a slim capsule bar).
    pub const STORAGE_GAUGE_HEIGHT: f32 = 8.0;

    /// A horizontal storage-usage gauge: a flat translucent track with a gradient fill
    /// proportional to `fraction` used, matching the macOS per-drive storage bars that
    /// paint `WalkmanTheme.storageGradient` (white .60 → .85, left → right).
    #[derive(Debug, Clone, Copy)]
    pub struct StorageGauge {
        fraction: f32,
        height: f32,
    }

    impl StorageGauge {
        /// A gauge showing `fraction` (0..=1) of capacity used.
        pub fn new(fraction: f32) -> Self {
            Self { fraction: fraction.clamp(0.0, 1.0), height: STORAGE_GAUGE_HEIGHT }
        }
        /// From used/total bytes (0 total → empty gauge).
        pub fn from_bytes(used: u64, total: u64) -> Self {
            let f = if total == 0 {
                0.0
            } else {
                (used as f64 / total as f64) as f32
            };
            Self::new(f)
        }
        /// Override the bar height.
        pub fn height(mut self, h: f32) -> Self {
            self.height = h;
            self
        }
        /// The clamped used-fraction this gauge renders.
        pub fn fraction(&self) -> f32 {
            self.fraction
        }

        /// Allocates a full-width bar `height` tall and paints it; returns the [`Response`].
        pub fn show(self, ui: &mut Ui) -> Response {
            let width = ui.available_width().max(1.0);
            let (rect, response) =
                ui.allocate_at_least(Vec2::new(width, self.height), Sense::hover());
            if ui.is_rect_visible(rect) {
                paint_storage_gauge(ui.painter(), rect, self.fraction);
            }
            response
        }
    }

    /// Paints a storage gauge into `rect`: a flat white @10% track capsule with a
    /// [`gradient::STORAGE`] fill spanning `fraction` of the width (left → right).
    pub fn paint_storage_gauge(painter: &egui::Painter, rect: Rect, fraction: f32) {
        let fraction = fraction.clamp(0.0, 1.0);
        let radius = rect.height() * 0.5;
        let rounding = Rounding::same(radius);

        // Flat translucent track.
        painter.rect_filled(rect, rounding, white_alpha(0.10));

        if fraction <= 0.0 || rect.width() <= 0.0 {
            return;
        }
        // Gradient fill spanning `fraction` of the full width, sampled over the full
        // track so colors stay fixed to position (left=.60 → right=.85) as the fill grows.
        let g = gradient::STORAGE;
        let fill_w = (rect.width() * fraction).max(radius.min(rect.width()));
        let slices = fill_w.ceil().max(1.0) as usize;
        for i in 0..slices {
            let x0 = rect.left() + i as f32;
            let x1 = (x0 + 1.0).min(rect.left() + fill_w);
            if x1 <= x0 {
                break;
            }
            let t = (i as f32 + 0.5) / rect.width();
            let slice = Rect::from_min_max(
                egui::pos2(x0, rect.top()),
                egui::pos2(x1, rect.bottom()),
            );
            let r = if i == 0 {
                Rounding { nw: radius, ne: 0.0, sw: radius, se: 0.0 }
            } else if x1 >= rect.left() + fill_w {
                Rounding { nw: 0.0, ne: radius, sw: 0.0, se: radius }
            } else {
                Rounding::ZERO
            };
            painter.rect_filled(slice, r, g.sample(t));
        }
    }
}

/// The bundled icon set and the fixed SF-Symbol→glyph mapping (requirements 1.5, 2.7).
///
/// SF Symbols are a macOS-only system resource, so the port ships its own icon set and a
/// **fixed** mapping from every SF Symbol used in the macOS sidebar/toolbar to a
/// visually-equivalent glyph. This module is the single source of truth for that mapping;
/// every icon-bearing view resolves its glyph through [`sidebar_icon`] / [`toolbar_icon`]
/// keyed by the exact SF Symbol name the macOS app uses (see `SidebarView.swift` and the
/// `ContentView` toolbar). The [`NavTab`](crate) enum is built later (task 4); until then
/// the table is keyed by SF Symbol name so it is usable by any view.
///
/// ## Chosen icon set: Lucide
///
/// The port uses [Lucide](https://lucide.dev) — an actively-maintained, ISC-licensed fork
/// of Feather with a broad, consistent outline set that is the closest community match to
/// SF Symbols' visual weight at sidebar/toolbar sizes. Each [`Icon`] carries its Lucide
/// *glyph name* (e.g. `"list-music"`, `"copy"`), matching the examples in the design doc
/// (`music.note.list`→list-music, `doc.on.doc.fill`→copy, `hifispeaker.fill`→speaker).
///
/// ## Asset slot & graceful fallback
///
/// The actual Lucide glyph assets (an icon font or per-glyph SVGs) are not yet vendored.
/// Consistent with how [`install_fonts`] handles bundled fonts, the real assets live in a
/// documented, git-ignored slot under `windows-app/assets/icons/` that the packaging step
/// (task 19.1) fills in. Until then [`Icon::glyph_name`] returns the Lucide name a renderer
/// looks up, and [`Icon::fallback_text`] gives a short text label (the mono-faced badge
/// style used elsewhere) so an icon-bearing row still renders a legible placeholder when
/// no glyph asset is present. No glyph name is ever empty.
pub mod icon {
    /// A bundled (Lucide) glyph referenced by the sidebar/toolbar. Each variant maps to a
    /// single Lucide glyph; see [`Icon::glyph_name`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum Icon {
        /// `speaker` — a Walkman / hi-fi speaker device (SF `hifispeaker.fill`).
        Speaker,
        /// `folder-cog` — the local library folder with a settings gear (SF `folder.badge.gearshape`).
        FolderCog,
        /// `activity` — the SensMe waveform/ECG channel (SF `waveform.path.ecg.rectangle.fill`).
        Activity,
        /// `list-music` — a song list / All Songs (SF `music.note.list`).
        ListMusic,
        /// `history` — recently added (clock with a circular arrow) (SF `clock.arrow.circlepath`).
        History,
        /// `layers` — stacked albums (SF `square.stack.fill`).
        Layers,
        /// `mic` — artists (SF `music.mic`).
        Mic,
        /// `search` — Hi-Res audio inspector (waveform + magnifier) (SF `waveform.badge.magnifyingglass`).
        Search,
        /// `book` — a closed book / language study (SF `character.book.closed.fill`).
        Book,
        /// `list` — a playlist (SF `list.bullet.rectangle.portrait.fill`).
        List,
        /// `copy` — duplicate documents (SF `doc.on.doc.fill`).
        Copy,
        /// `arrow-up-down` — transfer & convert (SF `arrow.up.arrow.down.circle.fill`).
        ArrowUpDown,
        /// `refresh-cw` — backup & restore (two circular arrows) (SF `arrow.triangle.2.circlepath.circle.fill`).
        RefreshCw,
        /// `info` — the About button (SF `info.circle.fill`).
        Info,
        /// `hard-drive` — an internal drive (SF `internaldrive.fill`).
        HardDrive,
        /// `sd-card` — a microSD drive (SF `sdcard.fill`).
        SdCard,
        /// `audio-waveform` — the Hi-Res gold waveform badge on a sidebar row (SF `waveform`).
        Waveform,
        /// `cable` — the disconnected-device row (SF `cable.connector.horizontal`).
        Cable,
        /// `rotate-cw` — the toolbar Refresh action (SF `arrow.clockwise`).
        RotateCw,
        /// `eject` — the toolbar Eject action (SF `eject.circle`).
        Eject,
        /// `panel-right` — the toolbar Inspector toggle (SF `sidebar.trailing`).
        PanelRight,
    }

    impl Icon {
        /// The Lucide glyph name a renderer looks up in the bundled icon set. Never empty.
        pub fn glyph_name(self) -> &'static str {
            match self {
                Icon::Speaker => "speaker",
                Icon::FolderCog => "folder-cog",
                Icon::Activity => "activity",
                Icon::ListMusic => "list-music",
                Icon::History => "history",
                Icon::Layers => "layers",
                Icon::Mic => "mic",
                Icon::Search => "search",
                Icon::Book => "book",
                Icon::List => "list",
                Icon::Copy => "copy",
                Icon::ArrowUpDown => "arrow-up-down",
                Icon::RefreshCw => "refresh-cw",
                Icon::Info => "info",
                Icon::HardDrive => "hard-drive",
                Icon::SdCard => "sd-card",
                Icon::Waveform => "audio-waveform",
                Icon::Cable => "cable",
                Icon::RotateCw => "rotate-cw",
                Icon::Eject => "eject",
                Icon::PanelRight => "panel-right",
            }
        }

        /// A short text label used as a graceful fallback when the glyph asset is missing
        /// (rendered in the mono badge style). Kept to 1–2 compact ASCII chars so it fits
        /// an icon slot. Never empty.
        pub fn fallback_text(self) -> &'static str {
            match self {
                Icon::Speaker => "SP",
                Icon::FolderCog => "LB",
                Icon::Activity => "SM",
                Icon::ListMusic => "AS",
                Icon::History => "RA",
                Icon::Layers => "AL",
                Icon::Mic => "AR",
                Icon::Search => "HR",
                Icon::Book => "LS",
                Icon::List => "PL",
                Icon::Copy => "DU",
                Icon::ArrowUpDown => "TR",
                Icon::RefreshCw => "BR",
                Icon::Info => "i",
                Icon::HardDrive => "HD",
                Icon::SdCard => "SD",
                Icon::Waveform => "HR",
                Icon::Cable => "NC",
                Icon::RotateCw => "RF",
                Icon::Eject => "EJ",
                Icon::PanelRight => "IN",
            }
        }
    }

    /// The fixed SF-Symbol → [`Icon`] mapping for every symbol the macOS **sidebar** uses:
    /// the 13 `NavTab` section icons plus the device-card / drive-row / disconnected-row
    /// and About-button symbols. Keyed by the exact SF Symbol name so it is stable across
    /// the port (the `NavTab` enum that will also key these arrives in task 4).
    ///
    /// Source: `NavigationTab.icon`, `SidebarView` rows, and the About button in
    /// `SidebarView.swift`.
    pub const SIDEBAR_ICON: &[(&str, Icon)] = &[
        // --- 13 NavTab section icons (R1.4) ---
        ("hifispeaker.fill", Icon::Speaker),
        ("folder.badge.gearshape", Icon::FolderCog),
        ("waveform.path.ecg.rectangle.fill", Icon::Activity),
        ("music.note.list", Icon::ListMusic),
        ("clock.arrow.circlepath", Icon::History),
        ("square.stack.fill", Icon::Layers),
        ("music.mic", Icon::Mic),
        ("waveform.badge.magnifyingglass", Icon::Search),
        ("character.book.closed.fill", Icon::Book),
        ("list.bullet.rectangle.portrait.fill", Icon::List),
        ("doc.on.doc.fill", Icon::Copy),
        ("arrow.up.arrow.down.circle.fill", Icon::ArrowUpDown),
        ("arrow.triangle.2.circlepath.circle.fill", Icon::RefreshCw),
        // --- Other sidebar chrome ---
        ("info.circle.fill", Icon::Info),
        ("internaldrive.fill", Icon::HardDrive),
        ("sdcard.fill", Icon::SdCard),
        ("waveform", Icon::Waveform),
        ("cable.connector.horizontal", Icon::Cable),
    ];

    /// The fixed SF-Symbol → [`Icon`] mapping for the macOS **toolbar** actions:
    /// Refresh, Eject, and the Inspector toggle. Source: the `ContentView` toolbar.
    pub const TOOLBAR_ICON: &[(&str, Icon)] = &[
        ("arrow.clockwise", Icon::RotateCw),
        ("eject.circle", Icon::Eject),
        ("sidebar.trailing", Icon::PanelRight),
    ];

    /// Resolve a **sidebar** SF Symbol name to its bundled [`Icon`], or `None` if the name
    /// is not one the sidebar uses (which, per the exhaustiveness test, should not happen
    /// for any symbol the app actually renders).
    pub fn sidebar_icon(sf_symbol: &str) -> Option<Icon> {
        lookup(SIDEBAR_ICON, sf_symbol)
    }

    /// Resolve a **toolbar** SF Symbol name to its bundled [`Icon`].
    pub fn toolbar_icon(sf_symbol: &str) -> Option<Icon> {
        lookup(TOOLBAR_ICON, sf_symbol)
    }

    fn lookup(table: &[(&str, Icon)], sf_symbol: &str) -> Option<Icon> {
        table
            .iter()
            .find_map(|&(name, icon)| (name == sf_symbol).then_some(icon))
    }
}

// Bundled icon-asset slot. The Lucide glyph assets (an icon font or per-glyph SVGs) are
// vendored into `windows-app/assets/icons/` by the packaging step (task 19.1); see
// `assets/README.md`. Until then the slot resolves to `None` and renderers fall back to
// `Icon::fallback_text`, mirroring how the bundled-font slots drive the font fallback.
#[cfg(walkman_bundled_icons)]
const OPTIONAL_ICON_FONT: Option<&[u8]> =
    Some(include_bytes!("../../../assets/icons/lucide.ttf"));
#[cfg(not(walkman_bundled_icons))]
const OPTIONAL_ICON_FONT: Option<&[u8]> = None;

/// Whether the bundled Lucide icon font is present this build. When `false`, icon-bearing
/// views fall back to [`icon::Icon::fallback_text`] (see the module docs on [`icon`]).
pub fn bundled_icon_font() -> Option<&'static [u8]> {
    OPTIONAL_ICON_FONT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obsidian_is_101114() {
        assert_eq!(color::OBSIDIAN, Color32::from_rgb(16, 17, 20));
    }

    #[test]
    fn window_min_matches_macos() {
        assert_eq!(sizing::WINDOW_MIN, (1320.0, 720.0));
        assert_eq!(sizing::SIDEBAR_WIDTH, 260.0);
    }

    /// The 38pt control-height baseline (R2.7) and 88pt table bottom inset (R2.9).
    #[test]
    fn layout_constants_match_design() {
        assert_eq!(sizing::CONTROL_BASELINE, 38.0);
        assert_eq!(sizing::TABLE_BOTTOM_INSET, 88.0);
    }

    // --- Gradients (requirement 2.2) ---

    /// `Color(white: level)` → opaque grey with each channel `round(level*255)`.
    fn white(level: f32) -> Color32 {
        let v = (level * 255.0 + 0.5) as u8;
        Color32::from_rgb(v, v, v)
    }

    #[test]
    fn silver_gradient_is_white_90_to_78_diagonal() {
        let g = gradient::SILVER;
        assert_eq!(g.start, white(0.90));
        assert_eq!(g.end, white(0.78));
        assert_eq!(g.direction, gradient::Direction::TopLeadingToBottomTrailing);
    }

    #[test]
    fn storage_gradient_is_white_60_to_85_left_to_right() {
        let g = gradient::STORAGE;
        assert_eq!(g.start, white(0.60));
        assert_eq!(g.end, white(0.85));
        assert_eq!(g.direction, gradient::Direction::LeadingToTrailing);
    }

    #[test]
    fn card_header_gradient_is_white_24_to_16_diagonal() {
        let g = gradient::CARD_HEADER;
        assert_eq!(g.start, white(0.24));
        assert_eq!(g.end, white(0.16));
        assert_eq!(g.direction, gradient::Direction::TopLeadingToBottomTrailing);
    }

    #[test]
    fn hires_gold_gradient_light_to_dark_diagonal() {
        let g = gradient::HIRES_GOLD;
        assert_eq!(g.start, color::HIRES_GOLD_LIGHT);
        assert_eq!(g.end, color::HIRES_GOLD_DARK);
        assert_eq!(g.direction, gradient::Direction::TopLeadingToBottomTrailing);
    }

    #[test]
    fn gradient_sample_hits_endpoints_and_midpoint() {
        let g = gradient::STORAGE;
        assert_eq!(g.sample(0.0), g.start);
        assert_eq!(g.sample(1.0), g.end);
        // Midpoint of white .60 (153) and white .85 (217) → ~185.
        let mid = g.sample(0.5);
        assert_eq!(mid, Color32::from_rgb(185, 185, 185));
    }

    #[test]
    fn gradient_sample_clamps_out_of_range() {
        let g = gradient::SILVER;
        assert_eq!(g.sample(-1.0), g.start);
        assert_eq!(g.sample(2.0), g.end);
    }

    // --- Typography (requirement 2.3) ---

    #[test]
    fn typography_sizes_match_walkman_font_scale() {
        assert_eq!(font::title1().size, 22.0);
        assert_eq!(font::title2().size, 18.0);
        assert_eq!(font::title().size, 18.0); // alias of title2
        assert_eq!(font::title3().size, 15.0);
        assert_eq!(font::headline().size, 14.0);
        assert_eq!(font::body().size, 13.0);
        assert_eq!(font::body_medium().size, 13.0);
        assert_eq!(font::body_bold().size, 13.0);
        assert_eq!(font::callout().size, 12.0);
        assert_eq!(font::callout_medium().size, 12.0);
        assert_eq!(font::callout_bold().size, 12.0);
        assert_eq!(font::subheadline().size, 11.5);
        assert_eq!(font::caption().size, 11.0);
        assert_eq!(font::section_header().size, 11.0);
        assert_eq!(font::mono().size, 11.5);
        assert_eq!(font::mono_small().size, 10.5);
        assert_eq!(font::badge().size, 9.5);
        assert_eq!(font::pill_counter().size, 10.0);
    }

    #[test]
    fn typography_families_match_weight_and_design() {
        use egui::FontFamily;
        let name = |f: FontId| match f.family {
            FontFamily::Name(n) => n.to_string(),
            other => format!("{other:?}"),
        };
        // Bold headings resolve to the bold sans family.
        assert_eq!(name(font::title1()), family::SANS_BOLD);
        assert_eq!(name(font::title2()), family::SANS_BOLD);
        assert_eq!(name(font::section_header()), family::SANS_BOLD);
        // Semibold.
        assert_eq!(name(font::title3()), family::SANS_SEMIBOLD);
        assert_eq!(name(font::headline()), family::SANS_SEMIBOLD);
        assert_eq!(name(font::body_bold()), family::SANS_SEMIBOLD);
        // Regular / medium.
        assert_eq!(name(font::body()), family::SANS);
        assert_eq!(name(font::body_medium()), family::SANS_MEDIUM);
        // Monospace & rounded design.
        assert_eq!(name(font::mono()), family::MONO);
        assert_eq!(name(font::mono_small()), family::MONO);
        assert_eq!(name(font::badge()), family::MONO_BOLD);
        assert_eq!(name(font::pill_counter()), family::ROUNDED_BOLD);
    }

    #[test]
    fn install_fonts_registers_named_families_with_builtin_fallback() {
        // On a host without bundled Segoe UI, install_fonts must still register every
        // named family (aliased to egui's built-in faces) so no text is ever empty.
        let ctx = egui::Context::default();
        install_fonts(&ctx);

        // egui only materializes the font set during a frame, so run one headless frame,
        // laying out one glyph per named family inside it. An unknown family or an empty
        // face would panic / produce a zero-width glyph.
        let input = egui::RawInput::default();
        let _ = ctx.run(input, |ctx| {
            ctx.fonts(|f| {
                for fam in [
                    family::SANS,
                    family::SANS_MEDIUM,
                    family::SANS_SEMIBOLD,
                    family::SANS_BOLD,
                    family::MONO,
                    family::MONO_BOLD,
                    family::ROUNDED_BOLD,
                ] {
                    let id = FontId::new(13.0, egui::FontFamily::Name(fam.into()));
                    let w = f.glyph_width(&id, 'M');
                    assert!(w > 0.0, "family {fam} produced no glyph");
                }
            });
        });
    }

    // --- Glass surfaces (requirements 2.4 & 2.6) ---

    mod glass_surfaces {
        use super::super::glass::{self, Sheen, State, StudioSurface};
        use super::super::{color, Color32};

        /// `Color.white.opacity(x)` reference the tests compare against.
        fn white(x: f32) -> Color32 {
            Color32::from_white_alpha((x * 255.0).round() as u8)
        }
        /// `Color.black.opacity(x)` reference (premultiplied black = plain alpha).
        fn black(x: f32) -> Color32 {
            Color32::from_black_alpha((x * 255.0).round() as u8)
        }

        // --- Corner radii (requirement 2.4) ---
        #[test]
        fn corner_radii_match_design() {
            assert_eq!(glass::LIQUID_RADIUS, 12.0);
            assert_eq!(glass::STUDIO_RADIUS, 14.0);
            assert_eq!(glass::MODAL_RADIUS, 16.0);

            assert_eq!(glass::liquid_glass(State::Normal).rounding, egui::Rounding::same(12.0));
            assert_eq!(glass::studio_glass_card().rounding, egui::Rounding::same(14.0));
            assert_eq!(glass::modal_glass_chassis().rounding, egui::Rounding::same(16.0));
        }

        // --- Flat fills: CARD at the documented opacity, no gradient (requirement 2.6) ---
        #[test]
        fn fills_are_flat_card_at_documented_opacity() {
            assert_eq!(glass::LIQUID_FILL, 0.78);
            assert_eq!(glass::STUDIO_FILL, 0.88);
            assert_eq!(glass::MODAL_FILL, 0.92);

            assert_eq!(
                glass::liquid_glass(State::Normal).fill,
                color::CARD.gamma_multiply(0.78)
            );
            assert_eq!(glass::studio_glass_card().fill, color::CARD.gamma_multiply(0.88));
            assert_eq!(glass::modal_glass_chassis().fill, color::CARD.gamma_multiply(0.92));
        }

        // --- liquid_glass border: 1px white rim whose opacity varies by state ---
        #[test]
        fn liquid_glass_border_opacity_varies_by_state() {
            let normal = glass::liquid_glass(State::Normal);
            let hovered = glass::liquid_glass(State::Hovered);
            let selected = glass::liquid_glass(State::Selected);

            // Resting & hover: 1px white rim, opacity = 0.35 * {0.20, 0.35}.
            assert_eq!(normal.stroke.width, 1.0);
            assert_eq!(normal.stroke.color, white(0.35 * 0.20));
            assert_eq!(hovered.stroke.width, 1.0);
            assert_eq!(hovered.stroke.color, white(0.35 * 0.35));

            // Hover rim is strictly brighter than the resting rim.
            assert!(hovered.stroke.color.a() > normal.stroke.color.a());

            // Selected: crisp silver rim at 1.5px (not plain white).
            assert_eq!(selected.stroke.width, 1.5);
            assert_eq!(
                selected.stroke.color,
                Color32::from_rgba_unmultiplied(
                    color::SILVER_LIGHT.r(),
                    color::SILVER_LIGHT.g(),
                    color::SILVER_LIGHT.b(),
                    (0.35_f32 * 0.95 * 255.0).round() as u8,
                )
            );
        }

        // --- liquid_glass shadow: neutral black, radius 6 → 10 on hover, no glow ---
        #[test]
        fn liquid_glass_shadow_is_neutral_black_and_grows_on_hover() {
            let normal = glass::liquid_glass(State::Normal);
            let hovered = glass::liquid_glass(State::Hovered);
            let selected = glass::liquid_glass(State::Selected);

            // Neutral black at 24% on every state (no colored glow).
            for f in [normal, hovered, selected] {
                assert_eq!(f.shadow.color, black(0.24));
                assert_eq!(f.shadow.spread, 0.0);
            }

            // Radius 6 at rest, 10 on hover; selected keeps the resting radius.
            assert_eq!(normal.shadow.blur, 6.0);
            assert_eq!(hovered.shadow.blur, 10.0);
            assert_eq!(selected.shadow.blur, 6.0);

            // Shadow drops straight down and deepens on hover (depth, not glow).
            assert_eq!(normal.shadow.offset, egui::Vec2::new(0.0, 2.0));
            assert_eq!(hovered.shadow.offset, egui::Vec2::new(0.0, 4.0));
        }

        // --- studio_glass_card rim + shadow ---
        #[test]
        fn studio_glass_card_rim_and_shadow() {
            let f = glass::studio_glass_card();
            assert_eq!(f.stroke.width, 0.85);
            assert_eq!(f.stroke.color, white(0.24));
            assert_eq!(f.shadow.blur, 8.0);
            assert_eq!(f.shadow.color, black(0.22));
            assert_eq!(f.shadow.offset, egui::Vec2::new(0.0, 3.0));
            assert_eq!(f.shadow.spread, 0.0);
        }

        // --- modal_glass_chassis rim + strong shadow ---
        #[test]
        fn modal_glass_chassis_rim_and_strong_shadow() {
            let f = glass::modal_glass_chassis();
            assert_eq!(f.stroke.width, 0.85);
            assert_eq!(f.stroke.color, white(0.35));
            // Strong drop shadow ~radius 28.
            assert_eq!(f.shadow.blur, 28.0);
            assert_eq!(f.shadow.color, black(0.45));
            assert_eq!(f.shadow.offset, egui::Vec2::new(0.0, 12.0));
            assert_eq!(f.shadow.spread, 0.0);

            // The chassis shadow is the strongest of the three surfaces.
            assert!(f.shadow.blur > glass::studio_glass_card().shadow.blur);
            assert!(f.shadow.blur > glass::liquid_glass(State::Hovered).shadow.blur);
        }

        // --- Sheens: the only gradient, top-edge specular, fading to clear ---
        #[test]
        fn sheen_heights_and_stops_match_design() {
            assert_eq!(Sheen::Studio.height(), 8.0);
            assert_eq!(Sheen::Modal.height(), 10.0);

            assert_eq!(Sheen::Studio.stops(), [(0.0, 0.16), (0.5, 0.03), (1.0, 0.0)]);
            assert_eq!(Sheen::Modal.stops(), [(0.0, 0.24), (0.5, 0.05), (1.0, 0.0)]);

            // Every sheen fades to fully clear at the bottom (fades into the flat fill).
            for sheen in [Sheen::Studio, Sheen::Modal] {
                let last = sheen.stops()[2];
                assert_eq!(last, (1.0, 0.0));
                // Sheen is brightest at the very top edge.
                assert!(sheen.stops()[0].1 > sheen.stops()[1].1);
            }
        }

        // --- Zero glow / zero background gradient invariant (requirement 2.6) ---
        #[test]
        fn surfaces_have_no_glow_and_flat_fills() {
            // A glow would be a non-black, non-transparent shadow or a positive spread.
            // Assert every surface shadow is pure neutral black with zero spread.
            let frames = [
                glass::liquid_glass(State::Normal),
                glass::liquid_glass(State::Hovered),
                glass::liquid_glass(State::Selected),
                glass::studio_glass_card(),
                glass::modal_glass_chassis(),
            ];
            for f in frames {
                assert_eq!(f.shadow.spread, 0.0, "no spread/bloom");
                assert_eq!(f.shadow.color.r(), 0);
                assert_eq!(f.shadow.color.g(), 0);
                assert_eq!(f.shadow.color.b(), 0, "shadow must be neutral black, not a colored glow");
                // Fill is a flat tint of CARD (a single color, never a gradient): it
                // preserves CARD's near-neutral hue (r ≤ g ≤ b, as in #18191D) rather
                // than carrying any colored cast.
                assert!(f.fill.r() <= f.fill.g() && f.fill.g() <= f.fill.b());
            }
        }

        #[test]
        fn white_alpha_matches_swiftui_opacity() {
            // "white @ 8%" per the design tokens.
            assert_eq!(glass::white_alpha(0.08), white(0.08));
            assert_eq!(glass::white_alpha(0.0), Color32::from_white_alpha(0));
            // Clamps out of range.
            assert_eq!(glass::white_alpha(2.0), Color32::from_white_alpha(255));
        }

        // --- StudioSurface bundles frame + sheen correctly ---
        #[test]
        fn studio_surface_bundles_frame_and_sheen() {
            let s = StudioSurface::studio();
            assert_eq!(s.sheen, Sheen::Studio);
            assert_eq!(s.rounding, 14.0);
            assert_eq!(s.frame.fill, color::CARD.gamma_multiply(0.88));

            let m = StudioSurface::modal();
            assert_eq!(m.sheen, Sheen::Modal);
            assert_eq!(m.rounding, 16.0);
            assert_eq!(m.frame.fill, color::CARD.gamma_multiply(0.92));
        }

        // --- Surfaces render inside a headless frame without panicking ---
        #[test]
        fn surfaces_render_in_headless_frame() {
            let ctx = egui::Context::default();
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    glass::liquid_glass(State::Normal).show(ui, |ui| {
                        ui.label("liquid");
                    });
                    glass::liquid_glass(State::Hovered).show(ui, |ui| {
                        ui.label("liquid hover");
                    });
                    StudioSurface::studio().show(ui, |ui| {
                        ui.label("studio");
                    });
                    StudioSurface::modal().show(ui, |ui| {
                        ui.label("modal");
                    });
                });
            });
        }
    }

    // --- Button variants (requirement 2.5) ---

    mod buttons {
        use super::super::button::{self, State, Style, Variant, HOVER_SCALE, PRESS_SCALE};
        use super::super::glass::white_alpha;
        use super::super::gradient;
        use egui::Color32;

        const ALL_VARIANTS: [Variant; 6] = [
            Variant::Prominent,
            Variant::Gold,
            Variant::Glass,
            Variant::Capsule,
            Variant::Subtle,
            Variant::Destructive,
        ];

        /// All six variants exist and resolve a distinct style across states.
        #[test]
        fn six_variants_exist() {
            assert_eq!(ALL_VARIANTS.len(), 6);
            for v in ALL_VARIANTS {
                let _ = button::style(v, State::Normal);
            }
        }

        /// Hover scales to ~1.025, press to ~0.97, rest/disabled to 1.0 (requirement 2.5).
        #[test]
        fn hover_and_press_scale_feedback() {
            for v in ALL_VARIANTS {
                assert_eq!(button::style(v, State::Normal).scale, 1.0);
                assert_eq!(button::style(v, State::Hovered).scale, HOVER_SCALE);
                assert_eq!(button::style(v, State::Pressed).scale, PRESS_SCALE);
                // Disabled never scales.
                assert_eq!(button::style(v, State::Disabled).scale, 1.0);
            }
            assert_eq!(HOVER_SCALE, 1.025);
            assert_eq!(PRESS_SCALE, 0.97);
        }

        /// Pressing dims the label to 0.85, disabling to 0.40, rest is fully opaque.
        #[test]
        fn label_opacity_tracks_state() {
            let v = Variant::Prominent;
            assert_eq!(button::style(v, State::Normal).label_opacity, 1.0);
            assert_eq!(button::style(v, State::Hovered).label_opacity, 1.0);
            assert_eq!(button::style(v, State::Pressed).label_opacity, 0.85);
            assert_eq!(button::style(v, State::Disabled).label_opacity, 0.40);
        }

        /// Prominent = silver/platinum vertical gradient (#F5F7FC → #D7DCE6).
        #[test]
        fn prominent_is_silver_platinum_gradient() {
            let s = button::style(Variant::Prominent, State::Normal);
            assert_eq!(s.fill_top, Color32::from_rgb(245, 247, 252));
            assert_eq!(s.fill_bottom, Color32::from_rgb(215, 220, 230));
            // A real (non-flat) gradient.
            assert_ne!(s.fill_top, s.fill_bottom);
        }

        /// Gold = the Hi-Res gold gradient endpoints.
        #[test]
        fn gold_uses_hires_gold_gradient() {
            let s = button::style(Variant::Gold, State::Normal);
            assert_eq!(s.fill_top, gradient::HIRES_GOLD.start);
            assert_eq!(s.fill_bottom, gradient::HIRES_GOLD.end);
        }

        /// Glass & Capsule share the same translucent white fill that brightens on hover.
        #[test]
        fn glass_and_capsule_share_translucent_fill_and_brighten_on_hover() {
            for v in [Variant::Glass, Variant::Capsule] {
                let rest = button::style(v, State::Normal);
                let hover = button::style(v, State::Hovered);
                // Flat fill (top == bottom), translucent white.
                assert_eq!(rest.fill_top, rest.fill_bottom);
                assert_eq!(rest.fill_top, white_alpha(0.08));
                assert_eq!(hover.fill_top, white_alpha(0.14));
                assert!(hover.fill_top.a() > rest.fill_top.a());
            }
            // The two variants are fill-identical.
            assert_eq!(
                button::style(Variant::Glass, State::Normal).fill_top,
                button::style(Variant::Capsule, State::Normal).fill_top
            );
        }

        /// Subtle is the lowest-contrast frosted fill.
        #[test]
        fn subtle_is_lowest_contrast() {
            let subtle = button::style(Variant::Subtle, State::Normal).fill_top;
            let glass = button::style(Variant::Glass, State::Normal).fill_top;
            assert_eq!(subtle, white_alpha(0.05));
            assert!(subtle.a() < glass.a());
        }

        /// Destructive is red-tinted (border + fill carry red, not white). Colors are
        /// stored premultiplied, so compare against the exact system-red constructions
        /// rather than raw post-multiply channel values.
        #[test]
        fn destructive_is_red_tinted() {
            // System red (#FF3B30) at the documented opacities.
            let red = |o: f32| Color32::from_rgba_unmultiplied(255, 59, 48, (o * 255.0).round() as u8);
            let s = button::style(Variant::Destructive, State::Normal);
            // Fill is red at 12%; a destructive fill reads red-dominant (r > g, r > b).
            assert_eq!(s.fill_top, red(0.12));
            assert!(s.fill_top.r() > s.fill_top.g() && s.fill_top.r() > s.fill_top.b());
            // Border is red at 30% (0.55 on hover) and brighter on hover.
            let hov = button::style(Variant::Destructive, State::Hovered);
            assert_eq!(s.border.color, red(0.30));
            assert_eq!(hov.border.color, red(0.55));
            assert!(hov.border.color.a() > s.border.color.a());
        }

        /// Shadows are neutral black and deepen on hover; never a colored glow.
        #[test]
        fn shadows_are_neutral_black_and_deepen_on_hover() {
            for v in ALL_VARIANTS {
                let rest = button::style(v, State::Normal);
                let hover = button::style(v, State::Hovered);
                assert_eq!(rest.shadow_blur, 3.0);
                assert_eq!(hover.shadow_blur, 6.0);
                assert!(hover.shadow_alpha > rest.shadow_alpha);
                // Shadow alpha is applied to pure black in `paint` (no color stored here);
                // assert the resting/hover alphas match the source.
                assert_eq!(rest.shadow_alpha, 0.14);
                assert_eq!(hover.shadow_alpha, 0.25);
            }
        }

        /// The prominent gloss peak is the strongest; destructive the weakest.
        #[test]
        fn gloss_peak_ordering_matches_source() {
            let prom = button::style(Variant::Prominent, State::Normal).gloss_peak;
            let glass = button::style(Variant::Glass, State::Normal).gloss_peak;
            let dest = button::style(Variant::Destructive, State::Normal).gloss_peak;
            assert_eq!(prom, 0.65);
            assert_eq!(glass, 0.35);
            assert_eq!(dest, 0.28);
            assert!(prom > glass && glass > dest);
            // Disabled dims the gloss to 30%.
            assert!((button::style(Variant::Prominent, State::Disabled).gloss_peak - 0.65 * 0.30).abs() < 1e-6);
        }

        /// A disabled button never reports a hover/press scale even if those states are asked for.
        #[test]
        fn disabled_buttons_render_in_headless_frame() {
            let _: Style = button::style(Variant::Prominent, State::Disabled);
            let ctx = egui::Context::default();
            super::super::install_fonts(&ctx);
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    for v in ALL_VARIANTS {
                        let _ = ui.add(button::Button::new(format!("{v:?}"), v));
                        let _ = ui.add(button::Button::new("disabled", v).enabled(false));
                    }
                });
            });
        }
    }

    // --- Pills, badges, gauges (requirement 2.8) ---

    mod small_widgets {
        use super::super::glass::white_alpha;
        use super::super::widgets::{self, Pill};
        use super::super::color;

        /// The active pill is a brighter capsule than the hover pill (selected reads stronger).
        #[test]
        fn active_pill_is_brighter_than_hover_pill() {
            let active = widgets::pill_style(Pill::Active { hi_res: false, hovered: false });
            let hover = widgets::pill_style(Pill::Hover);
            assert_eq!(active.fill, white_alpha(0.16));
            assert_eq!(hover.fill, white_alpha(0.09));
            assert!(active.fill.a() > hover.fill.a());
            assert!(active.rim.color.a() > hover.rim.color.a());
        }

        /// A Hi-Res active pill swaps the rim to gold and softens the body fill.
        #[test]
        fn hi_res_active_pill_uses_gold_rim() {
            let plain = widgets::pill_style(Pill::Active { hi_res: false, hovered: false });
            let hires = widgets::pill_style(Pill::Active { hi_res: true, hovered: false });
            // Softer body fill when Hi-Res.
            assert_eq!(hires.fill, white_alpha(0.13));
            assert!(hires.fill.a() < plain.fill.a());
            // Rim is Hi-Res gold at 55% (stored premultiplied, so compare the exact
            // construction rather than raw channel values).
            let gold = egui::Color32::from_rgba_unmultiplied(
                color::HIRES_GOLD.r(),
                color::HIRES_GOLD.g(),
                color::HIRES_GOLD.b(),
                (0.55_f32 * 255.0).round() as u8,
            );
            assert_eq!(hires.rim.color, gold);
            // The plain (non-Hi-Res) rim is white, not gold.
            assert_ne!(plain.rim.color, gold);
        }

        /// Hovering an active pill brightens the sheen peak and the (non-gold) rim.
        #[test]
        fn active_pill_brightens_on_hover() {
            let rest = widgets::pill_style(Pill::Active { hi_res: false, hovered: false });
            let hov = widgets::pill_style(Pill::Active { hi_res: false, hovered: true });
            assert!(hov.sheen_peak > rest.sheen_peak);
            assert!(hov.rim.color.a() > rest.rim.color.a());
        }

        /// HR badge metrics match the source (3pt radius, 8.5pt glyphs).
        #[test]
        fn hr_badge_metrics_match_source() {
            assert_eq!(widgets::HR_BADGE_RADIUS, 3.0);
            assert_eq!(widgets::HR_BADGE_FONT_SIZE, 8.5);
        }

        /// StorageGauge clamps the fraction and derives it from bytes correctly.
        #[test]
        fn storage_gauge_fraction_from_bytes() {
            assert_eq!(widgets::StorageGauge::new(0.5).fraction(), 0.5);
            // Clamps out of range.
            assert_eq!(widgets::StorageGauge::new(2.0).fraction(), 1.0);
            assert_eq!(widgets::StorageGauge::new(-1.0).fraction(), 0.0);
            // From bytes: half full.
            assert_eq!(widgets::StorageGauge::from_bytes(50, 100).fraction(), 0.5);
            // Zero capacity → empty (no divide-by-zero).
            assert_eq!(widgets::StorageGauge::from_bytes(10, 0).fraction(), 0.0);
        }

        /// The pills, badges, and gauge all render inside a headless frame without panicking.
        #[test]
        fn small_widgets_render_in_headless_frame() {
            let ctx = egui::Context::default();
            super::super::install_fonts(&ctx);
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    // Pills painted into allocated rects.
                    for pill in [
                        Pill::Active { hi_res: false, hovered: false },
                        Pill::Active { hi_res: true, hovered: true },
                        Pill::Hover,
                    ] {
                        let (rect, _) =
                            ui.allocate_at_least(egui::vec2(80.0, 24.0), egui::Sense::hover());
                        widgets::paint_pill(ui.painter(), rect, pill);
                    }
                    let _ = widgets::hi_res_badge(ui);
                    let _ = widgets::IconBadge::new("A").show(ui);
                    let _ = widgets::StorageGauge::from_bytes(60, 100).show(ui);
                });
            });
        }
    }

    // --- Icon mapping (requirements 1.5 & 2.7) ---

    mod icons {
        use super::super::icon::{self, Icon, SIDEBAR_ICON, TOOLBAR_ICON};
        use std::collections::HashSet;

        /// The exact set of SF Symbols the macOS **sidebar** renders (13 NavTab icons +
        /// device/drive/disconnected rows + About). Mirrors `SidebarView.swift`; the
        /// mapping table MUST cover each of these, no more and no less.
        const SIDEBAR_SF_SYMBOLS: &[&str] = &[
            "hifispeaker.fill",
            "folder.badge.gearshape",
            "waveform.path.ecg.rectangle.fill",
            "music.note.list",
            "clock.arrow.circlepath",
            "square.stack.fill",
            "music.mic",
            "waveform.badge.magnifyingglass",
            "character.book.closed.fill",
            "list.bullet.rectangle.portrait.fill",
            "doc.on.doc.fill",
            "arrow.up.arrow.down.circle.fill",
            "arrow.triangle.2.circlepath.circle.fill",
            "info.circle.fill",
            "internaldrive.fill",
            "sdcard.fill",
            "waveform",
            "cable.connector.horizontal",
        ];

        /// The exact set of SF Symbols the macOS **toolbar** renders (`ContentView`).
        const TOOLBAR_SF_SYMBOLS: &[&str] = &["arrow.clockwise", "eject.circle", "sidebar.trailing"];

        /// The sidebar table is exhaustive for the symbols the app uses — every expected
        /// SF Symbol resolves, and the table carries no extra, unused entries.
        #[test]
        fn sidebar_mapping_is_exhaustive() {
            for &sf in SIDEBAR_SF_SYMBOLS {
                assert!(
                    icon::sidebar_icon(sf).is_some(),
                    "sidebar SF Symbol {sf} has no bundled glyph mapping"
                );
            }
            // No extra entries: table length equals the expected symbol count.
            assert_eq!(SIDEBAR_ICON.len(), SIDEBAR_SF_SYMBOLS.len());
            // And every table key is one of the expected symbols.
            let expected: HashSet<&str> = SIDEBAR_SF_SYMBOLS.iter().copied().collect();
            for &(name, _) in SIDEBAR_ICON {
                assert!(expected.contains(name), "unexpected sidebar mapping entry: {name}");
            }
            // The 13 NavTab section icons are present (R1.4/R1.5).
            assert_eq!(SIDEBAR_SF_SYMBOLS.len() - 5, 13);
        }

        /// The toolbar table is exhaustive for the three toolbar actions (R2.7).
        #[test]
        fn toolbar_mapping_is_exhaustive() {
            for &sf in TOOLBAR_SF_SYMBOLS {
                assert!(
                    icon::toolbar_icon(sf).is_some(),
                    "toolbar SF Symbol {sf} has no bundled glyph mapping"
                );
            }
            assert_eq!(TOOLBAR_ICON.len(), TOOLBAR_SF_SYMBOLS.len());
        }

        /// No duplicate SF Symbol keys within a table (a key maps to exactly one glyph).
        #[test]
        fn mapping_keys_are_unique() {
            for table in [SIDEBAR_ICON, TOOLBAR_ICON] {
                let mut seen = HashSet::new();
                for &(name, _) in table {
                    assert!(seen.insert(name), "duplicate SF Symbol key in mapping: {name}");
                }
            }
        }

        /// An unknown SF Symbol resolves to `None` rather than a wrong glyph.
        #[test]
        fn unknown_symbol_resolves_to_none() {
            assert_eq!(icon::sidebar_icon("not.a.real.symbol"), None);
            assert_eq!(icon::toolbar_icon("not.a.real.symbol"), None);
            // A sidebar-only symbol is not resolved by the toolbar table and vice versa.
            assert_eq!(icon::toolbar_icon("hifispeaker.fill"), None);
            assert_eq!(icon::sidebar_icon("arrow.clockwise"), None);
        }

        /// Every mapped glyph name and fallback label is non-empty (nothing renders blank).
        #[test]
        fn glyph_names_and_fallbacks_are_non_empty() {
            for &(_, ic) in SIDEBAR_ICON.iter().chain(TOOLBAR_ICON) {
                assert!(!ic.glyph_name().is_empty(), "{ic:?} has an empty glyph name");
                assert!(!ic.fallback_text().is_empty(), "{ic:?} has an empty fallback");
            }
        }

        /// The design-doc example mappings hold exactly.
        #[test]
        fn documented_example_mappings_hold() {
            assert_eq!(icon::sidebar_icon("hifispeaker.fill"), Some(Icon::Speaker));
            assert_eq!(icon::sidebar_icon("music.note.list"), Some(Icon::ListMusic));
            assert_eq!(icon::sidebar_icon("doc.on.doc.fill"), Some(Icon::Copy));
            assert_eq!(Icon::Speaker.glyph_name(), "speaker");
            assert_eq!(Icon::ListMusic.glyph_name(), "list-music");
            assert_eq!(Icon::Copy.glyph_name(), "copy");
        }

        /// The icon-font asset slot is absent on a non-Windows dev host (no cfg), so views
        /// fall back to text — mirroring the bundled-font slot behavior.
        #[test]
        fn icon_font_slot_defaults_to_none() {
            assert!(super::super::bundled_icon_font().is_none());
        }
    }
}
