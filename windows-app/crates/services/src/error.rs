//! Shared service error type. Per-domain variants mirror the design's error-handling
//! strategy; the UI maps these to the same dialogs/alerts as the macOS app.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("audio file not found: {0}")]
    FileNotFound(String),

    #[error("metadata/tagging error: {0}")]
    Tag(String),

    #[error("transcode error: {0}")]
    Transcode(String),

    #[error("backup/restore error: {0}")]
    Backup(String),

    #[error("device error: {0}")]
    Device(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("database error: {0}")]
    Database(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Database(e.to_string())
    }
}
