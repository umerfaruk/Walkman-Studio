//! The iced design system, application shell, and per-section views/sheets — the only
//! rendering layer in this crate as of Milestone 8 of the egui-to-iced migration plan
//! (the egui modules this coexisted alongside through Milestones 1–7 were deleted once
//! this reached parity with them). The sibling top-level modules (`nav`, `selection`,
//! `now_playing`, `inspector`, `views::*`, `sheets::*`) hold the pure, framework-agnostic
//! types/logic every view and sheet in here reuses unchanged. Wired up by the
//! `walkman-studio` binary in `crates/app`.

pub mod now_playing;
pub mod sheets;
pub mod shell;
pub mod theme;
pub mod views;
pub mod widgets;
