//! Design tokens for the iced UI, ported from `../../theme.rs` (itself ported from
//! `Sources/WalkmanStudio/Theme.swift`). The macOS source's own header states "Zero
//! Background Gradients" / "Zero Glow" — a flat matte obsidian/silver look with
//! black-only shadows — so these tokens target that flat look directly rather than the
//! glass/blur aesthetic the egui port's doc comments (mis)named it after.

pub mod color;
pub mod font;
pub mod gradient;
pub mod sizing;
