//! Walkman Studio — service layer.
//!
//! Services orchestrate the portable [`walkman_domain`] logic and the
//! [`walkman_platform`] abstraction to deliver application features. Each module
//! below maps to a Swift service/engine and is implemented by the corresponding
//! spec task (7–13). During bootstrap (task 1) they are declared as stubs so the
//! workspace compiles.

pub mod error;

// Service modules (implemented in later tasks). Declared here to fix the module
// layout; bodies are filled in incrementally.
//
pub mod library_scanner; // task 7.1
pub mod local_library; // task 7.2/7.3
// pub mod device_detector;   // task 9.1
// pub mod usb_detector;      // task 9.2
// pub mod walkman_engine;    // task 9.3 / 9.4
// pub mod audio_tagger;      // task 8
pub mod artwork; // task 8.1
pub mod audio_tagger; // task 8.2/8.3/8.4 orchestrator
pub mod clean_eject; // task 9.4
pub mod device_detector; // task 9.1
pub mod duplicate_guard; // task 10.3
pub mod transcode; // task 10.1
pub mod transfer_queue; // task 10.2
pub mod usb_detector; // task 9.2
pub mod walkman_engine; // task 9.3
pub mod ffmpeg_util; // bundled-first ffmpeg/ffprobe discovery (tasks 8.3, 10.1)
pub mod flac_tags; // task 8.2 (native FLAC reader/writer)
pub mod genre; // task 8.2 (genre normalization)
pub mod id3v2; // task 8.3 (native ID3v2.3 reader/writer)
pub mod smfm; // task 8.4 (SMFM payload injection into FLAC/MP3)
// pub mod transcode;         // task 10.1
// pub mod transfer_queue;    // task 10.2
// pub mod duplicate_finder;  // task 11.1
// pub mod duplicate_guard;   // task 10.3
// pub mod backup_restore;    // task 11.2
// pub mod trim_cut;          // task 12.1
// pub mod lyrics;            // task 12.2
// pub mod online_metadata;   // task 12.3
// pub mod artist_art_cache;  // task 12.3
// pub mod cover_art_cache;   // task 12.3
// pub mod player;            // task 13

pub use error::{AppError, Result};
