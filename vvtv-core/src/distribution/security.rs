use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::Serialize;
use thiserror::Error;

use sha2::Sha256;

#[derive(Debug, Error)]
pub enum DistributionSecurityError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("hmac error")]
    Hmac,
    #[error("token expired")]
    TokenExpired,
    #[error("invalid token signature")]
    InvalidSignature,
    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
}

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub tls_required: bool,
    pub certificate_path: PathBuf,
    pub token_secret_path: PathBuf,
    pub token_ttl_minutes: u64,
    pub firewall_sources: Vec<String>,
    pub access_log_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct SegmentToken {
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub path: String,
}

pub struct DistributionSecurity {
    config: SecurityConfig,
    secret: Vec<u8>,
}

impl DistributionSecurity {
    pub fn new(config: SecurityConfig) -> Result<Self, DistributionSecurityError> {
        let secret = fs::read(&config.token_secret_path)?;
        Ok(Self { config, secret })
    }

    pub fn tls_required(&self) -> bool {
        self.config.tls_required
    }

    pub fn firewall_sources(&self) -> &[String] {
        &self.config.firewall_sources
    }

    pub fn certificate_path(&self) -> &PathBuf {
        &self.config.certificate_path
    }

    pub fn issue_token(&self, path: &str) -> Result<SegmentToken, DistributionSecurityError> {
        let expires_at =
            Utc::now() + chrono::Duration::minutes(self.config.token_ttl_minutes as i64);
        let signature = self.sign(path, expires_at.timestamp())?;
        let token = URL_SAFE_NO_PAD.encode(signature);
        let segment_token = SegmentToken {
            token,
            expires_at,
            path: path.to_string(),
        };
        self.append_access_log(&segment_token, None)?;
        Ok(segment_token)
    }

    pub fn validate_token(
        &self,
        token: &SegmentToken,
        path: &str,
    ) -> Result<(), DistributionSecurityError> {
        if token.expires_at < Utc::now() {
            return Err(DistributionSecurityError::TokenExpired);
        }
        let expected = URL_SAFE_NO_PAD.encode(self.sign(path, token.expires_at.timestamp())?);
        if expected != token.token {
            return Err(DistributionSecurityError::InvalidSignature);
        }
        Ok(())
    }

    pub fn record_access(
        &self,
        path: &str,
        token: &SegmentToken,
        source_ip: Option<&str>,
    ) -> Result<(), DistributionSecurityError> {
        let mut copy = token.clone();
        copy.path = path.to_string();
        self.append_access_log(&copy, source_ip.map(|ip| ip.to_string()))?;
        Ok(())
    }

    fn sign(&self, path: &str, expires: i64) -> Result<Vec<u8>, DistributionSecurityError> {
        let mut mac = HmacSha256::new_from_slice(&self.secret)
            .map_err(|_| DistributionSecurityError::Hmac)?;
        mac.update(path.as_bytes());
        mac.update(b":");
        mac.update(expires.to_string().as_bytes());
        Ok(mac.finalize().into_bytes().to_vec())
    }

    fn append_access_log(
        &self,
        token: &SegmentToken,
        source_ip: Option<String>,
    ) -> Result<(), DistributionSecurityError> {
        if let Some(parent) = self.config.access_log_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.config.access_log_path)?;
        let entry = AccessLogEntry {
            path: token.path.clone(),
            token: token.token.clone(),
            expires_at: token.expires_at,
            source_ip,
        };
        let json = serde_json::to_string(&entry)?;
        writeln!(file, "{}", json)?;
        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct AccessLogEntry {
    path: String,
    token: String,
    expires_at: DateTime<Utc>,
    source_ip: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn issues_and_validates_token() {
        let dir = tempdir().unwrap();
        let secret_path = dir.path().join("secret.key");
        fs::write(&secret_path, b"super-secret-key").unwrap();
        let config = SecurityConfig {
            tls_required: true,
            certificate_path: dir.path().join("cert.pem"),
            token_secret_path: secret_path.clone(),
            token_ttl_minutes: 5,
            firewall_sources: vec!["tailscale".into(), "cloudflare".into()],
            access_log_path: dir.path().join("access.log"),
        };
        let security = DistributionSecurity::new(config).unwrap();
        let token = security.issue_token("/live/segment1.ts").unwrap();
        security
            .validate_token(&token, "/live/segment1.ts")
            .unwrap();
        assert!(security
            .validate_token(&token, "/live/segment2.ts")
            .is_err());
    }
}
