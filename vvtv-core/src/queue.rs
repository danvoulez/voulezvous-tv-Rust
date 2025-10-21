use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags};
use thiserror::Error;

const QUEUE_SCHEMA: &str = include_str!("../../sql/queue.sql");

#[derive(Debug, Error)]
pub enum QueueError {
    #[error("failed to open queue database {path}: {source}")]
    Open {
        source: rusqlite::Error,
        path: PathBuf,
    },
    #[error("failed to execute statement on queue database: {0}")]
    Execute(#[from] rusqlite::Error),
    #[error("queue path not configured")]
    MissingStore,
}

pub type QueueResult<T> = Result<T, QueueError>;

#[derive(Debug, Clone, Default)]
pub struct QueueItem {
    pub plan_id: String,
    pub asset_path: String,
    pub duration_s: Option<i64>,
    pub curation_score: Option<f64>,
    pub priority: i64,
    pub node_origin: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PlayoutQueueStoreBuilder {
    path: Option<PathBuf>,
    read_only: bool,
    create_if_missing: bool,
}

impl Default for PlayoutQueueStoreBuilder {
    fn default() -> Self {
        Self {
            path: None,
            read_only: false,
            create_if_missing: true,
        }
    }
}

impl PlayoutQueueStoreBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn path(mut self, path: impl AsRef<Path>) -> Self {
        self.path = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn read_only(mut self, value: bool) -> Self {
        self.read_only = value;
        self
    }

    pub fn create_if_missing(mut self, value: bool) -> Self {
        self.create_if_missing = value;
        self
    }

    pub fn build(self) -> QueueResult<PlayoutQueueStore> {
        let path = self.path.ok_or(QueueError::MissingStore)?;
        let mut flags = if self.read_only {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE
        };
        if !self.read_only && self.create_if_missing {
            flags |= OpenFlags::SQLITE_OPEN_CREATE;
        }
        Ok(PlayoutQueueStore { path, flags })
    }
}

#[derive(Debug, Clone)]
pub struct PlayoutQueueStore {
    path: PathBuf,
    flags: OpenFlags,
}

impl PlayoutQueueStore {
    pub fn builder() -> PlayoutQueueStoreBuilder {
        PlayoutQueueStoreBuilder::new()
    }

    pub fn new(path: impl AsRef<Path>) -> QueueResult<Self> {
        PlayoutQueueStoreBuilder::new().path(path).build()
    }

    fn open(&self) -> QueueResult<Connection> {
        Connection::open_with_flags(&self.path, self.flags).map_err(|source| QueueError::Open {
            source,
            path: self.path.clone(),
        })
    }

    pub fn initialize(&self) -> QueueResult<()> {
        let conn = self.open()?;
        conn.execute_batch(QUEUE_SCHEMA)?;
        Ok(())
    }

    pub fn enqueue(&self, item: &QueueItem) -> QueueResult<i64> {
        let conn = self.open()?;
        conn.execute(
            "INSERT INTO playout_queue (
                plan_id, asset_path, duration_s, status, curation_score, priority, node_origin
            ) VALUES (?1, ?2, ?3, 'queued', ?4, ?5, ?6)",
            params![
                &item.plan_id,
                &item.asset_path,
                &item.duration_s,
                &item.curation_score,
                item.priority,
                &item.node_origin
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }
}
