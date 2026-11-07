//! Walkman Studio — portable domain layer.
//!
//! This crate contains the pure, platform-independent core of the application:
//! data models, artist-name parsing, Walkman text/filename sanitization, and the
//! SensMe 12-Tone analysis math + channel classification.
//!
//! It has **no** OS, I/O, or GUI dependencies so it can be unit-tested in isolation
//! and ported 1:1 from the original macOS/Swift sources.
//!
//! Module map (filled in by spec tasks 5.1–5.4):
//! - [`models`]       — Track, AlbumItem, ArtistItem, WalkmanDrive, WalkmanDevice, ...
//! - [`artist_parse`] — primary/featured artist parsing + known-exceptions list
//! - [`sanitize`]     — Walkman-safe text & filename sanitization, target path layout
//! - [`senseme`]      — 12-Tone analysis + 14-channel classification + SMFM payload

pub mod artist_parse;
pub mod models;
pub mod sanitize;
pub mod senseme;
