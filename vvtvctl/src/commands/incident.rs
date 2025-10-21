use std::path::PathBuf;

use chrono::{DateTime, Utc};
use clap::{ArgAction, Args, ValueEnum};
use vvtv_core::{IncidentSeverity, IncidentTimelineEntry};

#[derive(Args, Debug)]
pub struct IncidentReportArgs {
    /// Identificador único do incidente
    #[arg(long, value_name = "ID")]
    pub id: String,
    /// Título curto do incidente
    #[arg(long, value_name = "TÍTULO")]
    pub title: String,
    /// Gravidade do incidente
    #[arg(long, value_enum)]
    pub severity: IncidentSeverityArg,
    /// Categoria (Técnica / Legal / Operacional)
    #[arg(long, default_value = "Operacional")]
    pub category: String,
    /// Resumo objetivo do ocorrido
    #[arg(long, value_name = "TEXTO")]
    pub summary: String,
    /// Impacto percebido
    #[arg(long, value_name = "TEXTO")]
    pub impact: String,
    /// Causa raiz identificada
    #[arg(long = "root-cause", value_name = "TEXTO")]
    pub root_cause: String,
    /// Momento da detecção do incidente (RFC3339)
    #[arg(long = "detected-at", value_parser = parse_datetime)]
    pub detected_at: Option<DateTime<Utc>>,
    /// Momento da resolução do incidente (RFC3339)
    #[arg(long = "resolved-at", value_parser = parse_datetime)]
    pub resolved_at: Option<DateTime<Utc>>,
    /// Ações executadas durante a resposta (repetir flag para múltiplas)
    #[arg(long = "action", action = ArgAction::Append, value_name = "TEXTO")]
    pub actions: Vec<String>,
    /// Lições aprendidas (repetir flag)
    #[arg(long = "lesson", action = ArgAction::Append, value_name = "TEXTO")]
    pub lessons: Vec<String>,
    /// Ações preventivas (repetir flag)
    #[arg(long = "prevention", action = ArgAction::Append, value_name = "TEXTO")]
    pub preventive: Vec<String>,
    /// Eventos relevantes na linha do tempo (<timestamp>=<descrição>)
    #[arg(
        long = "timeline",
        action = ArgAction::Append,
        value_parser = parse_timeline_entry,
        value_name = "RFC3339=descrição"
    )]
    pub timeline: Vec<IncidentTimelineEntry>,
    /// Responsável que assina o postmortem
    #[arg(long, value_name = "NOME")]
    pub author: Option<String>,
    /// Link para relatório externo (opcional)
    #[arg(long, value_name = "URL")]
    pub link: Option<String>,
    /// Diretório alternativo para armazenar histórico
    #[arg(long = "history-dir", value_name = "PATH")]
    pub history_dir: Option<PathBuf>,
    /// Não enviar notificações (apenas gerar arquivos)
    #[arg(long = "no-notify")]
    pub no_notify: bool,
    /// Executar sem disparar comandos externos
    #[arg(long = "dry-run")]
    pub dry_run: bool,
    /// Salvar cópia JSON além do relatório Markdown
    #[arg(long = "include-json")]
    pub include_json: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum IncidentSeverityArg {
    Critical,
    High,
    Medium,
    Low,
}

impl From<IncidentSeverityArg> for IncidentSeverity {
    fn from(value: IncidentSeverityArg) -> Self {
        match value {
            IncidentSeverityArg::Critical => IncidentSeverity::Critical,
            IncidentSeverityArg::High => IncidentSeverity::High,
            IncidentSeverityArg::Medium => IncidentSeverity::Medium,
            IncidentSeverityArg::Low => IncidentSeverity::Low,
        }
    }
}

fn parse_datetime(value: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value)
        .map_err(|err| format!("timestamp inválido: {err}"))
        .map(|dt| dt.with_timezone(&Utc))
}

fn parse_timeline_entry(value: &str) -> Result<IncidentTimelineEntry, String> {
    let (timestamp, description) = value
        .split_once('=')
        .ok_or_else(|| "use formato <timestamp>=<descrição>".to_string())?;
    let ts = parse_datetime(timestamp)?;
    if description.trim().is_empty() {
        return Err("descrição da linha do tempo não pode ser vazia".to_string());
    }
    Ok(IncidentTimelineEntry::new(ts, description.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_timeline_entry_parses_value() {
        let entry = parse_timeline_entry("2025-04-12T12:00:00Z=Falha detectada").unwrap();
        assert_eq!(entry.description, "Falha detectada");
        assert_eq!(entry.timestamp.to_rfc3339(), "2025-04-12T12:00:00+00:00");
    }
}
