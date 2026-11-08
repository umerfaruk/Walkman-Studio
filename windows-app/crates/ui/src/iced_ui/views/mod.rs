//! Per-section views for the iced shell (Milestone 5 of the egui-to-iced migration
//! plan). Each module reuses its egui counterpart's pure, framework-agnostic types and
//! logic unchanged (display-mode/sort enums, the `*ViewState` struct, filter/sort/group
//! functions — none of them touch `egui`) and only rewrites the `render` function itself
//! as an iced `view`/`update` pair. See `local_library.rs` for the pattern.

pub mod local_library;
pub mod recently_added;
