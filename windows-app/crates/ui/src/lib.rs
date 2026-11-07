//! Walkman Studio — UI layer (egui/eframe).
//!
//! Holds the design system (`theme`), the application shell (`shell`), the sidebar,
//! and the per-section views. During bootstrap (task 1 / task 2) this provides the
//! theme tokens, the `NavTab` navigation enum, and an empty shell window so the app
//! launches; views are added in later tasks.

pub mod detail;
pub mod nav;
pub mod now_playing;
pub mod selection;
pub mod shell;
pub mod sidebar;
pub mod theme;
pub mod toolbar;
pub mod views;

pub use nav::NavTab;
pub use now_playing::{NowPlayingPlaceholder, PlayerRequests};
pub use shell::{layout_shell, AppState};
pub use sidebar::{DeviceSectionMode, DriveRow, SidebarModel};
pub use toolbar::Command;
