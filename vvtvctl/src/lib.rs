#![allow(
    clippy::result_large_err,
    clippy::field_reassign_with_default,
    clippy::to_string_in_format_args,
    clippy::vec_init_then_push
)]

mod commands;

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::sync::Arc;

use chrono::{DateTime, Duration, SecondsFormat, Utc};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use commands::{discover::DiscoverArgs, incident::IncidentReportArgs};
use rusqlite::{Connection, OpenFlags};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;
use tokio::runtime::Builder;
use tracing_subscriber::{fmt as tracing_fmt, EnvFilter};
use vvtv_core::{
    load_broadcaster_config, load_browser_config, load_processor_config, load_vvtv_config,
    AdaptiveProgrammer, AdaptiveReport, AudienceReport, AudienceStore, AudienceStoreBuilder,
    BrowserError, BrowserLauncher, BrowserPbdRunner, BrowserQaRunner, BrowserSearchSessionFactory,
    BusinessLogic, BusinessLogicError, ConfigBundle, ContentSearcher, DashboardArtifacts,
    DashboardError, DashboardGenerator, DiscoveryConfig, DiscoveryLoop, DiscoveryPbd,
    DiscoveryPlanStore, DiscoveryStats, DispatchAction, DispatchStatus, EconomyError, EconomyEvent,
    EconomyEventType, EconomyStore, EconomyStoreBuilder, EconomySummary, IncidentDispatch,
    IncidentError, IncidentHistoryWriter, IncidentNotifier, IncidentReport, IncidentSeverity,
    LedgerExport, MetricRecord, MetricsStore, MicroSpotContract, MicroSpotInjection,
    MicroSpotManager, MonetizationDashboard, MonitorError, NewEconomyEvent, NewViewerSession, Plan,
    PlanAuditFinding, PlanAuditKind, PlanBlacklistEntry, PlanImportRecord, PlanMetrics, PlanStatus,
    PlayBeforeDownload, PlayoutQueueStore, ProfileManager, QaMetricsStore, QaStatistics,
    QueueEntry as QueueStoreEntry, QueueError, QueueFilter, QueueMetrics, QueueStatus,
    SearchConfig, SearchEngine, SearchSessionFactory, SessionRecorder, SessionRecorderConfig,
    SmokeMode, SmokeTestOptions, SmokeTestResult, SqlitePlanStore, ViewerSession,
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
    #[error("economy error: {0}")]
    Economy(#[from] vvtv_core::EconomyError),
    #[error("audience error: {0}")]
    Audience(#[from] vvtv_core::AudienceError),
    #[error("dashboard error: {0}")]
    Dashboard(#[from] DashboardError),
    #[error("adaptive error: {0}")]
    Adaptive(#[from] vvtv_core::AdaptiveError),
    #[error("spots error: {0}")]
    Spots(#[from] vvtv_core::SpotsError),
    #[error("incident error: {0}")]
    Incident(#[from] IncidentError),
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
    /// Caminho alternativo para business_logic.yaml
    #[arg(long)]
    pub business_logic: Option<PathBuf>,
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
    /// Caminho alternativo para economy.sqlite
    #[arg(long)]
    pub economy_db: Option<PathBuf>,
    /// Caminho alternativo para viewers.sqlite
    #[arg(long)]
    pub viewers_db: Option<PathBuf>,
    /// Diretório para relatórios de monetização
    #[arg(long)]
    pub reports_dir: Option<PathBuf>,
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
    /// Executa descoberta autônoma de conteúdo
    Discover(DiscoverArgs),
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
    /// Gera scripts de autocompletar para shells suportados
    Completions {
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Operações relacionadas ao cartão de negócios
    #[command(name = "business-logic")]
    #[command(subcommand)]
    BusinessLogic(BusinessLogicCommands),
    /// Operações de monetização e analytics
    #[command(subcommand)]
    Monetization(MonetizationCommands),
    /// Comunicação e registro de incidentes
    #[command(subcommand)]
    Incident(IncidentCommands),
}

#[derive(Args, Debug)]
pub struct BusinessLogicArgs {
    /// Caminho alternativo para business_logic.yaml
    #[arg(long)]
    pub path: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum BusinessLogicCommands {
    /// Exibe o cartão de negócios carregado
    Show(BusinessLogicArgs),
    /// Valida o cartão de negócios
    Validate(BusinessLogicArgs),
    /// Recarrega o cartão de negócios a partir do disco
    Reload(BusinessLogicArgs),
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

#[derive(Debug, Clone, Serialize)]
pub struct LedgerRecordResult {
    pub event: EconomyEvent,
}

#[derive(Debug, Clone, Serialize)]
pub struct LedgerSummaryResult {
    pub summary: EconomySummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct LedgerExportResult {
    pub export: LedgerExport,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudienceRecordResult {
    pub session: ViewerSession,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudienceMetricsResult {
    pub report: AudienceReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudienceHeatmapResult {
    pub output: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudienceReportResultView {
    pub path: PathBuf,
    pub report: AudienceReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdaptiveReportView {
    pub report: AdaptiveReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpotListResult {
    pub contracts: Vec<MicroSpotContract>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpotActionResult {
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpotInjectionResult {
    pub injections: Vec<MicroSpotInjection>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardResultView {
    pub artifacts: DashboardArtifacts,
}

#[derive(Debug, Clone, Serialize)]
pub struct IncidentReportResultView {
    pub incident_id: String,
    pub severity: String,
    pub markdown_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification: Option<IncidentDispatchView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IncidentDispatchView {
    pub subject: String,
    pub severity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub actions: Vec<IncidentDispatchActionView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IncidentDispatchActionView {
    pub channel: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl From<IncidentDispatch> for IncidentDispatchView {
    fn from(dispatch: IncidentDispatch) -> Self {
        let actions = dispatch
            .actions
            .iter()
            .map(IncidentDispatchActionView::from)
            .collect();
        Self {
            subject: dispatch.subject,
            severity: dispatch.severity.badge().to_string(),
            message: Some(dispatch.message),
            actions,
        }
    }
}

impl From<&DispatchAction> for IncidentDispatchActionView {
    fn from(action: &DispatchAction) -> Self {
        match &action.status {
            DispatchStatus::Executed { detail } => Self {
                channel: action.channel.to_string(),
                status: "executed".to_string(),
                detail: detail.clone(),
            },
            DispatchStatus::Skipped { reason } => Self {
                channel: action.channel.to_string(),
                status: "skipped".to_string(),
                detail: Some(reason.clone()),
            },
            DispatchStatus::Failed { reason } => Self {
                channel: action.channel.to_string(),
                status: "failed".to_string(),
                detail: Some(reason.clone()),
            },
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum MonetizationCommands {
    /// Operações no ledger econômico
    #[command(subcommand)]
    Ledger(LedgerCommands),
    /// Registrar e consultar métricas de audiência
    #[command(subcommand)]
    Audience(AudienceCommands),
    /// Atualiza scores de curadoria com base em retenção e receita
    Adaptive,
    /// Gerencia micro-spots e slots premium
    #[command(subcommand)]
    Spots(SpotCommands),
    /// Gera dashboard HTML/JSON de monetização
    Dashboard(DashboardArgs),
}

#[derive(Subcommand, Debug)]
pub enum IncidentCommands {
    /// Gera relatório de incidente e dispara comunicações configuradas
    Report(IncidentReportArgs),
}

#[derive(Subcommand, Debug)]
pub enum LedgerCommands {
    /// Registra um evento financeiro
    Record(LedgerRecordArgs),
    /// Exibe resumo financeiro
    Summary(LedgerSummaryArgs),
    /// Exporta CSV e .logline do período
    Export(LedgerExportArgs),
}

#[derive(Args, Debug)]
pub struct LedgerRecordArgs {
    /// Tipo do evento (view, click, slot_sell, affiliate, cost, payout)
    pub event_type: String,
    /// Valor em euros
    pub value_eur: f64,
    /// Origem do evento (viewer, campanha...)
    #[arg(long)]
    pub source: String,
    /// Contexto associado (plan_id, campanha)
    #[arg(long)]
    pub context: String,
    /// Observações opcionais
    #[arg(long)]
    pub notes: Option<String>,
}

#[derive(Args, Debug)]
pub struct LedgerSummaryArgs {
    /// Intervalo em horas (padrão 24h)
    #[arg(long, default_value_t = 24)]
    pub hours: i64,
}

#[derive(Args, Debug)]
pub struct LedgerExportArgs {
    /// Início do período (RFC3339). Padrão: agora - 7 dias.
    #[arg(long)]
    pub start: Option<String>,
    /// Fim do período (RFC3339). Padrão: agora.
    #[arg(long)]
    pub end: Option<String>,
    /// Diretório de saída
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum AudienceCommands {
    /// Registra sessão de audiência
    Record(AudienceRecordArgs),
    /// Resumo de métricas agregadas
    Metrics(AudienceMetricsArgs),
    /// Gera heatmap geográfico
    Heatmap(AudienceHeatmapArgs),
    /// Exporta relatório JSON
    Report(AudienceReportArgs),
}

#[derive(Args, Debug)]
pub struct AudienceRecordArgs {
    /// Identificador da sessão
    pub session_id: String,
    /// Região (EU, NA, SA, ...)
    #[arg(long)]
    pub region: String,
    /// Dispositivo
    #[arg(long)]
    pub device: String,
    /// Duração em minutos
    #[arg(long, default_value_t = 10.0)]
    pub duration_minutes: f64,
    /// Banda média (Mbps)
    #[arg(long)]
    pub bandwidth_mbps: Option<f64>,
    /// Score de engajamento (0-1)
    #[arg(long)]
    pub engagement: Option<f64>,
}

#[derive(Args, Debug)]
pub struct AudienceMetricsArgs {
    /// Intervalo em horas (padrão 24h)
    #[arg(long, default_value_t = 24)]
    pub hours: i64,
}

#[derive(Args, Debug)]
pub struct AudienceHeatmapArgs {
    /// Caminho de saída para o PNG
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct AudienceReportArgs {
    /// Caminho do relatório JSON
    #[arg(long)]
    pub output: Option<PathBuf>,
    /// Intervalo em horas (padrão 24h)
    #[arg(long, default_value_t = 24)]
    pub hours: i64,
}

#[derive(Subcommand, Debug)]
pub enum SpotCommands {
    /// Lista contratos de micro-spot
    List,
    /// Carrega contrato .lll
    Load(SpotLoadArgs),
    /// Ativa contrato
    Activate(SpotToggleArgs),
    /// Desativa contrato
    Deactivate(SpotToggleArgs),
    /// Injeta microspots devidos
    Inject,
}

#[derive(Args, Debug)]
pub struct SpotLoadArgs {
    /// Caminho do arquivo .lll
    pub path: PathBuf,
}

#[derive(Args, Debug)]
pub struct SpotToggleArgs {
    /// ID do contrato
    pub id: String,
}

#[derive(Args, Debug)]
pub struct DashboardArgs {
    /// Diretório de saída (padrão monitor/)
    #[arg(long)]
    pub output: Option<PathBuf>,
}

pub fn run(cli: Cli) -> Result<()> {
    enforce_token(&cli)?;
    let context = AppContext::new(&cli)?;

    match &cli.command {
        Commands::Status => {
            let status = context.gather_status()?;
            render(&status, cli.format)?;
        }
        Commands::Discover(args) => {
            let report = context.discovery_run(args)?;
            render(&report, cli.format)?;
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
        Commands::Completions { shell } => {
            output_completions(*shell)?;
        }
        Commands::BusinessLogic(command) => match command {
            BusinessLogicCommands::Show(args) => {
                let view = context.business_logic_show(&args.path)?;
                render(&view, cli.format)?;
            }
            BusinessLogicCommands::Validate(args) => {
                let status = context.business_logic_validate(&args.path)?;
                render(&status, cli.format)?;
            }
            BusinessLogicCommands::Reload(args) => {
                let status = context.business_logic_reload(&args.path)?;
                render(&status, cli.format)?;
            }
        },
        Commands::Monetization(command) => match command {
            MonetizationCommands::Ledger(sub) => match sub {
                LedgerCommands::Record(args) => {
                    let result = context.ledger_record(args)?;
                    render(&result, cli.format)?;
                }
                LedgerCommands::Summary(args) => {
                    let result = context.ledger_summary(args)?;
                    render(&result, cli.format)?;
                }
                LedgerCommands::Export(args) => {
                    let result = context.ledger_export(args)?;
                    render(&result, cli.format)?;
                }
            },
            MonetizationCommands::Audience(sub) => match sub {
                AudienceCommands::Record(args) => {
                    let result = context.audience_record(args)?;
                    render(&result, cli.format)?;
                }
                AudienceCommands::Metrics(args) => {
                    let result = context.audience_metrics(args)?;
                    render(&result, cli.format)?;
                }
                AudienceCommands::Heatmap(args) => {
                    let result = context.audience_heatmap(args)?;
                    render(&result, cli.format)?;
                }
                AudienceCommands::Report(args) => {
                    let result = context.audience_report(args)?;
                    render(&result, cli.format)?;
                }
            },
            MonetizationCommands::Adaptive => {
                let result = context.run_adaptive()?;
                render(&result, cli.format)?;
            }
            MonetizationCommands::Spots(sub) => match sub {
                SpotCommands::List => {
                    let result = context.spots_list()?;
                    render(&result, cli.format)?;
                }
                SpotCommands::Load(args) => {
                    let result = context.spots_load(args)?;
                    render(&result, cli.format)?;
                }
                SpotCommands::Activate(args) => {
                    let result = context.spots_toggle(args, true)?;
                    render(&result, cli.format)?;
                }
                SpotCommands::Deactivate(args) => {
                    let result = context.spots_toggle(args, false)?;
                    render(&result, cli.format)?;
                }
                SpotCommands::Inject => {
                    let result = context.spots_inject()?;
                    render(&result, cli.format)?;
                }
            },
            MonetizationCommands::Dashboard(args) => {
                let result = context.generate_dashboard(args)?;
                render(&result, cli.format)?;
            }
        },
        Commands::Incident(command) => match command {
            IncidentCommands::Report(args) => {
                let result = context.incident_report(args)?;
                render(&result, cli.format)?;
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

fn output_completions(shell: Shell) -> Result<()> {
    let mut command = Cli::command();
    generate(shell, &mut command, "vvtvctl", &mut std::io::stdout());
    Ok(())
}

fn init_discovery_tracing(enable: bool) {
    if !enable {
        return;
    }
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(
            "info,vvtv_core::browser::discovery_loop=debug,vvtv_core::browser::searcher=debug",
        )
    });
    let _ = tracing_fmt().with_env_filter(filter).try_init();
}

#[derive(Debug, Serialize)]
pub struct BusinessLogicView {
    pub path: String,
    pub policy_version: String,
    pub environment: String,
    pub selection_method: String,
    pub temperature: f64,
    pub top_k: usize,
    pub bias: f64,
}

#[derive(Debug, Serialize)]
pub struct BusinessLogicValidation {
    pub path: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct BusinessLogicReloadResult {
    pub path: String,
    pub policy_version: String,
    pub reloaded: bool,
}

fn parse_event_type(value: &str) -> Result<EconomyEventType> {
    EconomyEventType::from_str(value).map_err(|err| match err {
        EconomyError::InvalidEventType(_) => {
            AppError::InvalidArgument(format!("tipo de evento inválido: {value}"))
        }
        other => AppError::Economy(other),
    })
}

fn parse_datetime(value: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|err| AppError::InvalidArgument(format!("data/hora inválida '{value}': {err}")))
}

trait DisplayFallback {
    fn display(&self) -> String;
}

impl DisplayFallback for BusinessLogicView {
    fn display(&self) -> String {
        format!(
            "business_logic: {path}\n policy_version: {version}\n env: {env}\n method: {method}\n temperature: {temp:.2}\n top_k: {top}\n bias: {bias:.3}",
            path = self.path,
            version = self.policy_version,
            env = self.environment,
            method = self.selection_method,
            temp = self.temperature,
            top = self.top_k,
            bias = self.bias
        )
    }
}

impl DisplayFallback for BusinessLogicValidation {
    fn display(&self) -> String {
        format!("{}: {}", self.path, self.status)
    }
}

impl DisplayFallback for BusinessLogicReloadResult {
    fn display(&self) -> String {
        format!(
            "reloaded {path} (policy_version={version})",
            path = self.path,
            version = self.policy_version
        )
    }
}

#[derive(Debug)]
struct AppContext {
    bundle: ConfigBundle,
    config_path: PathBuf,
    browser_path: PathBuf,
    processor_path: PathBuf,
    broadcaster_path: PathBuf,
    business_logic_path: PathBuf,
    data_dir: PathBuf,
    plans_db: PathBuf,
    queue_db: PathBuf,
    metrics_db: PathBuf,
    economy_db: PathBuf,
    viewers_db: PathBuf,
    reports_dir: PathBuf,
    scripts_dir: PathBuf,
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

        let business_logic_path = cli
            .business_logic
            .clone()
            .or_else(|| {
                bundle
                    .vvtv
                    .paths
                    .business_logic
                    .as_ref()
                    .map(|value| PathBuf::from(value))
            })
            .map(|candidate| {
                if candidate.is_absolute() {
                    candidate
                } else {
                    PathBuf::from(&bundle.vvtv.paths.base_dir).join(candidate)
                }
            })
            .unwrap_or_else(|| {
                PathBuf::from(&bundle.vvtv.paths.base_dir)
                    .join("business_logic/business_logic.yaml")
            });

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
        let economy_db = cli
            .economy_db
            .clone()
            .unwrap_or_else(|| data_dir.join("economy.sqlite"));
        let viewers_db = cli
            .viewers_db
            .clone()
            .unwrap_or_else(|| data_dir.join("viewers.sqlite"));
        let reports_dir = cli.reports_dir.clone().unwrap_or_else(|| {
            let base = PathBuf::from(&bundle.vvtv.paths.base_dir);
            base.join("monitor")
        });
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
            business_logic_path,
            data_dir,
            plans_db,
            queue_db,
            metrics_db,
            economy_db,
            viewers_db,
            reports_dir,
            scripts_dir,
            fill_script,
        })
    }

    fn resolve_business_logic_path(&self, override_path: &Option<PathBuf>) -> PathBuf {
        match override_path {
            Some(candidate) if candidate.is_absolute() => candidate.clone(),
            Some(candidate) => PathBuf::from(&self.bundle.vvtv.paths.base_dir).join(candidate),
            None => self.business_logic_path.clone(),
        }
    }

    fn load_business_logic(&self, path: &Path) -> Result<BusinessLogic> {
        BusinessLogic::load_from_file(path).map_err(|err| match err {
            BusinessLogicError::Io(inner) => AppError::Io(inner),
            BusinessLogicError::Yaml(inner) => AppError::InvalidArgument(inner.to_string()),
            BusinessLogicError::Invalid(message) => AppError::InvalidArgument(message),
        })
    }

    fn business_logic_show(&self, override_path: &Option<PathBuf>) -> Result<BusinessLogicView> {
        let path = self.resolve_business_logic_path(override_path);
        let logic = self.load_business_logic(&path)?;
        Ok(BusinessLogicView {
            path: path.display().to_string(),
            policy_version: logic.policy_version.clone(),
            environment: logic.env.clone(),
            selection_method: format!("{:?}", logic.selection_method()),
            temperature: logic.selection_temperature(),
            top_k: logic.selection_top_k(12),
            bias: logic.plan_selection_bias(),
        })
    }

    fn business_logic_validate(
        &self,
        override_path: &Option<PathBuf>,
    ) -> Result<BusinessLogicValidation> {
        let path = self.resolve_business_logic_path(override_path);
        self.load_business_logic(&path)?;
        Ok(BusinessLogicValidation {
            path: path.display().to_string(),
            status: "valid".to_string(),
        })
    }

    fn business_logic_reload(
        &self,
        override_path: &Option<PathBuf>,
    ) -> Result<BusinessLogicReloadResult> {
        let path = self.resolve_business_logic_path(override_path);
        let logic = self.load_business_logic(&path)?;
        Ok(BusinessLogicReloadResult {
            path: path.display().to_string(),
            policy_version: logic.policy_version.clone(),
            reloaded: true,
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

    fn plan_store_or_create(&self) -> Result<SqlitePlanStore> {
        if let Some(parent) = self.plans_db.parent() {
            fs::create_dir_all(parent)?;
        }
        let builder = SqlitePlanStore::builder()
            .path(&self.plans_db)
            .create_if_missing(true)
            .read_only(false);
        let store = builder.build()?;
        store.initialize()?;
        Ok(store)
    }

    fn economy_store(&self, read_only: bool) -> Result<EconomyStore> {
        if let Some(parent) = self.economy_db.parent() {
            fs::create_dir_all(parent)?;
        }
        if read_only && !self.economy_db.exists() {
            let bootstrap = EconomyStoreBuilder::new().path(&self.economy_db).build()?;
            bootstrap.ensure_schema()?;
        }
        let mut builder = EconomyStoreBuilder::new().path(&self.economy_db);
        if read_only {
            builder = builder.read_only(true);
        }
        let store = builder.build()?;
        if !read_only {
            store.ensure_schema()?;
        }
        Ok(store)
    }

    fn audience_store(&self, read_only: bool) -> Result<AudienceStore> {
        if let Some(parent) = self.viewers_db.parent() {
            fs::create_dir_all(parent)?;
        }
        if read_only && !self.viewers_db.exists() {
            let bootstrap = AudienceStoreBuilder::new().path(&self.viewers_db).build()?;
            bootstrap.initialize()?;
        }
        let mut builder = AudienceStoreBuilder::new().path(&self.viewers_db);
        if read_only {
            builder = builder.read_only(true);
        }
        let store = builder.build()?;
        if !read_only {
            store.initialize()?;
        }
        Ok(store)
    }

    fn monetization_reports_dir(&self) -> PathBuf {
        if let Some(parent) = self.reports_dir.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::create_dir_all(&self.reports_dir);
        self.reports_dir.clone()
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
            audio_peak_db: Some(item.audio_peak_db),
            freeze_events: Some(item.freeze_events),
            black_frame_ratio: Some(item.black_frame_ratio),
            signature_deviation: Some(item.signature_deviation),
            power_watts: item.power_watts,
            ups_runtime_minutes: item.ups_runtime_minutes,
            ups_charge_percent: item.ups_charge_percent,
            ups_status: item.ups_status.clone(),
            ssd_wear_percent: item.ssd_wear_percent,
            gpu_temp_c: item.gpu_temp_c,
            ssd_temp_c: item.ssd_temp_c,
            fan_rpm: item.fan_rpm,
        }))
    }

    fn record_metrics(&self, queue_metrics: &QueueMetrics) -> Result<()> {
        let store = self.metrics_store()?;
        let mut record = MetricRecord {
            buffer_duration_h: queue_metrics.buffer_duration_hours,
            queue_length: queue_metrics.queue_length,
            played_last_hour: queue_metrics.played_last_hour,
            failures_last_hour: queue_metrics.failures_last_hour,
            avg_cpu_load: self.read_loadavg().unwrap_or_default(),
            avg_temp_c: self.read_temperature().unwrap_or_default(),
            latency_s: 0.0,
            stream_bitrate_mbps: 0.0,
            vmaf_live: 0.0,
            audio_peak_db: 0.0,
            freeze_events: 0,
            black_frame_ratio: 0.0,
            signature_deviation: 0.0,
            power_watts: None,
            ups_runtime_minutes: None,
            ups_charge_percent: None,
            ups_status: None,
            ssd_wear_percent: None,
            gpu_temp_c: None,
            ssd_temp_c: None,
            fan_rpm: None,
        };
        if let Some(power) = self.run_json_script::<PowerStatus>("check_power.sh") {
            record.power_watts = power.power_watts;
            record.ups_runtime_minutes = power.ups_runtime_minutes;
            record.ups_charge_percent = power.ups_charge_percent;
            record.ups_status = power.ups_status;
        }
        if let Some(thermal) = self.run_json_script::<ThermalStatus>("check_thermal.sh") {
            if let Some(cpu_temp) = thermal.cpu_temp_c {
                record.avg_temp_c = cpu_temp;
            }
            record.gpu_temp_c = thermal.gpu_temp_c;
            record.ssd_temp_c = thermal.ssd_temp_c;
            record.ssd_wear_percent = thermal.ssd_wear_percent;
            record.fan_rpm = thermal.fan_rpm;
        }
        store.record(&record)?;
        Ok(())
    }

    fn ledger_record(&self, args: &LedgerRecordArgs) -> Result<LedgerRecordResult> {
        let mut event = NewEconomyEvent::new(
            parse_event_type(&args.event_type)?,
            args.value_eur,
            args.source.clone(),
            args.context.clone(),
        );
        event.notes = args.notes.clone();
        let store = self.economy_store(false)?;
        let recorded = store.record_event(&event)?;
        Ok(LedgerRecordResult { event: recorded })
    }

    fn ledger_summary(&self, args: &LedgerSummaryArgs) -> Result<LedgerSummaryResult> {
        let hours = args.hours.max(1);
        let store = self.economy_store(true)?;
        let end = Utc::now();
        let start = end - Duration::hours(hours);
        let summary = store.summarize(start, end)?;
        Ok(LedgerSummaryResult { summary })
    }

    fn ledger_export(&self, args: &LedgerExportArgs) -> Result<LedgerExportResult> {
        let store = self.economy_store(true)?;
        let end = args
            .end
            .as_ref()
            .map(|value| parse_datetime(value))
            .transpose()?
            .unwrap_or_else(Utc::now);
        let start = args
            .start
            .as_ref()
            .map(|value| parse_datetime(value))
            .transpose()?
            .unwrap_or_else(|| end - Duration::days(7));
        let output_dir = args
            .output
            .clone()
            .unwrap_or_else(|| self.monetization_reports_dir());
        fs::create_dir_all(&output_dir)?;
        let export = store.export_ledger(start, end, &output_dir)?;
        Ok(LedgerExportResult { export })
    }

    fn audience_record(&self, args: &AudienceRecordArgs) -> Result<AudienceRecordResult> {
        let mut session = NewViewerSession::new(&args.session_id, &args.region, &args.device);
        let duration = Duration::minutes(args.duration_minutes.max(0.5) as i64);
        session.leave_time = Some(session.join_time + duration);
        session.bandwidth_mbps = args.bandwidth_mbps;
        session.engagement_score = args.engagement.map(|v| v.clamp(0.0, 1.0));
        let store = self.audience_store(false)?;
        let recorded = store.record_session(&session)?;
        Ok(AudienceRecordResult { session: recorded })
    }

    fn audience_metrics(&self, args: &AudienceMetricsArgs) -> Result<AudienceMetricsResult> {
        let hours = args.hours.max(1);
        let store = self.audience_store(true)?;
        let end = Utc::now();
        let start = end - Duration::hours(hours);
        let report = store.metrics(start, end)?;
        Ok(AudienceMetricsResult { report })
    }

    fn audience_heatmap(&self, args: &AudienceHeatmapArgs) -> Result<AudienceHeatmapResult> {
        let store = self.audience_store(true)?;
        let end = Utc::now();
        let start = end - Duration::hours(24);
        let report = store.metrics(start, end)?;
        let output = args
            .output
            .clone()
            .unwrap_or_else(|| self.monetization_reports_dir().join("audience_heatmap.png"));
        let generated = store.generate_heatmap(&report, &output)?;
        Ok(AudienceHeatmapResult { output: generated })
    }

    fn audience_report(&self, args: &AudienceReportArgs) -> Result<AudienceReportResultView> {
        let hours = args.hours.max(1);
        let store = self.audience_store(true)?;
        let end = Utc::now();
        let start = end - Duration::hours(hours);
        let report = store.metrics(start, end)?;
        let output = args
            .output
            .clone()
            .unwrap_or_else(|| self.monetization_reports_dir().join("audience_report.json"));
        let path = store.export_report(&report, &output)?;
        Ok(AudienceReportResultView { path, report })
    }

    fn run_adaptive(&self) -> Result<AdaptiveReportView> {
        let plan_store = self.plan_store(false)?;
        let economy = self.economy_store(true)?;
        let audience = self.audience_store(true)?;
        let programmer = AdaptiveProgrammer::new(plan_store, economy, audience);
        let report = programmer.run_once(Utc::now())?;
        Ok(AdaptiveReportView { report })
    }

    fn spots_list(&self) -> Result<SpotListResult> {
        let manager = MicroSpotManager::new(self.economy_store(false)?);
        let contracts = manager.list()?;
        Ok(SpotListResult { contracts })
    }

    fn spots_load(&self, args: &SpotLoadArgs) -> Result<SpotActionResult> {
        let manager = MicroSpotManager::new(self.economy_store(false)?);
        let contract = manager.register_from_file(&args.path)?;
        Ok(SpotActionResult {
            message: format!("Contrato {} carregado", contract.id),
        })
    }

    fn spots_toggle(&self, args: &SpotToggleArgs, active: bool) -> Result<SpotActionResult> {
        let manager = MicroSpotManager::new(self.economy_store(false)?);
        manager.set_active(&args.id, active)?;
        Ok(SpotActionResult {
            message: format!(
                "Contrato {} {}",
                args.id,
                if active { "ativado" } else { "desativado" }
            ),
        })
    }

    fn spots_inject(&self) -> Result<SpotInjectionResult> {
        let manager = MicroSpotManager::new(self.economy_store(false)?);
        let queue = self.queue_store(false)?;
        let injections = manager.inject_due(&queue, Utc::now())?;
        Ok(SpotInjectionResult { injections })
    }

    fn generate_dashboard(&self, args: &DashboardArgs) -> Result<DashboardResultView> {
        let economy = self.economy_store(true)?;
        let audience = self.audience_store(true)?;
        let plans = self.plan_store(false)?;
        let dashboard = MonetizationDashboard::new(&economy, &audience, &plans);
        let output_dir = args
            .output
            .clone()
            .unwrap_or_else(|| self.monetization_reports_dir());
        let artifacts = dashboard.generate(&output_dir, Utc::now())?;
        Ok(DashboardResultView { artifacts })
    }

    fn incident_report(&self, args: &IncidentReportArgs) -> Result<IncidentReportResultView> {
        let severity: IncidentSeverity = args.severity.into();
        let detected_at = args.detected_at.unwrap_or_else(Utc::now);
        let report = IncidentReport {
            incident_id: args.id.clone(),
            title: args.title.clone(),
            severity,
            category: args.category.clone(),
            detected_at,
            resolved_at: args.resolved_at,
            summary: args.summary.clone(),
            impact: args.impact.clone(),
            root_cause: args.root_cause.clone(),
            lessons_learned: args.lessons.clone(),
            actions_taken: args.actions.clone(),
            preventive_actions: args.preventive.clone(),
            timeline: args.timeline.clone(),
            author: args.author.clone(),
        };

        let fallback_dir =
            PathBuf::from(&self.bundle.vvtv.paths.vault_dir).join("incident_history");
        let configured_dir = self.bundle.vvtv.communications.history_dir(&fallback_dir);
        let history_dir = args
            .history_dir
            .clone()
            .unwrap_or_else(|| configured_dir.to_path_buf());
        let writer = IncidentHistoryWriter::new(&history_dir);
        let record = writer.write(&report, args.include_json)?;

        let mut notification_view = None;
        if !args.no_notify {
            let mut notification = report.notification();
            let link = args
                .link
                .clone()
                .unwrap_or_else(|| record.markdown_path.display().to_string());
            notification.link = Some(link);
            let comms = &self.bundle.vvtv.communications;
            let notifier = IncidentNotifier::new(
                comms.routing.clone(),
                comms.telegram.clone(),
                comms.email.clone(),
            )
            .with_dry_run(args.dry_run);
            let dispatch = notifier.notify(&notification)?;
            notification_view = Some(dispatch.into());
        }

        Ok(IncidentReportResultView {
            incident_id: report.incident_id,
            severity: severity.badge().to_string(),
            markdown_path: record.markdown_path.display().to_string(),
            json_path: record
                .json_path
                .as_ref()
                .map(|path| path.display().to_string()),
            notification: notification_view,
        })
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

        let runtime = Builder::new_current_thread()
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

    fn discovery_run(&self, args: &DiscoverArgs) -> Result<DiscoverReport> {
        init_discovery_tracing(args.debug);

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
        let launcher = BrowserLauncher::new(browser_config.clone(), profile_manager)?;

        let plan_store = Arc::new(self.plan_store_or_create()?);

        let engine = match &args.search_engine {
            Some(value) => SearchEngine::from_str(value).map_err(AppError::Browser)?,
            None => SearchEngine::from_str(&browser_config.discovery.search_engine)
                .map_err(AppError::Browser)?,
        };

        let search_config = Arc::new(SearchConfig {
            search_engine: engine,
            scroll_iterations: browser_config.discovery.scroll_iterations,
            max_results: browser_config.discovery.max_results_per_search,
            filter_domains: browser_config.discovery.filter_domains.clone(),
            delay_range_ms: (
                browser_config.discovery.search_delay_ms[0],
                browser_config.discovery.search_delay_ms[1],
            ),
        });

        let discovery_config = DiscoveryConfig {
            max_plans_per_run: args.max_plans,
            candidate_delay_range_ms: (
                browser_config.discovery.candidate_delay_ms[0],
                browser_config.discovery.candidate_delay_ms[1],
            ),
            stop_on_first_error: false,
            dry_run: args.dry_run,
            debug: args.debug,
        };

        let browser_config_arc = Arc::new(browser_config.clone());
        let pbd = Arc::new(PlayBeforeDownload::new(browser_config_arc));

        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|err| AppError::InvalidArgument(err.to_string()))?;

        let query = args.query.clone();
        let stats = runtime.block_on({
            let search_config = Arc::clone(&search_config);
            let discovery_config = discovery_config.clone();
            let plan_store = Arc::clone(&plan_store);
            let pbd = Arc::clone(&pbd);
            async move {
                let automation = Arc::new(launcher.launch().await?);
                let session_factory: Arc<dyn SearchSessionFactory> =
                    Arc::new(BrowserSearchSessionFactory::new(Arc::clone(&automation)));
                let searcher = ContentSearcher::new(search_config, session_factory);
                let pbd_runner: Arc<dyn DiscoveryPbd> =
                    Arc::new(BrowserPbdRunner::new(Arc::clone(&automation), pbd));
                let plan_store_trait: Arc<dyn DiscoveryPlanStore> = plan_store;
                let stats = {
                    let mut discovery = DiscoveryLoop::new(
                        searcher,
                        pbd_runner,
                        plan_store_trait,
                        discovery_config,
                    );
                    discovery.run(&query).await?
                };
                let automation = Arc::try_unwrap(automation).map_err(|_| {
                    BrowserError::Unexpected("browser automation still in use".into())
                })?;
                automation.shutdown().await?;
                Ok::<DiscoveryStats, BrowserError>(stats)
            }
        })?;

        Ok(DiscoverReport::from_stats(stats))
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

    fn run_json_script<T: DeserializeOwned>(&self, name: &str) -> Option<T> {
        let path = self.scripts_dir.join(name);
        if !path.exists() {
            return None;
        }
        let output = Command::new(&path).output().ok()?;
        if !output.status.success() {
            return None;
        }
        serde_json::from_slice(&output.stdout).ok()
    }
}

#[derive(Debug, Serialize)]
pub struct DiscoverReport {
    pub query: String,
    pub search_engine: String,
    pub dry_run: bool,
    pub candidates_found: usize,
    pub candidates_processed: usize,
    pub plans_created: usize,
    pub total_wait_ms: u64,
    pub duration_secs: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

impl DiscoverReport {
    fn from_stats(stats: DiscoveryStats) -> Self {
        Self {
            query: stats.query,
            search_engine: stats.search_engine,
            dry_run: stats.dry_run,
            candidates_found: stats.candidates_found,
            candidates_processed: stats.candidates_processed,
            plans_created: stats.plans_created,
            total_wait_ms: stats.total_wait_ms,
            duration_secs: stats.duration_secs,
            errors: stats.errors,
        }
    }
}

impl DisplayFallback for DiscoverReport {
    fn display(&self) -> String {
        let mut lines = vec![format!(
            "Descoberta \"{}\" via {}",
            self.query, self.search_engine
        )];
        lines.push(format!(
            "Candidatos: {} encontrados / {} processados",
            self.candidates_found, self.candidates_processed
        ));
        if self.dry_run {
            lines.push(format!("PLANs simulados: {}", self.plans_created));
        } else {
            lines.push(format!("PLANs criados: {}", self.plans_created));
        }
        lines.push(format!(
            "Atraso acumulado: {} ms | duração total: {} s",
            self.total_wait_ms, self.duration_secs
        ));
        if !self.errors.is_empty() {
            lines.push(format!("Falhas ({}):", self.errors.len()));
            for err in &self.errors {
                lines.push(format!("  - {err}"));
            }
        }
        lines.join("\n")
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
            if let Some(audio_peak) = metrics.audio_peak_db {
                lines.push(format!("  - Audio peak: {:.1} dBFS", audio_peak));
            }
            if let Some(freeze) = metrics.freeze_events {
                lines.push(format!("  - Freeze events: {freeze}"));
            }
            if let Some(black_ratio) = metrics.black_frame_ratio {
                lines.push(format!(
                    "  - Black frame ratio: {:.2}%",
                    black_ratio * 100.0
                ));
            }
            if let Some(signature) = metrics.signature_deviation {
                lines.push(format!("  - Signature Δ: {:.2}", signature));
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
    pub audio_peak_db: Option<f64>,
    pub freeze_events: Option<i64>,
    pub black_frame_ratio: Option<f64>,
    pub signature_deviation: Option<f64>,
    pub power_watts: Option<f64>,
    pub ups_runtime_minutes: Option<f64>,
    pub ups_charge_percent: Option<f64>,
    pub ups_status: Option<String>,
    pub ssd_wear_percent: Option<f64>,
    pub gpu_temp_c: Option<f64>,
    pub ssd_temp_c: Option<f64>,
    pub fan_rpm: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct PowerStatus {
    power_watts: Option<f64>,
    ups_runtime_minutes: Option<f64>,
    ups_charge_percent: Option<f64>,
    ups_status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ThermalStatus {
    cpu_temp_c: Option<f64>,
    gpu_temp_c: Option<f64>,
    ssd_temp_c: Option<f64>,
    ssd_wear_percent: Option<f64>,
    fan_rpm: Option<f64>,
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

impl DisplayFallback for LedgerRecordResult {
    fn display(&self) -> String {
        format!(
            "Evento {} registrado: €{:.2} — contexto {}",
            self.event.event_type.as_str(),
            self.event.value_eur,
            self.event.context
        )
    }
}

impl DisplayFallback for LedgerSummaryResult {
    fn display(&self) -> String {
        format!(
            "Financeiro {} → {} | receita {:.2} | custos {:.2} | resultado {:+.2}",
            self.summary.start,
            self.summary.end,
            self.summary.revenue_total(),
            self.summary.cost_total(),
            self.summary.net_revenue
        )
    }
}

impl DisplayFallback for LedgerExportResult {
    fn display(&self) -> String {
        format!(
            "Ledger exportado: {}\nManifesto: {}\nChecksum: {}",
            self.export.csv_path.display(),
            self.export.manifest_path.display(),
            self.export.checksum
        )
    }
}

impl DisplayFallback for AudienceRecordResult {
    fn display(&self) -> String {
        format!(
            "Sessão {} registrada — região {} — duração {:.1} min",
            self.session.session_id,
            self.session.region,
            self.session.duration_seconds as f64 / 60.0
        )
    }
}

impl DisplayFallback for AudienceMetricsResult {
    fn display(&self) -> String {
        let metrics = &self.report.metrics;
        format!(
            "Sessões: {} | Retenção 5min: {:.0}% | Retenção 30min: {:.0}% | Duração média: {:.1} min",
            metrics.total_sessions,
            metrics.retention_5min * 100.0,
            metrics.retention_30min * 100.0,
            metrics.avg_duration_minutes
        )
    }
}

impl DisplayFallback for AudienceHeatmapResult {
    fn display(&self) -> String {
        format!("Heatmap gerado em {}", self.output.display())
    }
}

impl DisplayFallback for AudienceReportResultView {
    fn display(&self) -> String {
        format!(
            "Relatório salvo em {} ({} sessões)",
            self.path.display(),
            self.report.metrics.total_sessions
        )
    }
}

impl DisplayFallback for AdaptiveReportView {
    fn display(&self) -> String {
        format!(
            "Atualizações aplicadas: {} | Receita líquida: {:+.2}",
            self.report.updates.len(),
            self.report.economy.net_revenue
        )
    }
}

impl DisplayFallback for SpotListResult {
    fn display(&self) -> String {
        if self.contracts.is_empty() {
            "Nenhum microspot cadastrado".into()
        } else {
            self.contracts
                .iter()
                .map(|contract| {
                    format!(
                        "- {} ({}): €{:.2}, duração {}s, ativo={}",
                        contract.id,
                        contract.sponsor,
                        contract.value_eur,
                        contract.duration_s,
                        contract.active
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}

impl DisplayFallback for SpotActionResult {
    fn display(&self) -> String {
        self.message.clone()
    }
}

impl DisplayFallback for SpotInjectionResult {
    fn display(&self) -> String {
        format!("Microspots injetados: {}", self.injections.len())
    }
}

impl DisplayFallback for DashboardResultView {
    fn display(&self) -> String {
        format!(
            "Dashboard: {}\nFinance: {}\nTrends: {}\nHeatmap: {}",
            self.artifacts.html_path.display(),
            self.artifacts.finance_path.display(),
            self.artifacts.trends_path.display(),
            self.artifacts.heatmap_path.display()
        )
    }
}

impl DisplayFallback for IncidentReportResultView {
    fn display(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "Incidente {} [{}]",
            self.incident_id, self.severity
        ));
        lines.push(format!("Postmortem: {}", self.markdown_path));
        if let Some(json) = &self.json_path {
            lines.push(format!("JSON: {json}"));
        }
        match &self.notification {
            Some(notification) => {
                lines.push(format!("Notificação: {}", notification.subject));
                for action in &notification.actions {
                    if let Some(detail) = &action.detail {
                        lines.push(format!(
                            "- {} -> {} ({detail})",
                            action.channel, action.status
                        ));
                    } else {
                        lines.push(format!("- {} -> {}", action.channel, action.status));
                    }
                }
            }
            None => lines.push("Notificação: não enviada".to_string()),
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::incident::IncidentSeverityArg;
    use chrono::{Duration, Utc};
    use rusqlite::params;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;
    use vvtv_core::{BrowserMetrics, IncidentTimelineEntry};

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
        let economy_db = data_dir.join("economy.sqlite");
        let viewers_db = data_dir.join("viewers.sqlite");
        let reports_dir = root.join("reports");

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
        conn_metrics
            .execute(
                "INSERT INTO metrics(
                buffer_duration_h,
                queue_length,
                played_last_hour,
                failures_last_hour,
                avg_cpu_load,
                avg_temp_c,
                latency_s,
                stream_bitrate_mbps,
                vmaf_live,
                audio_peak_db,
                freeze_events,
                black_frame_ratio,
                signature_deviation
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![4.0, 3, 2, 0, 55.0, 48.5, 6.2, 2.8, 92.0, -1.2, 0, 0.02, 0.18],
            )
            .unwrap();

        let conn_economy = Connection::open(&economy_db).unwrap();
        conn_economy
            .execute_batch(&fs::read_to_string("../sql/economy.sql").unwrap())
            .unwrap();
        conn_economy
            .execute(
                "INSERT INTO economy_events (
                    timestamp, event_type, value_eur, source, context, proof, notes
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    Utc::now().naive_utc(),
                    "view",
                    1.25f64,
                    "viewer",
                    "plan-1",
                    "deadbeef",
                    "sample"
                ],
            )
            .unwrap();

        let conn_viewers = Connection::open(&viewers_db).unwrap();
        conn_viewers
            .execute_batch(&fs::read_to_string("../sql/viewers.sql").unwrap())
            .unwrap();
        let join_time = Utc::now() - Duration::minutes(30);
        let leave_time = join_time + Duration::minutes(20);
        conn_viewers
            .execute(
                "INSERT INTO viewer_sessions (
                    session_id, viewer_id, join_time, leave_time, duration_seconds,
                    region, device, bandwidth_mbps, engagement_score, notes
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    "session-1",
                    "viewer-1",
                    join_time.naive_utc(),
                    leave_time.naive_utc(),
                    (leave_time - join_time).num_seconds(),
                    "EU",
                    "desktop",
                    25.0f64,
                    0.85f64,
                    "seed"
                ],
            )
            .unwrap();

        fs::create_dir_all(&reports_dir).unwrap();

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
            business_logic: None,
            data_dir: Some(data_dir.clone()),
            scripts_dir: Some(scripts_dir.clone()),
            plans_db: Some(plans_db.clone()),
            queue_db: Some(queue_db.clone()),
            metrics_db: Some(metrics_db.clone()),
            economy_db: Some(economy_db.clone()),
            viewers_db: Some(viewers_db.clone()),
            reports_dir: Some(reports_dir.clone()),
            fill_script: Some(scripts_dir.join("fill_buffer.sh")),
            token: None,
            format: OutputFormat::Json,
            command: Commands::Status,
        };

        let context = AppContext::new(&cli)?;
        Ok((temp, context))
    }

    #[test]
    fn incident_report_generates_history() {
        let (temp, context) = prepare_test_context().unwrap();
        let history_dir = temp.path().join("history");
        let args = IncidentReportArgs {
            id: "INC-TEST".to_string(),
            title: "Teste de incidente".to_string(),
            severity: IncidentSeverityArg::High,
            category: "Operacional".to_string(),
            summary: "Buffer em nível baixo".to_string(),
            impact: "Transmissão impactada por 2 minutos".to_string(),
            root_cause: "Manutenção programada".to_string(),
            detected_at: Some(Utc::now()),
            resolved_at: Some(Utc::now()),
            actions: vec!["Ativar emergency loop".to_string()],
            lessons: vec!["Monitorar janelas de manutenção".to_string()],
            preventive: vec!["Avisar equipe com antecedência".to_string()],
            timeline: vec![IncidentTimelineEntry::new(Utc::now(), "Alerta emitido")],
            author: Some("Eng. Operações".to_string()),
            link: None,
            history_dir: Some(history_dir.clone()),
            no_notify: false,
            dry_run: true,
            include_json: true,
        };
        let result = context.incident_report(&args).unwrap();
        assert!(Path::new(&result.markdown_path).exists());
        assert!(result.notification.is_some());
        let json_path = result.json_path.as_ref().expect("json should exist");
        assert!(Path::new(json_path).exists());
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
