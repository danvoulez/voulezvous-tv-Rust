pub mod cdn;
pub mod edge;
pub mod replicator;
pub mod security;

use std::sync::Arc;

use serde::Serialize;
use thiserror::Error;

use crate::monitor::{MetricsStore, MonitorError};

use self::{
    cdn::{BackupCdnManager, CdnCoordinator, CdnError, PrimaryCdnManager},
    edge::{EdgeError, EdgeOrchestrator, EdgeSummary},
    replicator::{ReplicationError, ReplicationManager, ReplicationReport},
    security::{DistributionSecurity, DistributionSecurityError, SegmentToken},
};

#[derive(Debug, Error)]
pub enum DistributionError {
    #[error("replication error: {0}")]
    Replication(#[from] ReplicationError),
    #[error("cdn error: {0}")]
    Cdn(#[from] CdnError),
    #[error("edge error: {0}")]
    Edge(#[from] EdgeError),
    #[error("security error: {0}")]
    Security(#[from] DistributionSecurityError),
    #[error("metrics error: {0}")]
    Metrics(#[from] MonitorError),
}

#[derive(Debug, Serialize)]
pub struct DistributionCycleReport {
    pub replication: ReplicationReport,
    pub primary_cdn: Option<CdnCoordinator>,
    pub backup_cdn: Option<CdnCoordinator>,
    pub edge: Option<EdgeSummary>,
}

pub struct DistributionManager {
    replication: ReplicationManager,
    primary_cdn: Option<PrimaryCdnManager>,
    backup_cdn: Option<BackupCdnManager>,
    edge: Option<EdgeOrchestrator>,
    security: DistributionSecurity,
    metrics: Arc<MetricsStore>,
}

impl DistributionManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        replication: ReplicationManager,
        primary_cdn: Option<PrimaryCdnManager>,
        backup_cdn: Option<BackupCdnManager>,
        edge: Option<EdgeOrchestrator>,
        security: DistributionSecurity,
        metrics: Arc<MetricsStore>,
    ) -> Self {
        Self {
            replication,
            primary_cdn,
            backup_cdn,
            edge,
            security,
            metrics,
        }
    }

    pub fn security(&self) -> &DistributionSecurity {
        &self.security
    }

    pub fn replication_manager(&self) -> &ReplicationManager {
        &self.replication
    }

    pub fn primary_cdn(&self) -> Option<&PrimaryCdnManager> {
        self.primary_cdn.as_ref()
    }

    pub fn backup_cdn(&self) -> Option<&BackupCdnManager> {
        self.backup_cdn.as_ref()
    }

    pub fn edge(&self) -> Option<&EdgeOrchestrator> {
        self.edge.as_ref()
    }

    pub async fn execute_cycle(&self) -> Result<DistributionCycleReport, DistributionError> {
        let replication_report = self.replication.run_cycle().await?;
        self.metrics
            .record_replication_report(&replication_report)?;

        if let Some(primary) = &self.primary_cdn {
            let metrics = primary.fetch_metrics().await?;
            self.metrics.record_cdn_metrics(&metrics)?;
        }

        if let Some(backup) = &self.backup_cdn {
            let report = backup.sync_backup().await?;
            self.metrics.record_backup_sync(&report)?;
        }

        if let Some(edge) = &self.edge {
            let probes = edge.probe_latency().await?;
            for probe in &probes {
                self.metrics.record_edge_latency(probe)?;
            }
        }

        Ok(DistributionCycleReport {
            replication: replication_report,
            primary_cdn: self.primary_cdn.as_ref().map(|p| p.coordinator()),
            backup_cdn: self.backup_cdn.as_ref().map(|b| b.coordinator()),
            edge: self.edge.as_ref().map(|edge| edge.summary()),
        })
    }

    pub fn issue_token(&self, path: &str) -> Result<SegmentToken, DistributionError> {
        let token = self.security.issue_token(path)?;
        self.metrics.record_cdn_token(&token)?;
        Ok(token)
    }
}
