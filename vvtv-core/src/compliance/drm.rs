use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use super::{ComplianceError, ComplianceResult};

/// Configuration describing which patterns must be considered DRM/EME indicators.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrmDetectionConfig {
    /// Regular expressions executed against manifest or HTML files.
    pub patterns: Vec<String>,
    /// Only files with these extensions are scanned.
    pub allowed_extensions: Vec<String>,
}

impl Default for DrmDetectionConfig {
    fn default() -> Self {
        Self {
            patterns: vec![
                String::from(r"EXT-X-KEY"),
                String::from(r"widevine"),
                String::from(r"playready"),
                String::from(r"fairplay"),
                String::from(r"encryptedmedia"),
                String::from(r"urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed"),
                String::from(r"com\.microsoft\.playready"),
                String::from(r"skd://"),
            ],
            allowed_extensions: vec![
                "m3u8".into(),
                "mpd".into(),
                "ism".into(),
                "html".into(),
                "js".into(),
                "json".into(),
            ],
        }
    }
}

/// A single DRM finding with contextual snippet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct DrmScanFinding {
    /// File where the pattern was discovered.
    pub path: PathBuf,
    /// Regex pattern that triggered the finding.
    pub pattern: String,
    /// Snippet of text around the match for manual review.
    pub snippet: String,
}

/// Aggregated report for a DRM scan.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct DrmScanReport {
    /// Number of files inspected during the scan.
    pub files_scanned: usize,
    /// Collected findings.
    pub findings: Vec<DrmScanFinding>,
}

impl DrmScanReport {
    /// Returns `true` when no DRM patterns were detected.
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }
}

/// Scans manifests and HTML for DRM/EME markers.
#[derive(Debug, Clone)]
pub struct DrmScanner {
    config: DrmDetectionConfig,
    compiled: Vec<Regex>,
}

impl DrmScanner {
    /// Creates a new scanner using the provided configuration.
    pub fn new(config: DrmDetectionConfig) -> Self {
        let compiled = config
            .patterns
            .iter()
            .filter_map(|pattern| Regex::new(&pattern.to_lowercase()).ok())
            .collect();
        Self { config, compiled }
    }

    /// Scans a directory recursively, returning a report with all findings.
    pub fn scan_directory<P: AsRef<Path>>(&self, dir: P) -> ComplianceResult<DrmScanReport> {
        let dir = dir.as_ref();
        if !dir.exists() {
            return Err(ComplianceError::Missing(format!(
                "DRM scan directory {:?} not found",
                dir
            )));
        }

        let mut report = DrmScanReport::default();
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
            if !self.is_allowed(path) {
                continue;
            }

            report.files_scanned += 1;
            if let Some(finding) = self.scan_file(path)? {
                report.findings.push(finding);
            }
        }

        Ok(report)
    }

    fn is_allowed(&self, path: &Path) -> bool {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some(ext) => {
                let normalized = ext.to_lowercase();
                self.config
                    .allowed_extensions
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(&normalized))
            }
            None => false,
        }
    }

    fn scan_file(&self, path: &Path) -> ComplianceResult<Option<DrmScanFinding>> {
        let content = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::InvalidData => {
                return Ok(None);
            }
            Err(err) => return Err(ComplianceError::io(err, path)),
        };
        let lowered = content.to_lowercase();

        for regex in &self.compiled {
            if let Some(mat) = regex.find(&lowered) {
                let snippet = Self::extract_snippet(&content, mat.start(), mat.end());
                return Ok(Some(DrmScanFinding {
                    path: path.to_path_buf(),
                    pattern: regex.as_str().to_string(),
                    snippet,
                }));
            }
        }

        Ok(None)
    }

    fn extract_snippet(content: &str, start: usize, end: usize) -> String {
        let bytes = content.as_bytes();
        let mut left = start;
        while left > 0 && bytes[left - 1] != b'\n' {
            left -= 1;
            if start - left > 120 {
                break;
            }
        }
        let mut right = end;
        while right < bytes.len() && bytes[right] != b'\n' {
            right += 1;
            if right - end > 120 {
                break;
            }
        }
        content[left..right].trim().to_string()
    }
}
