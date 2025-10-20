use thiserror::Error;

pub type BrowserResult<T> = Result<T, BrowserError>;

#[derive(Debug, Error)]
pub enum BrowserError {
    #[error("chromium launch failed: {0}")]
    Launch(String),
    #[error("cdp error: {0}")]
    Cdp(#[from] chromiumoxide::error::CdpError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("timeout waiting for {0}")]
    Timeout(String),
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("profile error: {0}")]
    Profile(String),
    #[error("network capture error: {0}")]
    Network(String),
    #[error("metadata extraction failed: {0}")]
    Metadata(String),
    #[error("qa script failure: {0}")]
    Qa(String),
    #[error("unexpected error: {0}")]
    Unexpected(String),
}

impl From<tokio::task::JoinError> for BrowserError {
    fn from(err: tokio::task::JoinError) -> Self {
        BrowserError::Unexpected(err.to_string())
    }
}
