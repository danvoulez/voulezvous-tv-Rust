use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlanError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("plan {plan_id} not found")]
    NotFound { plan_id: String },
    #[error("plan {plan_id} in unexpected status: {status}")]
    InvalidStatus { plan_id: String, status: String },
    #[error("plan blacklist entry not found for {domain}")]
    BlacklistNotFound { domain: String },
    #[error("plan store path not configured")]
    MissingStore,
    #[error("failed to open database at {path}: {source}")]
    OpenDatabase {
        path: PathBuf,
        source: rusqlite::Error,
    },
}

pub type PlanResult<T> = std::result::Result<T, PlanError>;
