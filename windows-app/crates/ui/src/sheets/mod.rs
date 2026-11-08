//! Pure types/logic behind the modal editor sheets, one module per sheet. The rendering
//! itself lives in `iced_ui::sheets` (whose shared `modal()`/`text_field()` helpers
//! replaced this module's egui `Area`-based equivalents once the iced migration reached
//! parity — see Milestone 8 of the egui-to-iced migration plan).

pub mod about;
pub mod album_metadata;
pub mod batch_lyrics;
pub mod clean_eject;
pub mod lyrics_editor;
pub mod song_metadata;
pub mod transfer_item;
pub mod trim_cut;
