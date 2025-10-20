use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::{Args, Parser, Subcommand, ValueEnum};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::Serialize;
use thiserror::Error;
use vvtv_core::{
    load_broadcaster_config, load_browser_config, load_processor_config, load_vvtv_config,
    ConfigBundle,
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
    #[error("authentication failed")]
    Authentication,
    #[error("required resource missing: {0}")]
    MissingResource(String),
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
}

#[derive(Subcommand, Debug)]
pub enum PlanCommands {
    /// Lista planos registrados no banco
    List(PlanListArgs),
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

#[derive(Subcommand, Debug)]
pub enum QueueCommands {
    /// Lista itens da fila de playout
    Show(QueueShowArgs),
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

#[derive(Subcommand, Debug)]
pub enum HealthCommands {
    /// Executa checagens básicas
    Check,
}

pub fn run(cli: Cli) -> Result<()> {
    enforce_token(&cli)?;
    let context = AppContext::new(&cli)?;

    match &cli.command {
        Commands::Status => {
            let status = context.gather_status()?;
            render(&status, cli.format)?;
        }
        Commands::Plan(PlanCommands::List(args)) => {
            let plans = context.plan_list(args)?;
            render(&plans, cli.format)?;
        }
        Commands::Queue(QueueCommands::Show(args)) => {
            let queue = context.queue_show(args)?;
            render(&queue, cli.format)?;
        }
        Commands::Buffer(BufferCommands::Fill(args)) => {
            let result = context.buffer_fill(args)?;
            render(&result, cli.format)?;
        }
        Commands::Health(HealthCommands::Check) => {
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

        let plan_counts = self.plan_counts().unwrap_or_default();
        let queue_counts = self.queue_counts().unwrap_or_default();
        let metrics = self.metrics_snapshot()?;

        Ok(StatusReport {
            node,
            plan_counts,
            queue_counts,
            metrics,
        })
    }

    fn plan_list(&self, args: &PlanListArgs) -> Result<PlanList> {
        let conn = self.open_database(&self.plans_db)?;
        let mut stmt = conn.prepare(
            "SELECT plan_id, title, status, duration_est_s, curation_score, updated_at, created_at \
             FROM plans \
             WHERE (?1 IS NULL OR status = ?1) \
             ORDER BY updated_at DESC NULLS LAST, created_at DESC \
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map((args.status.as_ref(), args.limit as i64), |row| {
                Ok(PlanEntry {
                    plan_id: row.get(0)?,
                    title: row.get(1)?,
                    status: row.get(2)?,
                    duration_est_s: row.get::<_, Option<i64>>(3)?,
                    curation_score: row.get::<_, Option<f64>>(4)?,
                    updated_at: row.get::<_, Option<String>>(5)?,
                    created_at: row.get::<_, Option<String>>(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(PlanList { rows })
    }

    fn queue_show(&self, args: &QueueShowArgs) -> Result<QueueList> {
        let conn = self.open_database(&self.queue_db)?;
        let mut stmt = conn.prepare(
            "SELECT id, plan_id, status, duration_s, priority, created_at, updated_at \
             FROM playout_queue \
             WHERE (?1 IS NULL OR status = ?1) \
             ORDER BY priority DESC, created_at ASC \
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map((args.status.as_ref(), args.limit as i64), |row| {
                Ok(QueueEntry {
                    id: row.get(0)?,
                    plan_id: row.get(1)?,
                    status: row.get(2)?,
                    duration_s: row.get::<_, Option<i64>>(3)?,
                    priority: row.get::<_, Option<i64>>(4)?,
                    created_at: row.get::<_, Option<String>>(5)?,
                    updated_at: row.get::<_, Option<String>>(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(QueueList { rows })
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

    fn plan_counts(&self) -> Option<HashMap<String, i64>> {
        let conn = self.open_database(&self.plans_db).ok()?;
        let mut stmt = conn
            .prepare("SELECT status, COUNT(*) FROM plans GROUP BY status")
            .ok()?;
        let mut map = HashMap::new();
        for row in stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .ok()?
        {
            if let Ok((status, count)) = row {
                map.insert(status, count);
            }
        }
        Some(map)
    }

    fn queue_counts(&self) -> Option<HashMap<String, i64>> {
        let conn = self.open_database(&self.queue_db).ok()?;
        let mut stmt = conn
            .prepare("SELECT status, COUNT(*) FROM playout_queue GROUP BY status")
            .ok()?;
        let mut map = HashMap::new();
        for row in stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .ok()?
        {
            if let Ok((status, count)) = row {
                map.insert(status, count);
            }
        }
        Some(map)
    }

    fn metrics_snapshot(&self) -> Result<Option<MetricsSnapshot>> {
        if !self.metrics_db.exists() {
            return Ok(None);
        }
        let conn = self.open_database(&self.metrics_db)?;
        let mut stmt = conn.prepare(
            "SELECT ts, buffer_duration_h, queue_length, avg_cpu_load, avg_temp_c, latency_s \
             FROM metrics ORDER BY ts DESC LIMIT 1",
        )?;
        let snapshot = stmt
            .query_row([], |row| {
                Ok(MetricsSnapshot {
                    timestamp: row.get::<_, Option<String>>(0)?,
                    buffer_duration_h: row.get::<_, Option<f64>>(1)?,
                    queue_length: row.get::<_, Option<i64>>(2)?,
                    avg_cpu_load: row.get::<_, Option<f64>>(3)?,
                    avg_temp_c: row.get::<_, Option<f64>>(4)?,
                    latency_s: row.get::<_, Option<f64>>(5)?,
                })
            })
            .optional()?;
        Ok(snapshot)
    }
}

#[derive(Debug, Serialize)]
pub struct StatusReport {
    pub node: NodeStatus,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub plan_counts: HashMap<String, i64>,
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
            lines.push(format!(
                "{} | {} | status={} | score={} | dur={}",
                entry.plan_id,
                entry.title.as_deref().unwrap_or("<sem título>"),
                entry.status,
                score,
                duration
            ));
        }
        lines.join("\n")
    }
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
            let priority = entry
                .priority
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".to_string());
            lines.push(format!(
                "#{id} plan={plan} status={status} priority={priority} dur={duration}",
                id = entry.id,
                plan = entry.plan_id,
                status = entry.status,
                priority = priority,
            ));
        }
        lines.join("\n")
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
}

#[derive(Debug, Serialize)]
pub struct QueueList {
    pub rows: Vec<QueueEntry>,
}

#[derive(Debug, Serialize)]
pub struct QueueEntry {
    pub id: i64,
    pub plan_id: String,
    pub status: String,
    pub duration_s: Option<i64>,
    pub priority: Option<i64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use tempfile::TempDir;

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
            "INSERT INTO metrics(buffer_duration_h, queue_length, avg_cpu_load, avg_temp_c, latency_s) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![4.0, 3, 55.0, 48.5, 6.2],
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
}
