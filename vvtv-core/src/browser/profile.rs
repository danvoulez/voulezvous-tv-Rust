use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::config::BrowserConfig;

use super::error::{BrowserError, BrowserResult};

#[derive(Debug, Clone)]
pub struct BrowserProfile {
    id: String,
    path: PathBuf,
    created_at: DateTime<Utc>,
    ttl: Duration,
}

impl BrowserProfile {
    pub fn new(path: PathBuf, ttl: Duration) -> BrowserResult<Self> {
        let id = Uuid::new_v4().to_string();
        let created_at = Utc::now();
        let profile_dir = path.join(&id);
        std::fs::create_dir_all(&profile_dir)
            .map_err(|err| BrowserError::Profile(format!("failed to create profile dir: {err}")))?;
        Ok(Self {
            id,
            path: profile_dir,
            created_at,
            ttl,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    pub async fn touch(&self) -> BrowserResult<()> {
        if self.path.exists() {
            let marker = self.path.join(".last_used");
            let mut file = fs::File::create(&marker).await.map_err(|err| {
                BrowserError::Profile(format!("failed to write profile marker: {err}"))
            })?;
            file.write_all(self.created_at.to_rfc3339().as_bytes())
                .await
                .map_err(|err| {
                    BrowserError::Profile(format!("failed to update profile marker: {err}"))
                })?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ProfileManager {
    base_dir: PathBuf,
    ttl: Duration,
}

impl ProfileManager {
    pub fn new<P: AsRef<Path>>(base_dir: P, ttl: Duration) -> BrowserResult<Self> {
        let base_dir = base_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&base_dir).map_err(|err| {
            BrowserError::Profile(format!("failed to create profile base dir: {err}"))
        })?;
        Ok(Self { base_dir, ttl })
    }

    pub fn from_config(_config: &BrowserConfig, base_dir: &Path) -> BrowserResult<Self> {
        // Profiles live for 24 hours por especificação industrial.
        let ttl = Duration::from_secs(24 * 60 * 60);
        Self::new(base_dir, ttl)
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    pub fn allocate(&self) -> BrowserResult<BrowserProfile> {
        BrowserProfile::new(self.base_dir.clone(), self.ttl)
    }

    pub fn cleanup_expired(&self) -> BrowserResult<()> {
        let ttl = self.ttl;
        let now = SystemTime::now();
        let entries = std::fs::read_dir(&self.base_dir).map_err(|err| {
            BrowserError::Profile(format!("failed to list profile directory: {err}"))
        })?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(err) => {
                    tracing::warn!(path = %path.display(), error = %err, "failed to read profile metadata");
                    continue;
                }
            };
            if let Ok(modified) = metadata.modified() {
                if now.duration_since(modified).unwrap_or(Duration::ZERO) > ttl {
                    if let Err(err) = std::fs::remove_dir_all(&path) {
                        tracing::warn!(path = %path.display(), error = %err, "failed to remove expired profile");
                    }
                }
            }
        }
        Ok(())
    }
}
