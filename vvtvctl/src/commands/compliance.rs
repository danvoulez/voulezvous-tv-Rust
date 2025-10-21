use std::path::PathBuf;

use clap::{Args, Subcommand};

/// Grupo de comandos relacionado a auditoria de compliance.
#[derive(Subcommand, Debug, Clone)]
pub enum ComplianceCommands {
    /// Audita registros de consentimento/licença no formato JSONL.
    Audit(ComplianceAuditArgs),
    /// Procura marcas de DRM/EME em manifests e artefatos HTML.
    Drm(ComplianceDrmArgs),
    /// Executa varredura de hashes CSAM em diretórios locais.
    Csam(ComplianceCsamArgs),
    /// Executa todas as verificações de compliance em sequência.
    Suite(ComplianceSuiteArgs),
}

/// Parâmetros do comando `compliance audit`.
#[derive(Args, Debug, Clone)]
pub struct ComplianceAuditArgs {
    /// Diretório contendo JSON/JSONL com registros de consentimento.
    #[arg(long)]
    pub logs_dir: Option<PathBuf>,
    /// Dias de tolerância antes de sinalizar consentimentos prestes a expirar.
    #[arg(long, default_value_t = 14)]
    pub expiry_grace_days: i64,
    /// Idade máxima (em dias) de uma verificação antes de ser considerada desatualizada.
    #[arg(long, default_value_t = 30)]
    pub verification_max_age_days: i64,
}

/// Parâmetros do comando `compliance drm`.
#[derive(Args, Debug, Clone)]
pub struct ComplianceDrmArgs {
    /// Caminho do diretório contendo manifests/artefatos a inspecionar.
    #[arg(long)]
    pub input: Option<PathBuf>,
}

/// Parâmetros do comando `compliance csam`.
#[derive(Args, Debug, Clone)]
pub struct ComplianceCsamArgs {
    /// Diretório com mídias a serem verificadas.
    #[arg(long)]
    pub media_dir: Option<PathBuf>,
    /// Caminho para a base de hashes (CSV/JSON) usada na comparação.
    #[arg(long)]
    pub hash_db: Option<PathBuf>,
}

/// Parâmetros do comando `compliance suite`.
#[derive(Args, Debug, Clone)]
pub struct ComplianceSuiteArgs {
    /// Diretório com logs de consentimento.
    #[arg(long)]
    pub logs_dir: Option<PathBuf>,
    /// Diretórios com manifests/artefatos para inspeção DRM.
    #[arg(long = "manifests-dir")]
    pub manifests_dir: Vec<PathBuf>,
    /// Diretórios com mídias para varredura de hashes CSAM.
    #[arg(long = "media-dir")]
    pub media_dir: Vec<PathBuf>,
    /// Caminho para a base de hashes (CSV/JSON) usada na comparação.
    #[arg(long)]
    pub hash_db: Option<PathBuf>,
    /// Dias de tolerância antes de sinalizar consentimentos prestes a expirar.
    #[arg(long, default_value_t = 14)]
    pub expiry_grace_days: i64,
    /// Idade máxima (em dias) de uma verificação antes de ser considerada desatualizada.
    #[arg(long, default_value_t = 30)]
    pub verification_max_age_days: i64,
}
