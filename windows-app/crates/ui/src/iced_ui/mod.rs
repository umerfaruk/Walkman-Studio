//! iced-based UI layer — migration target for the egui/eframe UI in the rest of this
//! crate (see `../theme.rs`, `../shell.rs`, etc.).
//!
//! This module is additive: nothing under `crate::theme`, `crate::shell`, or any other
//! existing egui module is touched. The two UI stacks coexist in this crate until the
//! iced shell reaches feature parity with the shipping egui one, at which point the
//! egui modules are deleted and this module takes over `crate::theme`/`crate::shell`'s
//! names. Wired up by the `walkman-studio-iced` binary in `crates/app`; the shipping
//! `walkman-studio` binary is untouched and keeps using the egui modules.

pub mod now_playing;
pub mod sheets;
pub mod shell;
pub mod theme;
pub mod views;
pub mod widgets;
