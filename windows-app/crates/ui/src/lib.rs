//! Walkman Studio — UI layer (egui/eframe).
//!
//! Holds the design system (`theme`), the application shell (`shell`), the sidebar,
//! and the per-section views. During bootstrap (task 1 / task 2) this provides the
//! theme tokens, the `NavTab` navigation enum, and an empty shell window so the app
//! launches; views are added in later tasks.

pub mod nav;
pub mod theme;

pub use nav::NavTab;
