use std::fs::{self, File};
use std::io::Write;

use chrono::{Duration, Utc};
use tempfile::tempdir;
use vvtv_core::{
    ComplianceSuite, ComplianceSuiteConfig, CsamScanner, DrmScanner, LicenseAuditFindingKind,
    LicenseAuditor,
};

#[test]
fn license_auditor_flags_missing_and_expired_entries() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("consent.jsonl");
    let expired = Utc::now() - Duration::days(2);
    let verified = expired - Duration::days(40);
    let mut file = File::create(&log_path).unwrap();
    writeln!(
        file,
        "{}",
        serde_json::json!({
            "plan_id": "plan-001",
            "license_proof": null,
            "consent_source": "",
            "verified_at": verified.to_rfc3339(),
            "expires_at": expired.to_rfc3339(),
            "jurisdiction": "EU",
            "notes": "expired",
        })
    )
    .unwrap();
    writeln!(
        file,
        "{}",
        serde_json::json!({
            "plan_id": "plan-001",
            "license_proof": "https://proof.example/abc",
            "consent_source": "email",
            "verified_at": verified.to_rfc3339(),
            "expires_at": (Utc::now() + Duration::days(2)).to_rfc3339(),
            "jurisdiction": "US",
            "notes": "duplicate"
        })
    )
    .unwrap();

    writeln!(
        file,
        "{}",
        serde_json::json!({
            "plan_id": "plan-001",
            "license_proof": "https://proof.example/def",
            "consent_source": "archive",
            "verified_at": verified.to_rfc3339(),
            "expires_at": (Utc::now() + Duration::days(10)).to_rfc3339(),
            "jurisdiction": "BR",
            "notes": "alternate proof"
        })
    )
    .unwrap();

    let auditor = LicenseAuditor::new(Duration::days(14), Duration::days(30));
    let report = auditor.audit_directory(dir.path()).unwrap();

    assert_eq!(report.summary.total_entries, 3);
    assert_eq!(report.summary.unique_plans, 1);
    assert!(report
        .summary
        .findings_by_kind
        .contains_key(&LicenseAuditFindingKind::MissingProof));
    assert!(report
        .summary
        .findings_by_kind
        .contains_key(&LicenseAuditFindingKind::MissingConsentSource));
    assert!(report
        .summary
        .findings_by_kind
        .contains_key(&LicenseAuditFindingKind::ExpiredConsent));
    assert!(report
        .summary
        .findings_by_kind
        .contains_key(&LicenseAuditFindingKind::DuplicatePlan));
}

#[test]
fn drm_scanner_detects_patterns() {
    let dir = tempdir().unwrap();
    let manifest = dir.path().join("manifest.m3u8");
    fs::write(
        &manifest,
        "#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"skd://drm\"\n#EXTINF:10,\nsegment.ts\n",
    )
    .unwrap();

    let scanner = DrmScanner::new(Default::default());
    let report = scanner.scan_directory(dir.path()).unwrap();
    assert_eq!(report.files_scanned, 1);
    assert_eq!(report.findings.len(), 1);
    assert!(report.findings[0].snippet.contains("EXT-X-KEY"));
}

#[test]
fn csam_scanner_flags_matches() {
    let dir = tempdir().unwrap();
    let hashes = dir.path().join("hashes.csv");
    let media_dir = dir.path().join("media");
    fs::create_dir_all(&media_dir).unwrap();
    let asset_path = media_dir.join("clip.mp4");
    fs::write(&asset_path, b"suspicious").unwrap();
    let hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(b"suspicious");
        hex::encode(hasher.finalize())
    };
    fs::write(&hashes, format!("{hash},alert\n")).unwrap();

    let scanner = CsamScanner::from_database(&hashes).unwrap();
    assert!(scanner.is_ready());
    let report = scanner.scan_directory(&media_dir).unwrap();
    assert_eq!(report.files_scanned, 1);
    assert_eq!(report.matches.len(), 1);
    assert_eq!(report.matches[0].hash, hash);
}

#[test]
fn compliance_suite_runs_all_checks() {
    let dir = tempdir().unwrap();
    let logs_dir = dir.path().join("logs");
    let drm_dir = dir.path().join("manifests");
    let media_dir = dir.path().join("media");
    fs::create_dir_all(&logs_dir).unwrap();
    fs::create_dir_all(&drm_dir).unwrap();
    fs::create_dir_all(&media_dir).unwrap();

    // Prepare license log
    fs::write(
        logs_dir.join("records.jsonl"),
        serde_json::json!({
            "plan_id": "plan-123",
            "license_proof": null,
            "consent_source": "",
            "verified_at": (Utc::now() - Duration::days(40)).to_rfc3339(),
            "expires_at": (Utc::now() - Duration::days(1)).to_rfc3339(),
        })
        .to_string(),
    )
    .unwrap();

    // Prepare manifest with DRM marker
    fs::write(
        drm_dir.join("live.m3u8"),
        "#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"skd://drm\"\n",
    )
    .unwrap();

    // Prepare CSAM hash database and asset
    let hashes = dir.path().join("hashes.csv");
    fs::write(
        &hashes,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855,empty\n",
    )
    .unwrap();
    fs::write(media_dir.join("empty.mp4"), b"").unwrap();

    let mut config = ComplianceSuiteConfig::new();
    config.license_logs_dir = Some(logs_dir.clone());
    config.drm_roots.push(drm_dir.clone());
    config.csam_roots.push(media_dir.clone());
    config.csam_hash_db = Some(hashes.clone());

    let suite = ComplianceSuite::new(config);
    let summary = suite.run().unwrap();

    assert!(summary.license_audit.is_some());
    assert!(summary.drm_scan.is_some());
    assert!(summary.csam_scan.is_some());
    assert!(summary.has_findings());
}
