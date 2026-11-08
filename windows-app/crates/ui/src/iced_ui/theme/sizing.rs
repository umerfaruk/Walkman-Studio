//! Sizing baselines, ported verbatim from `crate::theme::sizing` (same macOS-derived
//! window/layout constants, framework-independent).

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

/// Corner radii (macOS `RoundedRectangle` continuous-style radii per surface type).
pub mod radius {
    pub const BUTTON: f32 = 8.0;
    pub const SIDE_ACTION_BUTTON: f32 = 6.0;
    pub const LIQUID_GLASS: f32 = 12.0;
    pub const STUDIO_CARD: f32 = 14.0;
    pub const MODAL_CHASSIS: f32 = 16.0;
    pub const ICON_BADGE: f32 = 9.0;
    pub const HIRES_BADGE: f32 = 3.0;
    pub const CAPSULE: f32 = 999.0;
}
