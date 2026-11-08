//! Reusable styled widgets for the iced UI, ported from `crate::theme`'s hand-painted
//! egui widgets (`glass`, `button`, pill/badge/gauge painters). Where the egui version
//! painted directly into a `Rect` with `egui::Painter`, these are expressed as iced
//! `Style` resolvers (declarative `Background`/`Border`/`Shadow`) since the flat
//! "Zero Gradient/Glow" macOS design (see `theme::color` doc comment) maps onto that
//! directly — no custom `canvas::Program` painting is needed for these.

pub mod button;
pub mod gauge;
pub mod glass;
pub mod pill;
