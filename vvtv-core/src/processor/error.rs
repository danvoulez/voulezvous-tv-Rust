use std::path::PathBuf;

use thiserror::Error;

use crate::browser::BrowserError;
use crate::plan::PlanError;
use crate::queue::QueueError;

#[derive(Debug, Error)]
pub enum ProcessorError {
    #[error("plan {plan_id} missing source url")]
    MissingSourceUrl { plan_id: String },
    #[error("play-before-download failed: {0}")]
    Capture(String),
    #[error("download failed: {0}")]
    Download(String),
    #[error("invalid media: {0}")]
    InvalidMedia(String),
    #[error("io error at {path}: {source}")]
    Io {
        source: std::io::Error,
        path: PathBuf,
    },
    #[error("network error: {0}")]
    Network(String),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("transcode operation failed: {0}")]
    Transcode(String),
    #[error("quality control failed: {0}")]
    Qc(String),
    #[error("database error: {0}")]
    Database(String),
}

impl From<BrowserError> for ProcessorError {
    fn from(error: BrowserError) -> Self {
        ProcessorError::Capture(error.to_string())
    }
}

impl From<PlanError> for ProcessorError {
    fn from(error: PlanError) -> Self {
        ProcessorError::Database(error.to_string())
    }
}

impl From<QueueError> for ProcessorError {
    fn from(error: QueueError) -> Self {
        ProcessorError::Database(error.to_string())
    }
}

impl From<serde_json::Error> for ProcessorError {
    fn from(error: serde_json::Error) -> Self {
        ProcessorError::Serialization(error.to_string())
    }
}

impl From<reqwest::Error> for ProcessorError {
    fn from(error: reqwest::Error) -> Self {
        ProcessorError::Network(error.to_string())
    }
}

impl From<std::io::Error> for ProcessorError {
    fn from(source: std::io::Error) -> Self {
        ProcessorError::Io {
            path: PathBuf::new(),
            source,
        }
    }
}

pub type ProcessorResult<T> = Result<T, ProcessorError>;
