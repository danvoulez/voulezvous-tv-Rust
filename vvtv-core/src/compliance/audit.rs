use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use super::{ComplianceError, ComplianceResult};

/// Representation of a single consent/licensing log entry stored as JSONL.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ConsentLogEntry {
    /// Identifier of the PLAN or asset covered by the consent.
    pub plan_id: String,
    /// Location of the proof (contract, email, blockchain receipt, etc.).
    pub license_proof: Option<String>,
    /// Human or system that granted the consent.
    pub consent_source: Option<String>,
    /// ISO timestamp when consent was verified by the operator.
    pub verified_at: DateTime<Utc>,
    /// Optional expiration timestamp for the consent.
    pub expires_at: Option<DateTime<Utc>>,
    /// Jurisdiction or policy under which the consent was granted.
    pub jurisdiction: Option<String>,
    /// Additional notes captured during compliance review.
    pub notes: Option<String>,
}

impl ConsentLogEntry {
    fn license_proof_is_empty(&self) -> bool {
        self.license_proof
            .as_ref()
            .map(|value| value.trim().is_empty())
            .unwrap_or(true)
    }

    fn consent_source_is_empty(&self) -> bool {
        self.consent_source
            .as_ref()
            .map(|value| value.trim().is_empty())
            .unwrap_or(true)
    }
}

/// Classification of issues discovered during license auditing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum LicenseAuditFindingKind {
    /// No proof document or URL was recorded.
    MissingProof,
    /// No human or automated consent source was recorded.
    MissingConsentSource,
    /// Consent is missing an explicit expiration policy.
    MissingExpiry,
    /// Consent has expired and must be renewed immediately.
    ExpiredConsent,
    /// Consent is close to expiration and requires action.
    ExpiringSoon,
    /// The verification timestamp is older than the permitted maximum.
    VerificationStale,
    /// Multiple conflicting entries found for the same plan identifier.
    DuplicatePlan,
}

/// Detailed finding emitted by the auditor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct LicenseAuditFinding {
    /// The affected plan identifier.
    pub plan_id: String,
    /// Classification for the detected issue.
    pub kind: LicenseAuditFindingKind,
    /// Human readable description of the issue.
    pub message: String,
    /// Timestamp associated with the record (verification or expiration).
    pub reference_time: DateTime<Utc>,
    /// Source file containing the offending entry.
    pub source: PathBuf,
}

/// Summary statistics for a license audit execution.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct LicenseAuditSummary {
    /// Total number of log entries parsed.
    pub total_entries: usize,
    /// Number of distinct plans encountered.
    pub unique_plans: usize,
    /// Findings grouped by kind for quick inspection.
    pub findings_by_kind: HashMap<LicenseAuditFindingKind, usize>,
}

/// Complete report returned by the auditor.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct LicenseAuditReport {
    /// Summary of the executed audit.
    pub summary: LicenseAuditSummary,
    /// Detailed findings, ordered by plan identifier.
    pub findings: Vec<LicenseAuditFinding>,
}

/// Audits consent/licensing JSONL logs to identify compliance gaps.
#[derive(Debug, Clone)]
pub struct LicenseAuditor {
    expiry_grace: Duration,
    verification_max_age: Duration,
}

impl LicenseAuditor {
    /// Creates a new auditor with the provided grace/expiry windows.
    pub fn new(expiry_grace: Duration, verification_max_age: Duration) -> Self {
        Self {
            expiry_grace,
            verification_max_age,
        }
    }

    /// Audits all JSON/JSONL files inside the provided directory.
    pub fn audit_directory<P: AsRef<Path>>(&self, dir: P) -> ComplianceResult<LicenseAuditReport> {
        let dir = dir.as_ref();
        if !dir.exists() {
            return Err(ComplianceError::Missing(format!(
                "license log directory {:?} not found",
                dir
            )));
        }

        let mut entries_by_plan: HashMap<String, Vec<(ConsentLogEntry, PathBuf)>> = HashMap::new();
        let mut total_entries = 0usize;

        for entry in std::fs::read_dir(dir).map_err(|err| ComplianceError::io(err, dir))? {
            let entry = entry.map_err(|err| ComplianceError::io(err, dir))?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if !Self::is_supported_log(&path) {
                continue;
            }
            self.load_file(&path, &mut entries_by_plan, &mut total_entries)?;
        }

        self.build_report(entries_by_plan, total_entries)
    }

