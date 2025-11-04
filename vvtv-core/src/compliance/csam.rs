use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use super::{ComplianceError, ComplianceResult};

/// Entry in a CSAM hash database.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct CsamHashEntry {
    /// Hex-encoded SHA-256 hash.
    pub sha256: String,
    /// Optional classification label provided by the authority.
    #[serde(default)]
    pub label: String,
}

/// Finding produced when an asset hash matches the database.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct CsamScanFinding {
    /// Absolute path to the offending asset.
    pub path: PathBuf,
    /// Matching hash.
    pub hash: String,
    /// Classification label.
    pub label: String,
}

/// Report returned by the CSAM scanner.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct CsamScanReport {
    /// Number of files inspected.
    pub files_scanned: usize,
    /// Matches detected during the scan.
    pub matches: Vec<CsamScanFinding>,
}

impl CsamScanReport {
    /// Returns true when the scan yielded no matches.
    pub fn is_clean(&self) -> bool {
        self.matches.is_empty()
    }
}

/// Computes hashes of media assets and compares against the CSAM database.
#[derive(Debug, Clone)]
pub struct CsamScanner {
    database: Vec<CsamHashEntry>,
}

impl CsamScanner {
    /// Loads a scanner from a hash database (JSON or CSV).
    pub fn from_database<P: AsRef<Path>>(path: P) -> ComplianceResult<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(ComplianceError::Missing(format!(
                "CSAM hash database {:?} not found",
                path
            )));
        }
        let content = fs::read_to_string(path).map_err(|err| ComplianceError::io(err, path))?;
        let database = if path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("json"))
            .unwrap_or(false)
            || content.trim_start().starts_with('[')
        {
            serde_json::from_str::<Vec<CsamHashEntry>>(&content)
                .map_err(|err| ComplianceError::json(err, path))?
        } else {
            Self::parse_csv_lines(&content, path)?
        };

        Ok(Self { database })
    }

    /// Scans a directory recursively for matches.
    pub fn scan_directory<P: AsRef<Path>>(&self, dir: P) -> ComplianceResult<CsamScanReport> {
        let dir = dir.as_ref();
        if !dir.exists() {
            return Err(ComplianceError::Missing(format!(
                "CSAM scan directory {:?} not found",
                dir
            )));
        }

        let mut report = CsamScanReport::default();
        for entry in WalkDir::new(dir) {
            let entry = entry.map_err(|err| {
                let path = err
                    .path()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| dir.to_path_buf());
                if let Some(io) = err.io_error() {
                    let owned = std::io::Error::new(io.kind(), io.to_string());
                    ComplianceError::io(owned, path)
                } else {
                    ComplianceError::walkdir(err, path)
                }
            })?;
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            report.files_scanned += 1;
            if let Some(finding) = self.check_file(path)? {
                report.matches.push(finding);
            }
        }

        Ok(report)
    }

    fn parse_csv_lines(content: &str, path: &Path) -> ComplianceResult<Vec<CsamHashEntry>> {
        let mut entries = Vec::new();
        for (idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = trimmed.split(',').collect();
            if parts.is_empty() {
                continue;
            }
            let hash = parts[0].trim();
            if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(ComplianceError::InvalidData(format!(
                    "invalid hash at line {} in {:?}",
                    idx + 1,
                    path
                )));
            }
            let label = parts
                .get(1)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            entries.push(CsamHashEntry {
                sha256: hash.to_lowercase(),
                label,
            });
        }
        Ok(entries)
    }

    fn check_file(&self, path: &Path) -> ComplianceResult<Option<CsamScanFinding>> {
        let mut file = File::open(path).map_err(|err| ComplianceError::io(err, path))?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .map_err(|err| ComplianceError::io(err, path))?;
        let mut hasher = Sha256::new();
        hasher.update(&buffer);
        let hash = hex::encode(hasher.finalize());

        if let Some(entry) = self.database.iter().find(|entry| entry.sha256 == hash) {
            return Ok(Some(CsamScanFinding {
                path: path.to_path_buf(),
                hash,
                label: entry.label.clone(),
            }));
        }

        Ok(None)
    }

    /// Returns true when the database contains at least one hash entry.
    pub fn is_ready(&self) -> bool {
        !self.database.is_empty()
    }

    /// Exposes the raw database for reporting purposes.
    pub fn database(&self) -> &[CsamHashEntry] {
        &self.database
    }
}
