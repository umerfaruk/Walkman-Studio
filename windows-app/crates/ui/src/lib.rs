//! Walkman Studio — UI layer (iced), per Milestone 8 of the egui-to-iced migration
//! plan. `iced_ui` holds the design system, application shell, and per-section views/
//! sheets; the other top-level modules are the pure, framework-agnostic types/logic
//! that both the former egui UI and the current iced UI share (nav sections, selection
//! logic, now-playing data, inspector formatting, and every view/sheet's own enums,
//! `*ViewState` structs, and filter/sort/group functions).

pub mod iced_ui;
pub mod inspector;
pub mod nav;
pub mod now_playing;
pub mod selection;
pub mod sheets;
pub mod views;

pub use nav::NavTab;
pub use now_playing::NowPlayingPlaceholder;