    fn is_supported_log(path: &Path) -> bool {
        matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("json" | "jsonl" | "log" | "ndjson")
        )
    }

    fn load_file(
        &self,
        path: &Path,
        entries_by_plan: &mut HashMap<String, Vec<(ConsentLogEntry, PathBuf)>>,
        total_entries: &mut usize,
    ) -> ComplianceResult<()> {
        let file = File::open(path).map_err(|err| ComplianceError::io(err, path))?;
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = line.map_err(|err| ComplianceError::io(err, path))?;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let entry: ConsentLogEntry =
                serde_json::from_str(trimmed).map_err(|err| ComplianceError::json(err, path))?;
            entries_by_plan
                .entry(entry.plan_id.clone())
                .or_default()
                .push((entry, path.to_path_buf()));
            *total_entries += 1;
        }
        Ok(())
    }

    fn build_report(
        &self,
        entries_by_plan: HashMap<String, Vec<(ConsentLogEntry, PathBuf)>>,
        total_entries: usize,
    ) -> ComplianceResult<LicenseAuditReport> {
        let now = Utc::now();
        let mut findings: Vec<LicenseAuditFinding> = Vec::new();
        let mut counts: HashMap<LicenseAuditFindingKind, usize> = HashMap::new();

        for (plan_id, entries) in entries_by_plan.iter() {
            let mut unique_proofs: HashSet<String> = HashSet::new();

            for (entry, path) in entries {
                if entry.license_proof_is_empty() {
                    Self::push_finding(
                        &mut findings,
                        &mut counts,
                        plan_id,
                        LicenseAuditFindingKind::MissingProof,
                        "Registro não possui license_proof".to_string(),
                        entry.verified_at,
                        path.as_path(),
                    );
                } else if let Some(proof) = &entry.license_proof {
                    unique_proofs.insert(proof.clone());
                }

                if entry.consent_source_is_empty() {
                    Self::push_finding(
                        &mut findings,
                        &mut counts,
                        plan_id,
                        LicenseAuditFindingKind::MissingConsentSource,
                        "Registro não informa consent_source".to_string(),
                        entry.verified_at,
                        path.as_path(),
                    );
                }

                match entry.expires_at {
                    Some(expiry) if expiry <= now => {
                        Self::push_finding(
                            &mut findings,
                            &mut counts,
                            plan_id,
                            LicenseAuditFindingKind::ExpiredConsent,
                            format!("Consentimento expirado em {expiry}"),
                            expiry,
                            path.as_path(),
                        );
                    }
                    Some(expiry) if expiry - now <= self.expiry_grace => {
                        Self::push_finding(
                            &mut findings,
                            &mut counts,
                            plan_id,
                            LicenseAuditFindingKind::ExpiringSoon,
                            format!("Consentimento expira em {expiry}"),
                            expiry,
                            path.as_path(),
                        );
                    }
                    None => {
                        Self::push_finding(
                            &mut findings,
                            &mut counts,
                            plan_id,
                            LicenseAuditFindingKind::MissingExpiry,
                            "Registro não define expires_at".to_string(),
                            entry.verified_at,
                            path.as_path(),
                        );
                    }
                    _ => {}
                }

                if now - entry.verified_at > self.verification_max_age {
                    Self::push_finding(
                        &mut findings,
                        &mut counts,
                        plan_id,
                        LicenseAuditFindingKind::VerificationStale,
                        format!(
                            "Verificação ({}) ultrapassa {} dias",
                            entry.verified_at,
                            self.verification_max_age.num_days()
                        ),
                        entry.verified_at,
                        path.as_path(),
                    );
                }
            }

            if unique_proofs.len() > 1 {
                let proofs = unique_proofs.into_iter().collect::<Vec<_>>().join(", ");
                let (entry, path) = &entries[0];
                Self::push_finding(
                    &mut findings,
                    &mut counts,
                    plan_id,
                    LicenseAuditFindingKind::DuplicatePlan,
                    format!("Múltiplos license_proof encontrados: {proofs}"),
                    entry.verified_at,
                    path.as_path(),
                );
            }
        }

        findings.sort_by(|a, b| a.plan_id.cmp(&b.plan_id));

        let summary = LicenseAuditSummary {
            total_entries,
            unique_plans: entries_by_plan.len(),
            findings_by_kind: counts,
        };

        Ok(LicenseAuditReport { summary, findings })
    }

    fn push_finding(
        findings: &mut Vec<LicenseAuditFinding>,
        counts: &mut HashMap<LicenseAuditFindingKind, usize>,
        plan_id: &str,
        kind: LicenseAuditFindingKind,
        message: String,
        reference_time: DateTime<Utc>,
        path: &Path,
    ) {
        *counts.entry(kind.clone()).or_insert(0) += 1;
        findings.push(LicenseAuditFinding {
            plan_id: plan_id.to_string(),
            kind,
            message,
            reference_time,
            source: path.to_path_buf(),
        });
    }
}
