use std::io;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config {path}: {source}")]
    Io { source: io::Error, path: PathBuf },
    #[error("failed to parse config {path}: {source}")]
    Parse {
        source: toml::de::Error,
        path: PathBuf,
    },
}

pub type Result<T> = std::result::Result<T, ConfigError>;
