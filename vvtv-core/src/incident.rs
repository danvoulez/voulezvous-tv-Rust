use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IncidentError {
    #[error("io error at {path:?}: {source}")]
    Io {
        source: std::io::Error,
        path: Option<PathBuf>,
    },
    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("command not found: {command}")]
    CommandNotFound { command: String },
    #[error("command {command} failed (status {status:?}): {stderr}")]
    CommandFailed {
        command: String,
        status: Option<i32>,
        stderr: String,
    },
    #[error("email recipients not configured")]
    MissingEmailRecipients,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentSeverity {
    Critical,
    High,
    Medium,
    Low,
}

impl IncidentSeverity {
    pub fn badge(self) -> &'static str {
        match self {
            Self::Critical => "CRITICAL",
            Self::High => "HIGH",
            Self::Medium => "MEDIUM",
            Self::Low => "LOW",
        }
    }

    pub fn emoji(self) -> &'static str {
        match self {
            Self::Critical => "🚨",
            Self::High => "⚠️",
            Self::Medium => "ℹ️",
            Self::Low => "📝",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentTimelineEntry {
    pub timestamp: DateTime<Utc>,
    pub description: String,
}

impl IncidentTimelineEntry {
    pub fn new(timestamp: DateTime<Utc>, description: impl Into<String>) -> Self {
        Self {
            timestamp,
            description: description.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentReport {
    pub incident_id: String,
    pub title: String,
    pub severity: IncidentSeverity,
    pub category: String,
    pub detected_at: DateTime<Utc>,
    #[serde(default)]
    pub resolved_at: Option<DateTime<Utc>>,
    pub summary: String,
    pub impact: String,
    pub root_cause: String,
    #[serde(default)]
    pub lessons_learned: Vec<String>,
    #[serde(default)]
    pub actions_taken: Vec<String>,
    #[serde(default)]
    pub preventive_actions: Vec<String>,
    #[serde(default)]
    pub timeline: Vec<IncidentTimelineEntry>,
    #[serde(default)]
    pub author: Option<String>,
}

impl IncidentReport {
    pub fn time_to_resolution(&self) -> Option<Duration> {
        self.resolved_at.map(|resolved| resolved - self.detected_at)
    }

    pub fn notification(&self) -> IncidentNotification {
        IncidentNotification {
            incident_id: self.incident_id.clone(),
            title: self.title.clone(),
            severity: self.severity,
            summary: self.summary.clone(),
            impact: self.impact.clone(),
            detected_at: self.detected_at,
            resolved_at: self.resolved_at,
            link: None,
        }
    }

    pub fn to_markdown(&self) -> String {
        let detected = self.detected_at.format("%Y-%m-%d %H:%M UTC");
        let resolved = self
            .resolved_at
            .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| "Em aberto".to_string());
        let duration = self
            .time_to_resolution()
            .map(|duration| format_duration(duration))
            .unwrap_or_else(|| "Indefinido".to_string());
        let lessons = if self.lessons_learned.is_empty() {
            "- (sem lições registradas)".to_string()
        } else {
            self.lessons_learned
                .iter()
                .map(|lesson| format!("- {lesson}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let actions = if self.actions_taken.is_empty() {
            "- (sem ações registradas)".to_string()
        } else {
            self.actions_taken
                .iter()
                .map(|action| format!("- {action}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let preventive = if self.preventive_actions.is_empty() {
            "- (nenhuma ação preventiva registrada)".to_string()
        } else {
            self.preventive_actions
                .iter()
                .map(|action| format!("- {action}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let timeline = if self.timeline.is_empty() {
            "- (sem eventos registrados)".to_string()
        } else {
            self.timeline
                .iter()
                .map(|entry| {
                    format!(
                        "- {} — {}",
                        entry.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
                        entry.description
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let author = self
            .author
            .as_deref()
            .unwrap_or("Eng. Responsável não informado");

        format!(
            "# VVTV POSTMORTEM — INCIDENT {id}\n\
             **Título:** {title}\n\
             **Data:** {detected}\n\
             **Gravidade:** {severity}\n\
             **Categoria:** {category}\n\
             **Impacto:** {impact}\n\
             **Resumo:** {summary}\n\
             **Causa-raiz:** {root_cause}\n\
             **Tempo até resolução:** {duration}\n\
             **Resolvido em:** {resolved}\n\
\n\
             ## Linha do tempo\n{timeline}\n\
\n\
             ## Ações executadas\n{actions}\n\
\n\
             ## Lições aprendidas\n{lessons}\n\
\n\
             ## Ações preventivas\n{preventive}\n\
\
             **Assinatura:** {author}\n",
            id = self.incident_id,
            title = self.title,
            detected = detected,
            severity = self.severity.badge(),
            category = self.category,
            impact = self.impact,
            summary = self.summary,
            root_cause = self.root_cause,
            duration = duration,
            resolved = resolved,
            timeline = timeline,
            actions = actions,
            lessons = lessons,
            preventive = preventive,
            author = author
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentNotification {
    pub incident_id: String,
    pub title: String,
    pub severity: IncidentSeverity,
    pub summary: String,
    pub impact: String,
    pub detected_at: DateTime<Utc>,
    #[serde(default)]
    pub resolved_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub link: Option<String>,
}

impl IncidentNotification {
    pub fn subject(&self) -> String {
        format!(
            "[{badge}] {id} — {title}",
            badge = self.severity.badge(),
            id = self.incident_id,
            title = self.title
        )
    }

    pub fn message(&self) -> String {
        let header = format!(
            "{emoji} VVTV incidente {id} — {title}",
            emoji = self.severity.emoji(),
            id = self.incident_id,
            title = self.title
        );
        let detected = self.detected_at.format("%Y-%m-%d %H:%M UTC");
        let resolved = self
            .resolved_at
            .map(|ts| ts.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| "em andamento".to_string());
        let link_line = self
            .link
            .as_ref()
            .map(|link| format!("Link: {link}\n"))
            .unwrap_or_default();
        format!(
            "{header}\n\
             Gravidade: {severity}\n\
             Detectado em: {detected}\n\
             Resolvido em: {resolved}\n\
             Impacto: {impact}\n\
             Resumo: {summary}\n{link_line}",
            header = header,
            severity = self.severity.badge(),
            detected = detected,
            resolved = resolved,
            impact = self.impact,
            summary = self.summary,
            link_line = link_line
        )
    }
}

fn format_duration(duration: Duration) -> String {
    let total_minutes = duration.num_minutes();
    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentChannel {
    Telegram,
    Email,
    Log,
}

impl fmt::Display for IncidentChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IncidentChannel::Telegram => write!(f, "telegram"),
            IncidentChannel::Email => write!(f, "email"),
            IncidentChannel::Log => write!(f, "log"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SeverityRouting {
    pub critical: Vec<IncidentChannel>,
    pub high: Vec<IncidentChannel>,
    pub medium: Vec<IncidentChannel>,
    pub low: Vec<IncidentChannel>,
}

impl SeverityRouting {
    pub fn channels(&self, severity: IncidentSeverity) -> &[IncidentChannel] {
        match severity {
            IncidentSeverity::Critical => &self.critical,
            IncidentSeverity::High => &self.high,
            IncidentSeverity::Medium => &self.medium,
            IncidentSeverity::Low => &self.low,
        }
    }
}

impl Default for SeverityRouting {
    fn default() -> Self {
        Self {
            critical: vec![IncidentChannel::Telegram, IncidentChannel::Email],
            high: vec![IncidentChannel::Telegram],
            medium: vec![IncidentChannel::Log],
            low: vec![IncidentChannel::Log],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TelegramChannelConfig {
    pub command: String,
    pub args: Vec<String>,
}

impl TelegramChannelConfig {
    fn default_command() -> String {
        "telegram-send".to_string()
    }

    fn send(&self, message: &str, dry_run: bool) -> Result<DispatchStatus, IncidentError> {
        if dry_run {
            return Ok(DispatchStatus::Skipped {
                reason: "dry-run".to_string(),
            });
        }
        let mut command = Command::new(&self.command);
        for arg in &self.args {
            command.arg(arg);
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|err| map_command_error(err, &self.command))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(message.as_bytes())
                .map_err(|err| IncidentError::Io {
                    source: err,
                    path: None,
                })?;
        }
        let output = child.wait_with_output().map_err(|err| IncidentError::Io {
            source: err,
            path: None,
        })?;
        if output.status.success() {
            Ok(DispatchStatus::Executed { detail: None })
        } else {
            Err(IncidentError::CommandFailed {
                command: self.command.clone(),
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            })
        }
    }
}

impl Default for TelegramChannelConfig {
    fn default() -> Self {
        Self {
            command: Self::default_command(),
            args: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EmailChannelConfig {
    pub command: String,
    pub args: Vec<String>,
    pub sender: Option<String>,
    pub recipients: Vec<String>,
    pub subject_prefix: Option<String>,
}

impl EmailChannelConfig {
    fn default_command() -> String {
        "sendmail".to_string()
    }

    fn send(
        &self,
        subject: &str,
        message: &str,
        dry_run: bool,
    ) -> Result<DispatchStatus, IncidentError> {
        if dry_run {
            return Ok(DispatchStatus::Skipped {
                reason: "dry-run".to_string(),
            });
        }
        if self.recipients.is_empty() {
            return Err(IncidentError::MissingEmailRecipients);
        }
        let subject_line = if let Some(prefix) = &self.subject_prefix {
            format!("{prefix} {subject}")
        } else {
            subject.to_string()
        };
        let mut command = Command::new(&self.command);
        for arg in &self.args {
            command.arg(arg);
        }
        for recipient in &self.recipients {
            command.arg(recipient);
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|err| map_command_error(err, &self.command))?;
        if let Some(mut stdin) = child.stdin.take() {
            let body = build_email_body(
                self.sender.as_deref(),
                &self.recipients,
                &subject_line,
                message,
            );
            stdin
                .write_all(body.as_bytes())
                .map_err(|err| IncidentError::Io {
                    source: err,
                    path: None,
                })?;
        }
        let output = child.wait_with_output().map_err(|err| IncidentError::Io {
            source: err,
            path: None,
        })?;
        if output.status.success() {
            Ok(DispatchStatus::Executed { detail: None })
        } else {
            Err(IncidentError::CommandFailed {
                command: self.command.clone(),
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            })
        }
    }
}

impl Default for EmailChannelConfig {
    fn default() -> Self {
        Self {
            command: Self::default_command(),
            args: vec![],
            sender: None,
            recipients: vec![],
            subject_prefix: None,
        }
    }
}

fn build_email_body(
    sender: Option<&str>,
    recipients: &[String],
    subject: &str,
    message: &str,
) -> String {
    let from_line = sender
        .map(|sender| format!("From: {sender}\n"))
        .unwrap_or_default();
    let to_line = if recipients.is_empty() {
        String::new()
    } else {
        format!("To: {}\n", recipients.join(", "))
    };
    format!(
        "{from_line}{to_line}Subject: {subject}\nContent-Type: text/plain; charset=UTF-8\n\n{message}\n",
        from_line = from_line,
        to_line = to_line,
        subject = subject,
        message = message
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentHistoryRecord {
    pub markdown_path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct IncidentHistoryWriter {
    base_dir: PathBuf,
}

impl IncidentHistoryWriter {
    pub fn new(base_dir: impl AsRef<Path>) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
        }
    }

    pub fn write(
        &self,
        report: &IncidentReport,
        include_json: bool,
    ) -> Result<IncidentHistoryRecord, IncidentError> {
        fs::create_dir_all(&self.base_dir).map_err(|err| IncidentError::Io {
            source: err,
            path: Some(self.base_dir.clone()),
        })?;
        let file_stem = format!(
            "{}-{}",
            report.detected_at.format("%Y%m%dT%H%M%SZ"),
            sanitize_identifier(&report.incident_id)
        );
        let markdown_path = self.base_dir.join(format!("{file_stem}.md"));
        fs::write(&markdown_path, report.to_markdown()).map_err(|err| IncidentError::Io {
            source: err,
            path: Some(markdown_path.clone()),
        })?;
        let mut json_path = None;
        if include_json {
            let serialized = serde_json::to_string_pretty(report)?;
            let path = self.base_dir.join(format!("{file_stem}.json"));
            fs::write(&path, serialized).map_err(|err| IncidentError::Io {
                source: err,
                path: Some(path.clone()),
            })?;
            json_path = Some(path);
        }
        Ok(IncidentHistoryRecord {
            markdown_path,
            json_path,
        })
    }
}

fn sanitize_identifier(identifier: &str) -> String {
    identifier
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '_',
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct IncidentDispatch {
    pub subject: String,
    pub message: String,
    pub severity: IncidentSeverity,
    pub actions: Vec<DispatchAction>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DispatchAction {
    pub channel: IncidentChannel,
    pub status: DispatchStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", content = "detail")]
pub enum DispatchStatus {
    Executed { detail: Option<String> },
    Skipped { reason: String },
    Failed { reason: String },
}

impl DispatchStatus {
    fn failed(reason: impl Into<String>) -> Self {
        DispatchStatus::Failed {
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IncidentCommunicationsConfig {
    pub incident_history_dir: Option<PathBuf>,
    pub routing: SeverityRouting,
    pub telegram: Option<TelegramChannelConfig>,
    pub email: Option<EmailChannelConfig>,
}

impl IncidentCommunicationsConfig {
    pub fn history_dir<'a>(&'a self, fallback: &'a Path) -> &'a Path {
        self.incident_history_dir.as_deref().unwrap_or(fallback)
    }
}

impl Default for IncidentCommunicationsConfig {
    fn default() -> Self {
        Self {
            incident_history_dir: None,
            routing: SeverityRouting::default(),
            telegram: None,
            email: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IncidentNotifier {
    routing: SeverityRouting,
    telegram: Option<TelegramChannelConfig>,
    email: Option<EmailChannelConfig>,
    dry_run: bool,
}

impl IncidentNotifier {
    pub fn new(
        routing: SeverityRouting,
        telegram: Option<TelegramChannelConfig>,
        email: Option<EmailChannelConfig>,
    ) -> Self {
        Self {
            routing,
            telegram,
            email,
            dry_run: false,
        }
    }

    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    pub fn notify(
        &self,
        notification: &IncidentNotification,
    ) -> Result<IncidentDispatch, IncidentError> {
        let mut actions = Vec::new();
        let subject = notification.subject();
        let message = notification.message();
        let channels = self.routing.channels(notification.severity);
        for channel in channels {
            match channel {
                IncidentChannel::Telegram => match &self.telegram {
                    Some(config) => match config.send(&message, self.dry_run) {
                        Ok(status) => actions.push(DispatchAction {
                            channel: *channel,
                            status,
                        }),
                        Err(err) => actions.push(DispatchAction {
                            channel: *channel,
                            status: DispatchStatus::failed(err.to_string()),
                        }),
                    },
                    None => actions.push(DispatchAction {
                        channel: *channel,
                        status: DispatchStatus::Skipped {
                            reason: "canal não configurado".to_string(),
                        },
                    }),
                },
                IncidentChannel::Email => match &self.email {
                    Some(config) => match config.send(&subject, &message, self.dry_run) {
                        Ok(status) => actions.push(DispatchAction {
                            channel: *channel,
                            status,
                        }),
                        Err(err) => actions.push(DispatchAction {
                            channel: *channel,
                            status: DispatchStatus::failed(err.to_string()),
                        }),
                    },
                    None => actions.push(DispatchAction {
                        channel: *channel,
                        status: DispatchStatus::Skipped {
                            reason: "canal não configurado".to_string(),
                        },
                    }),
                },
                IncidentChannel::Log => actions.push(DispatchAction {
                    channel: *channel,
                    status: DispatchStatus::Executed {
                        detail: Some("registrado".to_string()),
                    },
                }),
            }
        }
        Ok(IncidentDispatch {
            subject,
            message,
            severity: notification.severity,
            actions,
        })
    }
}

fn map_command_error(err: std::io::Error, command: &str) -> IncidentError {
    if err.kind() == std::io::ErrorKind::NotFound {
        IncidentError::CommandNotFound {
            command: command.to_string(),
        }
    } else {
        IncidentError::Io {
            source: err,
            path: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn markdown_contains_sections() {
        let report = IncidentReport {
            incident_id: "INC-001".to_string(),
            title: "Buffer underflow".to_string(),
            severity: IncidentSeverity::Critical,
            category: "Operacional".to_string(),
            detected_at: Utc::now(),
            resolved_at: Some(Utc::now()),
            summary: "Buffer caiu abaixo de 1h".to_string(),
            impact: "Transmissão interrompida por 12 minutos".to_string(),
            root_cause: "CDN origin offline".to_string(),
            lessons_learned: vec!["Monitorar TTL do manifest".to_string()],
            actions_taken: vec!["Failover automático executado".to_string()],
            preventive_actions: vec!["Adicionar alerta pré-falha".to_string()],
            timeline: vec![IncidentTimelineEntry::new(Utc::now(), "Alerta recebido")],
            author: Some("Eng. Operações".to_string()),
        };
        let markdown = report.to_markdown();
        assert!(markdown.contains("# VVTV POSTMORTEM"));
        assert!(markdown.contains("## Linha do tempo"));
        assert!(markdown.contains("## Lições aprendidas"));
    }

    #[test]
    fn history_writer_creates_files() {
        let temp = TempDir::new().unwrap();
        let writer = IncidentHistoryWriter::new(temp.path());
        let report = IncidentReport {
            incident_id: "INC-XYZ".to_string(),
            title: "Teste".to_string(),
            severity: IncidentSeverity::High,
            category: "Operacional".to_string(),
            detected_at: Utc::now(),
            resolved_at: None,
            summary: "Resumo".to_string(),
            impact: "Impacto mínimo".to_string(),
            root_cause: "Desconhecida".to_string(),
            lessons_learned: vec![],
            actions_taken: vec![],
            preventive_actions: vec![],
            timeline: vec![],
            author: None,
        };
        let record = writer.write(&report, true).unwrap();
        assert!(record.markdown_path.exists());
        assert!(record.json_path.unwrap().exists());
    }

    #[test]
    fn notifier_respects_routing() {
        let routing = SeverityRouting {
            critical: vec![IncidentChannel::Telegram, IncidentChannel::Email],
            high: vec![IncidentChannel::Log],
            medium: vec![IncidentChannel::Log],
            low: vec![IncidentChannel::Log],
        };
        let notifier = IncidentNotifier::new(routing, None, None).with_dry_run(true);
        let notification = IncidentNotification {
            incident_id: "INC-DRY".to_string(),
            title: "Teste".to_string(),
            severity: IncidentSeverity::High,
            summary: "Resumo".to_string(),
            impact: "Nenhum".to_string(),
            detected_at: Utc::now(),
            resolved_at: None,
            link: None,
        };
        let dispatch = notifier.notify(&notification).unwrap();
        assert_eq!(dispatch.actions.len(), 1);
        assert_eq!(dispatch.actions[0].channel, IncidentChannel::Log);
    }
}
