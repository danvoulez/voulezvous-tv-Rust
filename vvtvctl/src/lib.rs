#![allow(
    clippy::result_large_err,
    clippy::field_reassign_with_default,
    clippy::to_string_in_format_args,
    clippy::vec_init_then_push
)]

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::sync::Arc;

use chrono::{DateTime, Duration, SecondsFormat, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use thiserror::Error;
use tokio::runtime::Builder;
use vvtv_core::{
    load_broadcaster_config, load_browser_config, load_processor_config, load_vvtv_config,
    BrowserError, BrowserLauncher, BrowserQaRunner, ConfigBundle, DashboardGenerator, MetricRecord,
    MetricsStore, MonitorError, Plan, PlanAuditFinding, PlanAuditKind, PlanBlacklistEntry,
    PlanImportRecord, PlanMetrics, PlanStatus, PlayoutQueueStore, ProfileManager, QaMetricsStore,
    QaStatistics, QueueEntry as QueueStoreEntry, QueueError, QueueFilter, QueueMetrics,
    QueueStatus, SessionRecorder, SessionRecorderConfig, SmokeMode, SmokeTestOptions,
    SmokeTestResult, SqlitePlanStore,
};

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("config error: {0}")]
    Config(#[from] vvtv_core::ConfigError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("plan error: {0}")]
    Plan(#[from] vvtv_core::PlanError),
    #[error("queue error: {0}")]
    Queue(#[from] QueueError),
    #[error("monitor error: {0}")]
    Monitor(#[from] MonitorError),
    #[error("browser automation error: {0}")]
    Browser(#[from] BrowserError),
    #[error("authentication failed")]
    Authentication,
    #[error("required resource missing: {0}")]
    MissingResource(String),
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("script execution failed with status {status:?}: {stderr}")]
    ScriptFailure { status: Option<i32>, stderr: String },
}

#[derive(Parser, Debug)]
#[command(author, version, about = "VVTV command-line control interface", long_about = None)]
pub struct Cli {
    /// Caminho do vvtv.toml principal
    #[arg(long, default_value = "configs/vvtv.toml")]
    pub config: PathBuf,
    /// Caminho alternativo para browser.toml
    #[arg(long)]
    pub browser_config: Option<PathBuf>,
    /// Caminho alternativo para processor.toml
    #[arg(long)]
    pub processor_config: Option<PathBuf>,
    /// Caminho alternativo para broadcaster.toml
    #[arg(long)]
    pub broadcaster_config: Option<PathBuf>,
    /// Diretório override para dados (substitui paths.data_dir)
    #[arg(long)]
    pub data_dir: Option<PathBuf>,
    /// Diretório contendo scripts do sistema
    #[arg(long)]
    pub scripts_dir: Option<PathBuf>,
    /// Caminho alternativo para plans.sqlite
    #[arg(long)]
    pub plans_db: Option<PathBuf>,
    /// Caminho alternativo para queue.sqlite
    #[arg(long)]
    pub queue_db: Option<PathBuf>,
    /// Caminho alternativo para metrics.sqlite
    #[arg(long)]
    pub metrics_db: Option<PathBuf>,
    /// Caminho alternativo para o script fill_buffer.sh
    #[arg(long)]
    pub fill_script: Option<PathBuf>,
    /// Token para autenticação local (se VVTVCTL_TOKEN estiver definido)
    #[arg(long)]
    pub token: Option<String>,
    /// Formato de saída
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Exibe status operacional resumido
    Status,
    /// Operações relacionadas a PLANs
    #[command(subcommand)]
    Plan(PlanCommands),
    /// Operações relacionadas à fila de playout
    #[command(subcommand)]
    Queue(QueueCommands),
    /// Gerenciamento do buffer operacional
    #[command(subcommand)]
    Buffer(BufferCommands),
    /// Executa verificações de integridade
    #[command(name = "health")]
    #[command(subcommand)]
    Health(HealthCommands),
    /// Ferramentas de QA
    #[command(subcommand)]
    Qa(QaCommands),
}

#[derive(Subcommand, Debug)]
pub enum PlanCommands {
    /// Lista planos registrados no banco
    List(PlanListArgs),
    /// Executa auditoria de planos
    Audit(PlanAuditArgs),
    /// Gerencia blacklist de planos
    #[command(subcommand)]
    Blacklist(PlanBlacklistCommands),
    /// Importa planos a partir de um arquivo JSON
    Import(PlanImportArgs),
}

#[derive(Args, Debug)]
pub struct PlanListArgs {
    /// Filtrar por status específico
    #[arg(long)]
    pub status: Option<String>,
    /// Limite de registros retornados
    #[arg(long, default_value_t = 10)]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct PlanAuditArgs {
    /// Mostrar apenas findings com idade maior que este limite (horas)
    #[arg(long, default_value_t = 0.0)]
    pub min_age_hours: f64,
    /// Filtrar por tipo específico de finding
    #[arg(long)]
    pub kind: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum PlanBlacklistCommands {
    /// Lista entradas da blacklist
    List,
    /// Adiciona domínio à blacklist
    Add(PlanBlacklistAddArgs),
    /// Remove domínio da blacklist
    Remove(PlanBlacklistRemoveArgs),
}

#[derive(Args, Debug)]
pub struct PlanBlacklistAddArgs {
    /// Domínio a ser bloqueado
    pub domain: String,
    /// Motivo opcional
    #[arg(long)]
    pub reason: Option<String>,
}

#[derive(Args, Debug)]
pub struct PlanBlacklistRemoveArgs {
    /// Domínio a ser removido
    pub domain: String,
}

#[derive(Args, Debug)]
pub struct PlanImportArgs {
    /// Caminho do arquivo JSON contendo planos
    pub path: PathBuf,
    /// Força sobrescrita de planos existentes
    #[arg(long, default_value_t = false)]
    pub overwrite: bool,
}

#[derive(Subcommand, Debug)]
pub enum QueueCommands {
    /// Lista itens da fila de playout
    Show(QueueShowArgs),
    /// Exibe resumo da fila
    Summary,
    /// Ajusta prioridade de um item
    Promote(QueuePromoteArgs),
    /// Remove item da fila
    Remove(QueueRemoveArgs),
    /// Limpa itens reproduzidos mais antigos
    Cleanup(QueueCleanupArgs),
    /// Exporta backup compactado da fila
    Backup(QueueBackupArgs),
}

#[derive(Args, Debug)]
pub struct QueueShowArgs {
    /// Filtrar por status
    #[arg(long)]
    pub status: Option<String>,
    /// Limite de registros
    #[arg(long, default_value_t = 10)]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct QueuePromoteArgs {
    /// ID do item na fila
    pub id: i64,
    /// Nova prioridade (0 = normal, 1 = alta)
    #[arg(long, default_value_t = 1)]
    pub priority: i64,
}

#[derive(Args, Debug)]
pub struct QueueRemoveArgs {
    /// ID do item na fila
    pub id: i64,
}

#[derive(Args, Debug)]
pub struct QueueCleanupArgs {
    /// Limite em horas para remoção de itens `played`
    #[arg(long, default_value_t = 72)]
    pub older_than_hours: i64,
}

#[derive(Args, Debug)]
pub struct QueueBackupArgs {
    /// Caminho do arquivo `.sql.gz`
    pub output: PathBuf,
}

#[derive(Subcommand, Debug)]
pub enum BufferCommands {
    /// Dispara o script fill_buffer.sh
    Fill(BufferFillArgs),
}

#[derive(Args, Debug)]
pub struct BufferFillArgs {
    /// Alvo de horas de buffer
    #[arg(long)]
    pub target_hours: Option<f64>,
    /// Executa em modo dry-run
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct HealthDashboardArgs {
    /// Caminho de saída do dashboard
    #[arg(long)]
    pub output: Option<PathBuf>,
    /// Número de pontos exibidos
    #[arg(long, default_value_t = 48)]
    pub points: usize,
}

#[derive(Subcommand, Debug)]
pub enum HealthCommands {
    /// Executa checagens básicas
    Check,
    /// Gera dashboard HTML com histórico recente
    Dashboard(HealthDashboardArgs),
}

#[derive(Subcommand, Debug)]
pub enum QaCommands {
    /// Executa smoke test do curator
    SmokeTest(QaSmokeArgs),
    /// Gera dashboard HTML de QA
    Report(QaReportArgs),
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SmokeModeValue {
    Headless,
    Headed,
}

impl From<SmokeModeValue> for SmokeMode {
    fn from(value: SmokeModeValue) -> Self {
        match value {
            SmokeModeValue::Headless => SmokeMode::Headless,
            SmokeModeValue::Headed => SmokeMode::Headed,
        }
    }
}

#[derive(Args, Debug)]
pub struct QaSmokeArgs {
    /// URL alvo do smoke test
    pub url: String,
    /// Modo de execução (headed/headless)
    #[arg(long, value_enum, default_value_t = SmokeModeValue::Headless)]
    pub mode: SmokeModeValue,
    /// Diretório para salvar screenshots
    #[arg(long)]
    pub screenshot_dir: Option<PathBuf>,
    /// Não captura screenshot
    #[arg(long, default_value_t = false)]
    pub no_screenshot: bool,
    /// Ativa gravação de vídeo da sessão
    #[arg(long, default_value_t = false)]
    pub record_video: bool,
    /// Diretório para salvar vídeos
    #[arg(long)]
    pub video_dir: Option<PathBuf>,
    /// Caminho alternativo para ffmpeg
    #[arg(long)]
    pub ffmpeg_path: Option<PathBuf>,
    /// Duração máxima da captura de vídeo (segundos)
    #[arg(long, default_value_t = 30)]
    pub record_duration: u64,
}

#[derive(Args, Debug)]
pub struct QaReportArgs {
    /// Caminho do arquivo HTML de saída
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QaSmokeReport {
    pub result: SmokeTestResult,
}

#[derive(Debug, Clone, Serialize)]
pub struct QaReportResult {
    pub output: PathBuf,
    pub stats: QaStatistics,
}

pub fn run(cli: Cli) -> Result<()> {
    enforce_token(&cli)?;
    let context = AppContext::new(&cli)?;

    match &cli.command {
        Commands::Status => {
            let status = context.gather_status()?;
            render(&status, cli.format)?;
        }
        Commands::Plan(command) => match command {
            PlanCommands::List(args) => {
                let plans = context.plan_list(args)?;
                render(&plans, cli.format)?;
            }
            PlanCommands::Audit(args) => {
                let audit = context.plan_audit(args)?;
                render(&audit, cli.format)?;
            }
            PlanCommands::Blacklist(args) => {
                let result = context.plan_blacklist(args)?;
                render(&result, cli.format)?;
            }
            PlanCommands::Import(args) => {
                let result = context.plan_import(args)?;
                render(&result, cli.format)?;
            }
        },
        Commands::Queue(command) => match command {
            QueueCommands::Show(args) => {
                let queue = context.queue_show(args)?;
                render(&queue, cli.format)?;
            }
            QueueCommands::Summary => {
                let summary = context.queue_summary()?;
                render(&summary, cli.format)?;
            }
            QueueCommands::Promote(args) => {
                let result = context.queue_promote(args)?;
                render(&result, cli.format)?;
            }
            QueueCommands::Remove(args) => {
                let result = context.queue_remove(args)?;
                render(&result, cli.format)?;
            }
            QueueCommands::Cleanup(args) => {
                let result = context.queue_cleanup(args)?;
                render(&result, cli.format)?;
            }
            QueueCommands::Backup(args) => {
                let result = context.queue_backup(args)?;
                render(&result, cli.format)?;
            }
        },
        Commands::Buffer(BufferCommands::Fill(args)) => {
            let result = context.buffer_fill(args)?;
            render(&result, cli.format)?;
        }
        Commands::Health(command) => match command {
            HealthCommands::Check => {
                let report = context.health_check()?;
                render(&report, cli.format)?;
                if report
                    .iter()
                    .any(|entry| matches!(entry.status, CheckStatus::Error))
                {
                    return Err(AppError::MissingResource(
                        "Uma ou mais verificações falharam".to_string(),
                    ));
                }
            }
            HealthCommands::Dashboard(args) => {
                let result = context.health_dashboard(args)?;
                render(&result, cli.format)?;
            }
        },
        Commands::Qa(command) => match command {
            QaCommands::SmokeTest(args) => {
                let report = context.qa_smoke_test(args)?;
                render(&report, cli.format)?;
            }
            QaCommands::Report(args) => {
                let report = context.qa_report(args)?;
                render(&report, cli.format)?;
            }
        },
    }

    Ok(())
}

fn enforce_token(cli: &Cli) -> Result<()> {
    if let Ok(expected) = std::env::var("VVTVCTL_TOKEN") {
        match &cli.token {
            Some(provided) if provided == &expected => Ok(()),
            _ => Err(AppError::Authentication),
        }
    } else {
        Ok(())
    }
}

fn render<T>(value: &T, format: OutputFormat) -> Result<()>
where
    T: Serialize + DisplayFallback,
{
    match format {
        OutputFormat::Text => {
            println!("{}", value.display());
            Ok(())
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(value)?;
            println!("{}", json);
            Ok(())
        }
    }
}

trait DisplayFallback {
    fn display(&self) -> String;
}

#[derive(Debug)]
struct AppContext {
    bundle: ConfigBundle,
    config_path: PathBuf,
    browser_path: PathBuf,
    processor_path: PathBuf,
    broadcaster_path: PathBuf,
    data_dir: PathBuf,
    plans_db: PathBuf,
    queue_db: PathBuf,
    metrics_db: PathBuf,
    fill_script: PathBuf,
}

impl AppContext {
    fn new(cli: &Cli) -> Result<Self> {
        let config_path = cli.config.clone();
        let vvtv = load_vvtv_config(&config_path)?;

        let config_dir = config_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        let browser_path = cli
            .browser_config
            .clone()
            .unwrap_or_else(|| config_dir.join("browser.toml"));
        let processor_path = cli
            .processor_config
            .clone()
            .unwrap_or_else(|| config_dir.join("processor.toml"));
        let broadcaster_path = cli
            .broadcaster_config
            .clone()
            .unwrap_or_else(|| config_dir.join("broadcaster.toml"));

        let browser = load_browser_config(&browser_path)?;
        let processor = load_processor_config(&processor_path)?;
        let broadcaster = load_broadcaster_config(&broadcaster_path)?;
        let bundle = ConfigBundle {
            vvtv: vvtv.clone(),
            browser,
            processor,
            broadcaster,
        };

        let default_data = PathBuf::from(&bundle.vvtv.paths.data_dir);
        let data_dir = cli.data_dir.clone().unwrap_or_else(|| default_data.clone());

        let scripts_dir = cli.scripts_dir.clone().unwrap_or_else(|| {
            let candidate = config_dir.join("../scripts/system");
            if candidate.exists() {
                candidate
            } else {
                PathBuf::from("scripts/system")
            }
        });

        let plans_db = cli
            .plans_db
            .clone()
            .unwrap_or_else(|| data_dir.join("plans.sqlite"));
        let queue_db = cli
            .queue_db
            .clone()
            .unwrap_or_else(|| data_dir.join("queue.sqlite"));
        let metrics_db = cli
            .metrics_db
            .clone()
            .unwrap_or_else(|| data_dir.join("metrics.sqlite"));
        let fill_script = cli
            .fill_script
            .clone()
            .unwrap_or_else(|| scripts_dir.join("fill_buffer.sh"));

        Ok(Self {
            bundle,
            config_path,
            browser_path,
            processor_path,
            broadcaster_path,
            data_dir,
            plans_db,
            queue_db,
            metrics_db,
            fill_script,
        })
    }

    fn gather_status(&self) -> Result<StatusReport> {
        let node = NodeStatus {
            node_name: self.bundle.vvtv.system.node_name.clone(),
            node_role: self.bundle.vvtv.system.node_role.clone(),
            environment: self.bundle.vvtv.system.environment.clone(),
        };

        let plan_metrics = self.plan_metrics();
        let plan_counts = plan_metrics
            .as_ref()
            .map(|metrics| metrics.by_status.clone())
            .unwrap_or_default();
        let queue_counts = self.queue_summary_map().unwrap_or_default();
        let queue_metrics = self.queue_metrics().ok();
        if let Some(metrics) = &queue_metrics {
            let _ = self.record_metrics(metrics);
        }
        let metrics = self.metrics_snapshot()?;

        Ok(StatusReport {
            node,
            plan_counts,
            queue_counts,
            metrics,
            plan_metrics,
        })
    }

    fn plan_list(&self, args: &PlanListArgs) -> Result<PlanList> {
        let status = match &args.status {
            Some(value) => Some(
                PlanStatus::from_str(value)
                    .map_err(|_| AppError::InvalidArgument(format!("status inválido: {value}")))?,
            ),
            None => None,
        };
        let store = self.plan_store(true)?;
        let plans = store.list_by_status(status, args.limit)?;
        let rows = plans
            .into_iter()
            .map(|plan| PlanEntry {
                plan_id: plan.plan_id,
                title: plan.title,
                status: plan.status.to_string(),
                duration_est_s: plan.duration_est_s,
                curation_score: Some(plan.curation_score),
                updated_at: format_datetime(plan.updated_at),
                created_at: format_datetime(plan.created_at),
                kind: plan.kind,
                hd_missing: plan.hd_missing,
            })
            .collect();

        Ok(PlanList { rows })
    }

    fn plan_audit(&self, args: &PlanAuditArgs) -> Result<PlanAuditReport> {
        let store = self.plan_store(true)?;
        let mut findings = store.audit(chrono::Utc::now())?;
        if let Some(kind) = &args.kind {
            let filter = parse_audit_kind(kind)?;
            findings.retain(|finding| finding.kind == filter);
        }
        if args.min_age_hours > 0.0 {
            findings.retain(|finding| finding.age_hours >= args.min_age_hours);
        }
        Ok(PlanAuditReport { findings })
    }

    fn plan_blacklist(&self, command: &PlanBlacklistCommands) -> Result<PlanBlacklistResult> {
        match command {
            PlanBlacklistCommands::List => {
                let store = self.plan_store(true)?;
                let entries = store.blacklist_list()?;
                Ok(PlanBlacklistResult::List { entries })
            }
            PlanBlacklistCommands::Add(args) => {
                let store = self.plan_store(false)?;
                let entry = store.blacklist_add(&args.domain, args.reason.as_deref())?;
                Ok(PlanBlacklistResult::Ack {
                    message: format!("Domínio {} adicionado", entry.domain),
                })
            }
            PlanBlacklistCommands::Remove(args) => {
                let store = self.plan_store(false)?;
                store.blacklist_remove(&args.domain)?;
                Ok(PlanBlacklistResult::Ack {
                    message: format!("Domínio {} removido", args.domain),
                })
            }
        }
    }

    fn plan_import(&self, args: &PlanImportArgs) -> Result<PlanImportResult> {
        if !args.path.exists() {
            return Err(AppError::MissingResource(format!(
                "Arquivo não encontrado: {}",
                args.path.display()
            )));
        }
        let content = fs::read_to_string(&args.path)?;
        let mut records = parse_plan_import(&content, args.overwrite)?;
        if args.overwrite {
            for record in &mut records {
                record.overwrite = true;
            }
        }
        let store = self.plan_store(false)?;
        let imported = store.import(&records)?;
        Ok(PlanImportResult {
            imported,
            total: records.len(),
        })
    }

    fn buffer_fill(&self, args: &BufferFillArgs) -> Result<BufferFillResult> {
        if !self.fill_script.exists() {
            return Err(AppError::MissingResource(format!(
                "Script fill_buffer não encontrado em {}",
                self.fill_script.display()
            )));
        }

        let mut command = Command::new(&self.fill_script);
        command.stdin(Stdio::null());
        if let Some(target) = args.target_hours {
            command.arg("--target-hours").arg(format!("{:.2}", target));
        }
        if args.dry_run {
            command.arg("--dry-run");
        }
        command.env("VVTV_DATA_DIR", &self.data_dir);
        command.env("VVTV_STORAGE_DIR", &self.bundle.vvtv.paths.storage_dir);
        command.env("VVTV_BASE_DIR", &self.bundle.vvtv.paths.base_dir);

        let output = command.output()?;
        if !output.status.success() {
            return Err(AppError::ScriptFailure {
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        Ok(BufferFillResult {
            status: "ok".to_string(),
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        })
    }

    fn health_check(&self) -> Result<Vec<HealthEntry>> {
        let mut results = Vec::new();
        results.push(self.check_path("vvtv.toml", &self.config_path));
        results.push(self.check_path("browser.toml", &self.browser_path));
        results.push(self.check_path("processor.toml", &self.processor_path));
        results.push(self.check_path("broadcaster.toml", &self.broadcaster_path));
        results.push(self.check_database("plans.sqlite", &self.plans_db));
        results.push(self.check_database("queue.sqlite", &self.queue_db));
        results.push(self.check_database("metrics.sqlite", &self.metrics_db));
        results.push(self.check_path("fill_buffer.sh", &self.fill_script));

        let vault_dir = PathBuf::from(&self.bundle.vvtv.paths.vault_dir);
        results.push(self.check_directory("vault", &vault_dir));
        results.push(self.check_directory("vault/keys", &vault_dir.join("keys")));
        results.push(self.check_directory("vault/manifests", &vault_dir.join("manifests")));
        results.push(self.check_directory("vault/snapshots", &vault_dir.join("snapshots")));

        Ok(results)
    }

    fn check_path(&self, name: &str, path: &Path) -> HealthEntry {
        if path.exists() {
            HealthEntry::ok(name, format!("{}", path.display()))
        } else {
            HealthEntry::error(name, format!("{path} ausente", path = path.display()))
        }
    }

    fn check_directory(&self, name: &str, path: &Path) -> HealthEntry {
        match fs::metadata(path) {
            Ok(meta) if meta.is_dir() => HealthEntry::ok(name, format!("{}", path.display())),
            Ok(_) => HealthEntry::warn(
                name,
                format!("{path} não é diretório", path = path.display()),
            ),
            Err(_) => HealthEntry::warn(
                name,
                format!("{path} não encontrado", path = path.display()),
            ),
        }
    }

    fn check_database(&self, name: &str, path: &Path) -> HealthEntry {
        if !path.exists() {
            return HealthEntry::warn(
                name,
                format!("{path} não encontrado", path = path.display()),
            );
        }
        match self.open_database(path) {
            Ok(conn) => {
                let pragma: rusqlite::Result<String> =
                    conn.query_row("PRAGMA integrity_check;", [], |row| row.get(0));
                match pragma {
                    Ok(result) if result.to_lowercase() == "ok" => {
                        HealthEntry::ok(name, "integridade ok".to_string())
                    }
                    Ok(result) => HealthEntry::warn(name, format!("integrity_check: {result}")),
                    Err(err) => HealthEntry::warn(name, format!("erro: {err}")),
                }
            }
            Err(err) => HealthEntry::error(name, format!("falha ao abrir: {err}")),
        }
    }

    fn open_database(&self, path: &Path) -> Result<Connection> {
        if !path.exists() {
            return Err(AppError::MissingResource(format!(
                "Banco de dados ausente: {}",
                path.display()
            )));
        }
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        Ok(conn)
    }

    fn plan_metrics(&self) -> Option<PlanMetrics> {
        let store = self.plan_store(true).ok()?;
        store.compute_metrics().ok()
    }

    fn queue_store(&self, read_only: bool) -> Result<PlayoutQueueStore> {
        if read_only && !self.queue_db.exists() {
            return Err(AppError::MissingResource(format!(
                "Banco de dados ausente: {}",
                self.queue_db.display()
            )));
        }
        let builder = PlayoutQueueStore::builder()
            .path(&self.queue_db)
            .read_only(read_only)
            .create_if_missing(!read_only);
        let store = builder.build()?;
        if !read_only {
            store.initialize()?;
        }
        Ok(store)
    }

    fn queue_metrics(&self) -> Result<QueueMetrics> {
        let store = self.queue_store(true)?;
        Ok(store.metrics()?)
    }

    fn queue_summary_map(&self) -> Result<HashMap<String, i64>> {
        let store = self.queue_store(true)?;
        let summary = store.summary()?;
        let map = summary
            .counts
            .into_iter()
            .map(|(status, count)| (status.to_string(), count))
            .collect();
        Ok(map)
    }

    fn queue_show(&self, args: &QueueShowArgs) -> Result<QueueList> {
        let store = self.queue_store(true)?;
        let status = match &args.status {
            Some(value) => Some(parse_queue_status(value)?),
            None => None,
        };
        let mut filter = QueueFilter::default();
        filter.status = status;
        filter.limit = Some(args.limit);
        let entries = store.list(&filter)?;
        let rows = entries.into_iter().map(QueueDisplayEntry::from).collect();
        Ok(QueueList { rows })
    }

    fn queue_summary(&self) -> Result<QueueSummaryOutput> {
        let store = self.queue_store(true)?;
        let summary = store.summary()?;
        let metrics = store.metrics()?;
        let counts = summary
            .counts
            .into_iter()
            .map(|(status, count)| (status.to_string(), count))
            .collect();
        Ok(QueueSummaryOutput {
            buffer_hours: summary.buffer_duration_hours,
            counts,
            played_last_hour: metrics.played_last_hour,
            failures_last_hour: metrics.failures_last_hour,
        })
    }

    fn queue_promote(&self, args: &QueuePromoteArgs) -> Result<AckMessage> {
        let store = self.queue_store(false)?;
        store.mark_priority(args.id, args.priority)?;
        Ok(AckMessage {
            message: format!(
                "Prioridade do item {} ajustada para {}",
                args.id, args.priority
            ),
        })
    }

    fn queue_remove(&self, args: &QueueRemoveArgs) -> Result<AckMessage> {
        let store = self.queue_store(false)?;
        store.remove(args.id)?;
        Ok(AckMessage {
            message: format!("Item {} removido da fila", args.id),
        })
    }

    fn queue_cleanup(&self, args: &QueueCleanupArgs) -> Result<AckMessage> {
        let store = self.queue_store(false)?;
        let hours = args.older_than_hours.max(1);
        let removed = store.cleanup_played(Duration::hours(hours))?;
        Ok(AckMessage {
            message: format!("Removidos {removed} itens reproduzidos há mais de {hours}h"),
        })
    }

    fn queue_backup(&self, args: &QueueBackupArgs) -> Result<AckMessage> {
        let store = self.queue_store(true)?;
        store.export_backup(&args.output)?;
        Ok(AckMessage {
            message: format!("Backup salvo em {}", args.output.display()),
        })
    }

    fn metrics_store(&self) -> Result<MetricsStore> {
        let store = MetricsStore::new(&self.metrics_db)?;
        store.initialize()?;
        Ok(store)
    }

    fn plan_store(&self, read_only: bool) -> Result<SqlitePlanStore> {
        if !self.plans_db.exists() {
            return Err(AppError::MissingResource(format!(
                "Banco de dados ausente: {}",
                self.plans_db.display()
            )));
        }
        let builder = SqlitePlanStore::builder()
            .path(&self.plans_db)
            .create_if_missing(false)
            .read_only(read_only);
        Ok(builder.build()?)
    }

    fn metrics_snapshot(&self) -> Result<Option<MetricsSnapshot>> {
        let store = self.metrics_store()?;
        let snapshot = store.latest()?;
        Ok(snapshot.map(|item| MetricsSnapshot {
            timestamp: Some(item.timestamp.to_rfc3339()),
            buffer_duration_h: Some(item.buffer_duration_h),
            queue_length: Some(item.queue_length),
            avg_cpu_load: Some(item.avg_cpu_load),
            avg_temp_c: Some(item.avg_temp_c),
            latency_s: Some(item.latency_s),
            played_last_hour: Some(item.played_last_hour),
            failures_last_hour: Some(item.failures_last_hour),
            stream_bitrate_mbps: Some(item.stream_bitrate_mbps),
            vmaf_live: Some(item.vmaf_live),
        }))
    }

    fn record_metrics(&self, queue_metrics: &QueueMetrics) -> Result<()> {
        let store = self.metrics_store()?;
        let record = MetricRecord {
            buffer_duration_h: queue_metrics.buffer_duration_hours,
            queue_length: queue_metrics.queue_length,
            played_last_hour: queue_metrics.played_last_hour,
            failures_last_hour: queue_metrics.failures_last_hour,
            avg_cpu_load: self.read_loadavg().unwrap_or_default(),
            avg_temp_c: self.read_temperature().unwrap_or_default(),
            latency_s: 0.0,
            stream_bitrate_mbps: 0.0,
            vmaf_live: 0.0,
        };
        store.record(&record)?;
        Ok(())
    }

    fn health_dashboard(&self, args: &HealthDashboardArgs) -> Result<AckMessage> {
        let store = self.metrics_store()?;
        let output = args
            .output
            .clone()
            .unwrap_or_else(|| PathBuf::from("/vvtv/monitor/dashboard.html"));
        let generator = DashboardGenerator::new(store, &output);
        generator.generate(args.points)?;
        Ok(AckMessage {
            message: format!("Dashboard atualizado em {}", output.display()),
        })
    }

    fn qa_smoke_test(&self, args: &QaSmokeArgs) -> Result<QaSmokeReport> {
        let mut browser_config = self.bundle.browser.clone();
        let failure_log = self
            .bundle
            .vvtv
            .resolve_path(&browser_config.observability.failure_log);
        browser_config.observability.failure_log = failure_log.to_string_lossy().to_string();
        let metrics_db_path = self
            .bundle
            .vvtv
            .resolve_path(&browser_config.observability.metrics_db);
        browser_config.observability.metrics_db = metrics_db_path.to_string_lossy().to_string();

        let cache_dir = self
            .bundle
            .vvtv
            .resolve_path(&self.bundle.vvtv.paths.cache_dir);
        let profiles_dir = cache_dir.join("browser_profiles");
        let profile_manager = ProfileManager::from_config(&browser_config, &profiles_dir)?;
        let launcher = BrowserLauncher::new(browser_config, profile_manager)?;
        let runner = BrowserQaRunner::new(launcher)?;

        let mut options = SmokeTestOptions::default();
        options.mode = args.mode.into();
        options.capture_screenshot = !args.no_screenshot;
        if let Some(dir) = &args.screenshot_dir {
            options.screenshot_dir = Some(dir.clone());
        }
        options.record_video = args.record_video;
        options.record_duration = std::time::Duration::from_secs(args.record_duration);
        if args.record_video {
            let mut recorder_config = SessionRecorderConfig::default();
            if let Some(dir) = &args.video_dir {
                recorder_config.output_dir = dir.clone();
            }
            if let Some(path) = &args.ffmpeg_path {
                recorder_config.ffmpeg_path = path.clone();
            }
            options.session_recorder = Some(Arc::new(SessionRecorder::new(recorder_config)));
        }

        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|err| AppError::InvalidArgument(err.to_string()))?;
        let result = runtime.block_on(runner.run_smoke(&args.url, options))?;
        Ok(QaSmokeReport { result })
    }

    fn qa_report(&self, args: &QaReportArgs) -> Result<QaReportResult> {
        let store = QaMetricsStore::new(&self.metrics_db);
        let output_path = args
            .output
            .clone()
            .unwrap_or_else(|| PathBuf::from("artifacts/qa/dashboard.html"));
        let written = store.generate_dashboard(&output_path)?;
        let stats = store.summarize()?;
        Ok(QaReportResult {
            output: written,
            stats,
        })
    }

    fn read_loadavg(&self) -> Option<f64> {
        let content = fs::read_to_string("/proc/loadavg").ok()?;
        let first = content.split_whitespace().next()?;
        first.parse::<f64>().ok().map(|value| value * 100.0)
    }

    fn read_temperature(&self) -> Option<f64> {
        let path = Path::new("/sys/class/thermal/thermal_zone0/temp");
        let content = fs::read_to_string(path).ok()?;
        let raw = content.trim().parse::<f64>().ok()?;
        Some(raw / 1000.0)
    }
}

#[derive(Debug, Serialize)]
pub struct StatusReport {
    pub node: NodeStatus,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub plan_counts: HashMap<String, usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_metrics: Option<PlanMetrics>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub queue_counts: HashMap<String, i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<MetricsSnapshot>,
}

impl DisplayFallback for StatusReport {
    fn display(&self) -> String {
        let mut lines = vec![format!(
            "Nodo: {} (role: {}, env: {})",
            self.node.node_name, self.node.node_role, self.node.environment
        )];
        if !self.plan_counts.is_empty() {
            lines.push("Planos:".to_string());
            for (status, count) in self.plan_counts.iter() {
                lines.push(format!("  - {status}: {count}"));
            }
            if let Some(metrics) = &self.plan_metrics {
                lines.push(format!("  - HD missing: {}", metrics.hd_missing));
                lines.push(format!("  - Score médio: {:.2}", metrics.average_score));
            }
        }
        if !self.queue_counts.is_empty() {
            lines.push("Fila:".to_string());
            for (status, count) in self.queue_counts.iter() {
                lines.push(format!("  - {status}: {count}"));
            }
        }
        if let Some(metrics) = &self.metrics {
            lines.push("Métricas:".to_string());
            if let Some(hours) = metrics.buffer_duration_h {
                lines.push(format!("  - Buffer: {:.2} h", hours));
            }
            if let Some(queue) = metrics.queue_length {
                lines.push(format!("  - Queue len: {queue}"));
            }
            if let Some(cpu) = metrics.avg_cpu_load {
                lines.push(format!("  - CPU: {:.1}%", cpu));
            }
            if let Some(temp) = metrics.avg_temp_c {
                lines.push(format!("  - Temp: {:.1} °C", temp));
            }
            if let Some(lat) = metrics.latency_s {
                lines.push(format!("  - Latency: {:.2} s", lat));
            }
            if let Some(played) = metrics.played_last_hour {
                lines.push(format!("  - Played (1h): {played}"));
            }
            if let Some(failed) = metrics.failures_last_hour {
                lines.push(format!("  - Failures (1h): {failed}"));
            }
            if let Some(bitrate) = metrics.stream_bitrate_mbps {
                lines.push(format!("  - Bitrate: {:.2} Mbps", bitrate));
            }
            if let Some(vmaf) = metrics.vmaf_live {
                lines.push(format!("  - VMAF live: {:.1}", vmaf));
            }
        } else {
            lines.push("Métricas: indisponíveis".to_string());
        }
        lines.join("\n")
    }
}

#[derive(Debug, Serialize)]
pub struct NodeStatus {
    pub node_name: String,
    pub node_role: String,
    pub environment: String,
}

#[derive(Debug, Serialize)]
pub struct MetricsSnapshot {
    pub timestamp: Option<String>,
    pub buffer_duration_h: Option<f64>,
    pub queue_length: Option<i64>,
    pub avg_cpu_load: Option<f64>,
    pub avg_temp_c: Option<f64>,
    pub latency_s: Option<f64>,
    pub played_last_hour: Option<i64>,
    pub failures_last_hour: Option<i64>,
    pub stream_bitrate_mbps: Option<f64>,
    pub vmaf_live: Option<f64>,
}

impl DisplayFallback for PlanList {
    fn display(&self) -> String {
        if self.rows.is_empty() {
            return "Nenhum plano encontrado".to_string();
        }
        let mut lines = Vec::new();
        for entry in &self.rows {
            let score = entry
                .curation_score
                .map(|v| format!("{v:.2}"))
                .unwrap_or_else(|| "-".to_string());
            let duration = entry
                .duration_est_s
                .map(|v| format!("{v}s"))
                .unwrap_or_else(|| "-".to_string());
            let mut extras = Vec::new();
            if entry.hd_missing {
                extras.push("hd_missing".to_string());
            }
            if let Some(updated) = &entry.updated_at {
                extras.push(format!("updated={updated}"));
            }
            let extras = if extras.is_empty() {
                String::new()
            } else {
                format!(" | {}", extras.join(", "))
            };
            lines.push(format!(
                "{} | {} | status={} | kind={} | score={} | dur={}{}",
                entry.plan_id,
                entry.title.as_deref().unwrap_or("<sem título>"),
                entry.status,
                entry.kind,
                score,
                duration,
                extras
            ));
        }
        lines.join("\n")
    }
}

impl DisplayFallback for PlanAuditReport {
    fn display(&self) -> String {
        if self.findings.is_empty() {
            return "Nenhuma inconformidade encontrada".to_string();
        }
        let mut lines = Vec::new();
        for finding in &self.findings {
            let mut line = format!(
                "{} | kind={} | status={} | age={:.1}h",
                finding.plan_id,
                finding.kind.to_string(),
                finding.status,
                finding.age_hours
            );
            if let Some(note) = &finding.note {
                line.push_str(&format!(" | {}", note));
            }
            lines.push(line);
        }
        lines.join("\n")
    }
}

impl DisplayFallback for PlanBlacklistResult {
    fn display(&self) -> String {
        match self {
            PlanBlacklistResult::List { entries } => {
                if entries.is_empty() {
                    "Blacklist vazia".to_string()
                } else {
                    let mut lines = Vec::new();
                    for entry in entries {
                        let mut line = entry.domain.clone();
                        if let Some(reason) = &entry.reason {
                            line.push_str(&format!(" — {}", reason));
                        }
                        lines.push(line);
                    }
                    lines.join("\n")
                }
            }
            PlanBlacklistResult::Ack { message } => message.clone(),
        }
    }
}

impl DisplayFallback for PlanImportResult {
    fn display(&self) -> String {
        format!("Importados {}/{} planos", self.imported, self.total)
    }
}

fn format_datetime(dt: Option<DateTime<Utc>>) -> Option<String> {
    dt.map(|value| value.to_rfc3339_opts(SecondsFormat::Secs, true))
}

fn parse_audit_kind(value: &str) -> Result<PlanAuditKind> {
    match value.to_lowercase().as_str() {
        "expired" => Ok(PlanAuditKind::Expired),
        "missing_license" | "license" => Ok(PlanAuditKind::MissingLicense),
        "hd_missing" | "hd" => Ok(PlanAuditKind::HdMissing),
        "stuck" => Ok(PlanAuditKind::Stuck),
        other => Err(AppError::InvalidArgument(format!(
            "tipo de auditoria inválido: {other}"
        ))),
    }
}

fn parse_plan_import(content: &str, overwrite: bool) -> Result<Vec<PlanImportRecord>> {
    if let Ok(records) = serde_json::from_str::<Vec<PlanImportRecord>>(content) {
        return Ok(records);
    }
    if let Ok(record) = serde_json::from_str::<PlanImportRecord>(content) {
        return Ok(vec![record]);
    }
    if let Ok(plans) = serde_json::from_str::<Vec<Plan>>(content) {
        return Ok(plans
            .into_iter()
            .map(|plan| PlanImportRecord { plan, overwrite })
            .collect());
    }
    if let Ok(plan) = serde_json::from_str::<Plan>(content) {
        return Ok(vec![PlanImportRecord { plan, overwrite }]);
    }
    Err(AppError::InvalidArgument(
        "Arquivo JSON inválido para import".to_string(),
    ))
}

fn parse_queue_status(value: &str) -> Result<QueueStatus> {
    QueueStatus::from_str(value)
        .map_err(|_| AppError::InvalidArgument(format!("status inválido: {value}")))
}

impl DisplayFallback for QueueList {
    fn display(&self) -> String {
        if self.rows.is_empty() {
            return "Fila vazia".to_string();
        }
        let mut lines = Vec::new();
        for entry in &self.rows {
            let duration = entry
                .duration_s
                .map(|v| format!("{v}s"))
                .unwrap_or_else(|| "-".to_string());
            let mut extras = Vec::new();
            if let Some(score) = entry.curation_score {
                extras.push(format!("score={:.2}", score));
            }
            if let Some(kind) = &entry.content_kind {
                extras.push(format!("kind={kind}"));
            }
            if let Some(origin) = &entry.node_origin {
                extras.push(format!("origin={origin}"));
            }
            let extra = if extras.is_empty() {
                String::new()
            } else {
                format!(" [{}]", extras.join(", "))
            };
            lines.push(format!(
                "#{id} plan={plan} status={status} priority={priority} dur={duration}{extra}",
                id = entry.id,
                plan = entry.plan_id,
                status = entry.status,
                priority = entry.priority,
                extra = extra,
            ));
        }
        lines.join("\n")
    }
}

impl DisplayFallback for QueueSummaryOutput {
    fn display(&self) -> String {
        let mut lines = vec![format!("Buffer disponível: {:.2} h", self.buffer_hours)];
        lines.push("Contagens por status:".to_string());
        for (status, count) in self.counts.iter() {
            lines.push(format!("  - {status}: {count}"));
        }
        lines.push(format!(
            "Última hora: reproduzidos={}, falhas={}",
            self.played_last_hour, self.failures_last_hour
        ));
        lines.join("\n")
    }
}

impl DisplayFallback for AckMessage {
    fn display(&self) -> String {
        self.message.clone()
    }
}

impl DisplayFallback for BufferFillResult {
    fn display(&self) -> String {
        if self.stdout.is_empty() {
            "Buffer fill executado".to_string()
        } else {
            self.stdout.clone()
        }
    }
}

impl DisplayFallback for Vec<HealthEntry> {
    fn display(&self) -> String {
        let mut lines = Vec::new();
        for entry in self {
            lines.push(format!(
                "[{status}] {name} — {detail}",
                status = entry.status,
                name = entry.name,
                detail = entry.detail
            ));
        }
        lines.join("\n")
    }
}

#[derive(Debug, Serialize)]
pub struct PlanList {
    pub rows: Vec<PlanEntry>,
}

#[derive(Debug, Serialize)]
pub struct PlanEntry {
    pub plan_id: String,
    pub title: Option<String>,
    pub status: String,
    pub duration_est_s: Option<i64>,
    pub curation_score: Option<f64>,
    pub updated_at: Option<String>,
    pub created_at: Option<String>,
    pub kind: String,
    pub hd_missing: bool,
}

#[derive(Debug, Serialize)]
pub struct PlanAuditReport {
    pub findings: Vec<PlanAuditFinding>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum PlanBlacklistResult {
    List { entries: Vec<PlanBlacklistEntry> },
    Ack { message: String },
}

#[derive(Debug, Serialize)]
pub struct PlanImportResult {
    pub imported: usize,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct QueueList {
    pub rows: Vec<QueueDisplayEntry>,
}

#[derive(Debug, Serialize)]
pub struct QueueDisplayEntry {
    pub id: i64,
    pub plan_id: String,
    pub status: String,
    pub duration_s: Option<i64>,
    pub priority: i64,
    pub curation_score: Option<f64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub node_origin: Option<String>,
    pub content_kind: Option<String>,
}

impl From<QueueStoreEntry> for QueueDisplayEntry {
    fn from(entry: QueueStoreEntry) -> Self {
        Self {
            id: entry.id,
            plan_id: entry.plan_id,
            status: entry.status.to_string(),
            duration_s: entry.duration_s,
            priority: entry.priority,
            curation_score: entry.curation_score,
            created_at: format_datetime(entry.created_at),
            updated_at: format_datetime(entry.updated_at),
            node_origin: entry.node_origin,
            content_kind: entry.content_kind,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct QueueSummaryOutput {
    pub buffer_hours: f64,
    pub counts: HashMap<String, i64>,
    pub played_last_hour: i64,
    pub failures_last_hour: i64,
}

#[derive(Debug, Serialize)]
pub struct AckMessage {
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct BufferFillResult {
    pub status: String,
    pub stdout: String,
}

#[derive(Debug, Serialize)]
pub struct HealthEntry {
    pub name: String,
    pub status: CheckStatus,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub enum CheckStatus {
    #[serde(rename = "ok")]
    Ok,
    #[serde(rename = "warn")]
    Warn,
    #[serde(rename = "error")]
    Error,
}

impl fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            CheckStatus::Ok => "OK",
            CheckStatus::Warn => "WARN",
            CheckStatus::Error => "ERROR",
        };
        write!(f, "{}", label)
    }
}

impl HealthEntry {
    fn ok(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Ok,
            detail: detail.into(),
        }
    }

    fn warn(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Warn,
            detail: detail.into(),
        }
    }

    fn error(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Error,
            detail: detail.into(),
        }
    }
}

impl DisplayFallback for HealthEntry {
    fn display(&self) -> String {
        format!(
            "[{status}] {name} — {detail}",
            status = self.status,
            name = self.name,
            detail = self.detail
        )
    }
}

impl DisplayFallback for QaSmokeReport {
    fn display(&self) -> String {
        let mut lines = vec![
            format!("URL: {}", self.result.url),
            format!("Sucesso: {}", self.result.success),
            format!("Duração (ms): {}", self.result.duration_ms),
            format!("Tentativas: {}", self.result.attempts),
            format!(
                "HD success {}/{}",
                self.result.metrics.hd_success, self.result.metrics.hd_attempts
            ),
            format!(
                "PBD success rate: {:.1}%",
                self.result.metrics.pbd_success_rate()
            ),
        ];
        if let Some(path) = &self.result.screenshot_path {
            lines.push(format!("Screenshot: {}", path.display()));
        }
        if let Some(path) = &self.result.video_path {
            lines.push(format!("Vídeo: {}", path.display()));
        }
        if !self.result.warnings.is_empty() {
            lines.push(format!("Avisos: {}", self.result.warnings.join("; ")));
        }
        lines.join("\n")
    }
}

impl DisplayFallback for QaReportResult {
    fn display(&self) -> String {
        format!(
            "Dashboard: {}\nTotal runs: {}\nSuccess rate: {:.1}%\nProxy rotations: {}\nBot detections: {}",
            self.output.display(),
            self.stats.total_runs,
            self.stats.pbd_success_rate,
            self.stats.proxy_rotations,
            self.stats.bot_detections
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use vvtv_core::BrowserMetrics;

    fn prepare_test_context() -> Result<(TempDir, AppContext)> {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        let configs_dir = root.join("configs");
        fs::create_dir_all(&configs_dir).unwrap();
        fs::copy("../configs/vvtv.toml", configs_dir.join("vvtv.toml")).unwrap();
        fs::copy("../configs/browser.toml", configs_dir.join("browser.toml")).unwrap();
        fs::copy(
            "../configs/processor.toml",
            configs_dir.join("processor.toml"),
        )
        .unwrap();
        fs::copy(
            "../configs/broadcaster.toml",
            configs_dir.join("broadcaster.toml"),
        )
        .unwrap();

        let data_dir = root.join("data");
        fs::create_dir_all(&data_dir).unwrap();
        let plans_db = data_dir.join("plans.sqlite");
        let queue_db = data_dir.join("queue.sqlite");
        let metrics_db = data_dir.join("metrics.sqlite");

        let conn = Connection::open(&plans_db).unwrap();
        conn.execute_batch(&fs::read_to_string("../sql/plans.sql").unwrap())
            .unwrap();
        conn.execute(
            "INSERT INTO plans(plan_id, kind, title, status, duration_est_s, curation_score) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params!["plan-1", "video", "Video", "planned", 3600, 0.9],
        )
        .unwrap();

        let conn_queue = Connection::open(&queue_db).unwrap();
        conn_queue
            .execute_batch(&fs::read_to_string("../sql/queue.sql").unwrap())
            .unwrap();
        conn_queue.execute(
            "INSERT INTO playout_queue(plan_id, asset_path, duration_s, status, curation_score) VALUES (?1, ?2, ?3, ?4, ?5)",
            params!["plan-1", "asset.mp4", 3600, "queued", 0.9],
        )
        .unwrap();

        let conn_metrics = Connection::open(&metrics_db).unwrap();
        conn_metrics
            .execute_batch(&fs::read_to_string("../sql/metrics.sql").unwrap())
            .unwrap();
        conn_metrics.execute(
            "INSERT INTO metrics(buffer_duration_h, queue_length, played_last_hour, failures_last_hour, avg_cpu_load, avg_temp_c, latency_s, stream_bitrate_mbps, vmaf_live) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![4.0, 3, 2, 0, 55.0, 48.5, 6.2, 2.8, 92.0],
        )
        .unwrap();

        let scripts_dir = root.join("scripts/system");
        fs::create_dir_all(&scripts_dir).unwrap();
        fs::copy(
            "../scripts/system/fill_buffer.sh",
            scripts_dir.join("fill_buffer.sh"),
        )
        .unwrap();

        let cli = Cli {
            config: configs_dir.join("vvtv.toml"),
            browser_config: None,
            processor_config: None,
            broadcaster_config: None,
            data_dir: Some(data_dir.clone()),
            scripts_dir: Some(scripts_dir.clone()),
            plans_db: Some(plans_db.clone()),
            queue_db: Some(queue_db.clone()),
            metrics_db: Some(metrics_db.clone()),
            fill_script: Some(scripts_dir.join("fill_buffer.sh")),
            token: None,
            format: OutputFormat::Json,
            command: Commands::Status,
        };

        let context = AppContext::new(&cli)?;
        Ok((temp, context))
    }

    #[test]
    fn status_report_collects_metrics() {
        let (_temp, context) = prepare_test_context().unwrap();
        let status = context.gather_status().unwrap();
        assert_eq!(status.node.node_name, "vvtv-primary");
        assert!(status.plan_counts.get("planned").is_some());
        assert!(status.queue_counts.get("queued").is_some());
        assert!(status.metrics.is_some());
    }

    #[test]
    fn plan_listing_returns_entries() {
        let (_temp, context) = prepare_test_context().unwrap();
        let list = context
            .plan_list(&PlanListArgs {
                status: None,
                limit: 5,
            })
            .unwrap();
        assert_eq!(list.rows.len(), 1);
        assert_eq!(list.rows[0].plan_id, "plan-1");
    }

    #[test]
    fn qa_smoke_report_display_renders_paths_and_metrics() {
        let mut metrics = BrowserMetrics::default();
        metrics.hd_attempts = 1;
        metrics.hd_success = 1;
        metrics.proxy_rotations = 2;
        let report = QaSmokeReport {
            result: SmokeTestResult {
                url: "https://example.com".into(),
                success: true,
                capture: None,
                duration_ms: 900,
                warnings: vec!["placeholder video".into()],
                metrics,
                screenshot_path: Some(PathBuf::from("/tmp/screenshot.png")),
                video_path: Some(PathBuf::from("/tmp/video.mp4")),
                attempts: 1,
            },
        };
        let display = report.display();
        assert!(display.contains("URL: https://example.com"));
        assert!(display.contains("Screenshot: /tmp/screenshot.png"));
        assert!(display.contains("Avisos"));
        assert!(display.contains("PBD success rate: 100.0%"));
    }

    #[test]
    fn qa_report_result_display_includes_summary() {
        let stats = QaStatistics {
            total_runs: 4,
            success_count: 3,
            failure_count: 1,
            pbd_success_rate: 75.0,
            avg_duration_ms: 1200.0,
            proxy_rotations: 5,
            bot_detections: 2,
            last_run: None,
        };
        let report = QaReportResult {
            output: PathBuf::from("/tmp/dashboard.html"),
            stats,
        };
        let display = report.display();
        assert!(display.contains("Dashboard: /tmp/dashboard.html"));
        assert!(display.contains("Total runs: 4"));
        assert!(display.contains("Proxy rotations: 5"));
        assert!(display.contains("Bot detections: 2"));
    }
}
