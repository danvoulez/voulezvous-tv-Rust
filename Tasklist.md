# VVTV Industrial Implementation Tasklist

> Use esta tasklist como plano mestre para implementar todo o VoulezVous.TV Industrial conforme o dossiê técnico. Marque cada caixa `[ ]` com `[x]` quando o Pull Request correspondente for mergeado.

## 🧭 Legenda
- **Epic** → macro-entregável transversal que pode gerar vários PRs.
- **PR** → pacote de trabalho integrado que deve ser entregue em um Pull Request.
- **Subtasks** → passos específicos necessários para completar o PR.
- ✅ Espaço reservado para future green check: `- [ ]`.

---

## Epic A — Fundamentos Físicos e Setup Base
- [x] **PR A1 — Provisionar ambiente físico e rede de operação**
  - [x] Inventariar hardware (Mac Mini M1/M2, storage externo, UPS) e validar temperatura/umidade do ambiente.
  - [x] Configurar infraestrutura elétrica (UPS ≥1500 VA, monitorar consumo, rotular cabos).
  - [x] Habilitar e configurar malha Tailscale (`voulezvous.ts.net`) com hostnames fixos.
  - [x] Documentar layout físico, incluindo padrões visuais (luvas cinza, unhas grafite fosco).
- [x] **PR A2 — Inicializar estrutura de diretórios e permissões /vvtv**
  - [x] Criar diretórios raiz (`/vvtv/system`, `/vvtv/data`, `/vvtv/cache`, `/vvtv/storage`, `/vvtv/broadcast`, `/vvtv/monitor`, `/vvtv/vault`).
  - [x] Criar usuário `vvtv` (UID 9001) e aplicar `chown`/`chmod` corretos (bin 755, bancos 600).
  - [x] Provisionar scripts utilitários iniciais (`check_stream_health.sh`, `halt_stream.sh`, `fill_buffer.sh`).
  - [x] Configurar logrotate/retention (7–14 dias) para `/vvtv/system/logs`.
- [x] **PR A3 — Instalar stack base de software**
  - [x] Automatizar instalação de FFmpeg (com codecs h264, aac, libx265, opus, rtmp, hls, srt), SQLite3, NGINX-RTMP, aria2, Chromium, Rust toolchain, tailscale.
  - [x] Garantir compile flags adequadas (macOS brew, Debian apt) e validar versões mínimas.
  - [x] Desabilitar serviços indesejados (Spotlight, Sleep, Time Machine) e configurar firewall interno.
  - [x] Registrar script de health check `/vvtv/system/bin/check_stream_health.sh` com métricas de fila/buffer/processos.
- [x] **PR A4 — Configurar NGINX-RTMP origin mínimo**
  - [x] Criar `/vvtv/broadcast/nginx.conf` com RTMP listen 1935, aplicação `live`, saída HLS, restrições de publish/play.
  - [x] Subir serviço (systemd/launchd), validar endpoints `/hls` e `/status`.
  - [x] Implementar rotação automática de segmentos (4 s, playlist 48 min) e políticas de cache/no-cache.
  - [x] Criar monitoramento básico do serviço (systemctl unit + health check script).
- [x] **PR A5 — Criar esquemas SQLite iniciais**
  - [x] `plans.sqlite`: tabela principal com índices por status/score.
  - [x] `queue.sqlite`: playout_queue com índices (status+created_at).
  - [x] `metrics.sqlite`: métricas operacionais (buffer, queue_length, cpu, temperatura, etc.).
  - [x] `economy.sqlite`: ledger econômico (`event_type`, `value_eur`, `proof`).
  - [x] Popular scripts SQL (migrações) e testes de integridade (`PRAGMA integrity_check`).

---

## Epic B — Configuração Lógica & Orquestração de Configs
- [x] **PR B1 — Publicar arquivos TOML de configuração base**
  - [x] `vvtv.toml`: limites de buffer, CPU, diretórios, rede, segurança, economia.
  - [x] `browser.toml`: flags Chromium, user-agent pool, viewport, simulação humana, PBD, seletores.
  - [x] `processor.toml`: política de download (aria2), remux/transcode, loudnorm, perfis HLS, QC thresholds.
  - [x] `broadcaster.toml`: política FIFO + bump, HLS output, failover, watchdog, parâmetros FFmpeg.
  - [x] Validar parsing/config loader em Rust para cada módulo.
- [x] **PR B2 — Implementar CLI de gerenciamento (`vvtvctl`)**
  - [x] Comandos: `status`, `plan list`, `queue show`, `buffer fill`, `health check`.
  - [x] Implementar autenticação local (opcional) e saída JSON/humana.
  - [x] Integração com configs TOML para overrides temporários.
  - [x] Testes unitários para parsing de argumentos e comandos críticos.
- [x] **PR B3 — Configurar logline signatures e vault**
  - [x] Inicializar diretório `/vvtv/vault` com estrutura `snapshots/`, `keys/`, `manifests/`.
  - [x] Automatizar assinatura `logline sign` para configs críticos (`vvtv.toml`, scripts, snapshots).
  - [x] Armazenar chaves (`voulezvous_foundation.pem`) com permissões `600` e documentar rotação.
  - [x] Criar script `logline verify` para auditoria de integridade diária.

---

## Epic C — Planner & Realizer (Gestão de PLANs)
- [ ] **PR C1 — Definir modelos e camada de acesso SQLite**
  - [ ] Structs Rust para `Plan`, `PlanStatus`, `PlanMetrics` com `serde`.
  - [ ] Implementar `SqlitePlanStore` (CRUD, filtros por status/score, locking). 
  - [ ] Adicionar migrações/seeders para testes.
  - [ ] Testes unitários para operações críticas (create, update status, scoring).
- [ ] **PR C2 — Implementar motor de scoring e seleção (Planner)**
  - [ ] Calcular `curation_score` combinando diversidade, tendência, duração.
  - [ ] Agendar seleção T-4h (cron async) que move `planned` → `selected`.
  - [ ] Aplicar heurística 80/20 (conteúdo diverso vs trending) e flags `hd_missing`.
  - [ ] Registrar logs de decisões e atualizar métricas (`plans_created`, `plans_selected`).
- [ ] **PR C3 — Implementar Realizer (pré-processamento)**
  - [ ] Worker assíncrono que reserva slots, seta `selected` → `in_progress`/`downloaded`.
  - [ ] Gerenciar locks por `plan_id` para evitar concorrência duplicada.
  - [ ] Atualizar `updated_at`, gravar histórico de tentativas (com backoff).
  - [ ] Integração com fila de processamento (notificação via canal interno ou tabela auxiliar).
- [ ] **PR C4 — Scripts CLI para inspeção/maintenance de planos**
  - [ ] `vvtvctl plan audit` (verificar planos antigos, expired, sem license).
  - [ ] `vvtvctl plan blacklist --add/--remove` (domínios problemáticos).
  - [ ] `vvtvctl plan import` (seed manual a partir de arquivo JSON).
  - [ ] Documentar fluxos de uso para operadores.

---

## Epic D — Browser Automation & Human Simulation
- [ ] **PR D1 — Wrapper Chromium/CDP com perfis isolados**
  - [ ] Integrar `chromiumoxide` (ou Playwright) com flags definidas em `browser.toml`.
  - [ ] Gerenciar perfis (`/vvtv/cache/browser_profiles/<id>`) com lifetime 24h e limpeza seletiva.
  - [ ] Implementar rotação de user-agent, viewport, proxy residencial.
  - [ ] Observabilidade básica (logs de página, erros CDP, métricas).
- [ ] **PR D2 — Motor de simulação humana (mouse/teclado/scroll)**
  - [ ] Implementar curvas Bézier com jitter, hesitação e overshoot.
  - [ ] Simular cadência de digitação com erros intencionais e correção.
  - [ ] Implementar padrões de ociosidade, micro-movimentos e troca de abas falsas.
  - [ ] Testes visuais (modo headed) e gravação de sessões QA.
- [ ] **PR D3 — Implementar Play-Before-Download (PBD) discovery**
  - [ ] Fluxo completo: abrir página → localizar player → clicar play → forçar HD/720p → aguardar 5–12 s.
  - [ ] Capturar URLs reais (HLS master/media playlist, DASH MPD, MP4 progressivo) via CDP Network monitor.
  - [ ] Implementar fallback via proxy MITM para sites anti-devtools.
  - [ ] Validar playback (readyState, currentTime, videoWidth/Height, buffer ahead) antes de registrar PLAN.
- [ ] **PR D4 — Metadata extraction & normalization**
  - [ ] Scraping DOM de títulos, duração, tags, breadcrumbs, resolution labels.
  - [ ] Sanitizar strings (remover emojis, espaços múltiplos, query params tracking).
  - [ ] Calcular `expected_bitrate`, `duration_est_s`, `license_hint`.
  - [ ] Armazenar no `Plan` e atualizar métricas (`pages_per_hour`, `hd_hit_rate`).
- [ ] **PR D5 — Error handling & antibot resilience**
  - [ ] Categorizar erros (player não encontrado, HD indisponível, bloqueio, manifest inválido).
  - [ ] Implementar retries com backoff (10min → 45min → 24h blacklist).
  - [ ] Trocar IP/proxy automaticamente em caso de bloqueio.
  - [ ] Registrar incidentes em log dedicado (`curator_failures.log`).
- [ ] **PR D6 — QA tooling para browser**
  - [ ] Scripts headless/headed para smoke test por domínio.
  - [ ] Captura de vídeos/frames das interações para inspeção.
  - [ ] Métricas no `metrics.sqlite` (`pbd_success_rate`, `proxy_rotations`).
  - [ ] Documentação de testes (passo-a-passo QA noturno).

---

## Epic E — Processor & Media Engineering (T-4h)
- [ ] **PR E1 — Reabrir página e confirmar rendition no T-4h**
  - [ ] Reutilizar automação PBD para revalidação (planos `selected`).
  - [ ] Garantir captura da playlist correta (media playlist atual, representation ativa).
  - [ ] Registrar fallback para 720p com flag `hd_missing`.
  - [ ] Validar playback (buffer ≥3s, videoWidth/Height coerente).
- [ ] **PR E2 — Implementar pipeline de download (HLS/DASH/progressivo)**
  - [ ] Gerar staging `/vvtv/cache/tmp_downloads/<plan_id>/source`.
  - [ ] Baixar media playlist + segments (aria2 parallel) e reescrever caminhos locais.
  - [ ] Suporte a DASH (`SegmentTemplate`, `SegmentList`) com opcional remux para HLS.
  - [ ] Progressivo: HEAD check (Content-Length ≥2MB), GET com resume.
  - [ ] Validar integridade (sequência, duração, checksums).
- [ ] **PR E3 — Decisor remux vs transcode**
  - [ ] Detectar codecs compatíveis (`avc1`, `aac`) para `-c copy`.
  - [ ] Remux HLS/MP4 para mezzanine (`master.mp4`) com `+faststart`.
  - [ ] Se incompatível, enfileirar transcode total.
  - [ ] Registrar decisão no `manifest.json`.
- [ ] **PR E4 — Transcode & loudnorm**
  - [ ] Transcode libx264 (preset slow/fast adaptativo, keyint 120, vbv config).
  - [ ] Normalização áudio EBU R128 (two-pass) com `loudnorm` e equalização cinema noturno.
  - [ ] Gerar variantes HLS 720p/480p (fmp4, independent segments).
  - [ ] Permitir packaging sem reencode (copy) quando possível.
- [ ] **PR E5 — QC técnico e geração de artefatos**
  - [ ] Rodar `ffprobe` e salvar `qc_pre.json` com resultados.
  - [ ] Calcular `checksums.json` (SHA-256 por arquivo) e `manifest.json` consolidado.
  - [ ] Atualizar `plans` (`edited`), inserir `playout_queue` (`queued`).
  - [ ] Manter staging limpo (limpar tmp_downloads após sucesso).
- [ ] **PR E6 — Tratamento de falhas e logging**
  - [ ] Implementar retries configuráveis (3 tentativas, delays crescentes).
  - [ ] Logar falhas em `/vvtv/system/logs/processor_failures.log` com motivos e ação tomada.
  - [ ] Atualizar status `rejected`/`quarantine` com causa.
  - [ ] Notificações de falhas críticas (Telegram/email opcional).
- [ ] **PR E7 — Testes integrados de pipeline**
  - [ ] Fluxo end-to-end em sandbox (input mock URLs → output HLS pronto).
  - [ ] Testes de performance (2 downloads + 2 transcodes concorrentes).
  - [ ] Testes de fallback (remux-only, transcode fallback, loudnorm off).
  - [ ] Documentar resultados e métricas de throughput.

---

## Epic F — Queue, Playout & Watchdogs
- [ ] **PR F1 — Implementar gestão da fila `playout_queue`**
  - [ ] Serviços Rust para leitura/atualização `queued` → `playing` → `played`/`failed`.
  - [ ] Regras FIFO com curation bump (score >0.85, inserir músicas a cada 10 itens).
  - [ ] Limpeza automática de `played` >72h e backups `.sql.gz` diários.
  - [ ] CLI `vvtvctl queue` para inspeção, prioridade manual, remoção.
- [ ] **PR F2 — Broadcaster (FFmpeg → RTMP → HLS)**
  - [ ] Orquestrar FFmpeg `-re` ingest a partir do asset (HLS 720p preferido).
  - [ ] Atualizar status de fila em tempo real e medir duração executada.
  - [ ] Implementar crossfade/fades (vídeo 400ms, áudio acrossfade sin) entre itens.
  - [ ] Integrar com emergency loop e buffer calculations.
- [ ] **PR F3 — Watchdog de playout**
  - [ ] Daemon (tokio) que verifica stream ativo (ffprobe), buffer >= mínimo, ffmpeg processos.
  - [ ] Ações automáticas: restart encoder, reiniciar nginx, injetar emergency loop, pausar downloads.
  - [ ] Limite de tentativas (3 restarts em 5 min) + escalonamento (alerta humano).
  - [ ] Logs estruturados (`watchdog.log`) e métricas (`failures_last_hour`).
- [ ] **PR F4 — Failover local e sincronização**
  - [ ] Implementar processo standby (`failover` stream) e comutação automática (<3s) em caso de freeze.
  - [ ] Sincronizar `/vvtv/storage/ready/` para nó backup (rsync horário, checksums).
  - [ ] Implementar gravação de 4h de live em `/vvtv/storage/archive/`.
  - [ ] Scripts de verificação (`sync_status.sh`).
- [ ] **PR F5 — Métricas e dashboard local**
  - [ ] Persistir métricas (`buffer_duration_h`, `queue_length`, `latency`, `stream_bitrate`).
  - [ ] Gerar dashboard HTML com gráfico de buffer, uptime, alertas recentes.
  - [ ] Exportar snapshots JSON para `monitor/dashboard.html`.
  - [ ] Testes de geração de relatórios (diário/horário).

---

## Epic G — Quality Control & Visual Consistency
- [ ] **PR G1 — Pipeline de pré-QC (ffprobe, thresholds)**
  - [ ] Automatizar `ffprobe` → `qc_pre.json`, validar resolução ≥720p, FPS, bitrate, keyframes.
  - [ ] Reprocessar automaticamente se fora de faixa (transcode fallback).
  - [ ] Registrar resultados e anexar a `manifest.json`.
- [ ] **PR G2 — Mid-QC perceptual (SSIM, VMAF, blackdetect)**
  - [ ] Integrar `libvmaf` e `ssim` com referência neutra.
  - [ ] Rodar `blackdetect`, `astats` para ruído/picos de áudio.
  - [ ] Aplicar correções (compressão adicional, equalização) quando thresholds violados.
  - [ ] Marcar flag `qc_warning` e ações automáticas.
- [ ] **PR G3 — Perfil estético VoulezVous**
  - [ ] Extrair paleta cromática (`palettegen`, `color-thief`) e temperatura.
  - [ ] Aplicar filtros (`eq=contrast=1.05:saturation=1.1`) para alinhar com signature profile.
  - [ ] Registrar `signature_deviation` e ajustar `curation_score` quando necessário.
  - [ ] Manter `signature_profile.json` e permitir ajustes calibrados.
- [ ] **PR G4 — Monitoramento live-QC**
  - [ ] Capturar frame do stream público a cada 5 min → `/monitor/captures/`.
  - [ ] Medir `stream_bitrate`, `vmaf_live`, `audio_peak`, `latency` via `ffprobe`/`curl`.
  - [ ] Alimentar dashboard com telemetria live.
  - [ ] Alerts automáticos (bitrate <1 Mbps, VMAF <80, freeze >2s).
- [ ] **PR G5 — Relatórios QC e revisão humana**
  - [ ] Gerar `qc_report_<date>.json` diário com totals, médias, desvio estético.
  - [ ] Implementar painel de revisão visual (4 amostras + 6 perguntas) para operador/IA.
  - [ ] Integrar feedback qualitativo no `curation_score`.
  - [ ] Exportar relatórios para vault (`/vvtv/reports/`).

---

## Epic H — Distribuição, CDN e Redundância Global
- [ ] **PR H1 — Automação de replicação Origin → Backup (Railway)**
  - [ ] Scripts `rclone sync` e `rclone check` para `/broadcast/hls` e `/storage/ready`.
  - [ ] Monitorar diferenças (>5%) e acionar failover automático.
  - [ ] Configurar jobs (cron/systemd timer) com logs auditáveis.
  - [ ] Documentar política de consistência e verificação.
- [ ] **PR H2 — Integração CDN primária (Cloudflare)**
  - [ ] Configurar DNS, cache rules (TTL 60s), worker script para reescrever host.
  - [ ] Implementar health checks e failover automation via API (switch origin).
  - [ ] Monitorar métricas (`cdn_hits`, `latency_avg`) e gerar relatórios.
  - [ ] Documentar playbook de ban/rate limit.
- [ ] **PR H3 — CDN secundária / backup (Backblaze/Bunny)**
  - [ ] Upload automático de segmentos finalizados (TTL 7 dias) com limpeza baseada em manifest.
  - [ ] Implementar fallback `switch_cdn.sh` e verificação (`dig`, `curl`).
  - [ ] Configurar tokens/assinaturas temporárias para segmentos.
  - [ ] Testar failover manual e automático.
- [ ] **PR H4 — Edge nodes & latency monitoring**
  - [ ] Provisionar script `logline --init-node --role=edge` para novos nós.
  - [ ] Implementar ping/latência periódica (`curl -w "%{time_total}"`).
  - [ ] Buffer local (15 s) + recarregamento automático quando sem segmentos novos.
  - [ ] Mapear latência por região (dashboard heatmap).
- [ ] **PR H5 — Segurança de distribuição (TLS, tokens, firewall)**
  - [ ] Forçar HTTPS/TLS1.3, assinar segmentos (SHA-256 + tokens 5 min).
  - [ ] Configurar firewall (permitir RTMP/HLS apenas via Tailscale/CDN IPs).
  - [ ] Registrar acessos anonimizados e armazenar logs rotativos.
  - [ ] Testar compliance (TLS checkers, security audit).

---

## Epic I — Monetização, Analytics & Adaptive Programming
- [ ] **PR I1 — Ledger econômico computável**
  - [ ] Implementar `EconomyStore` com eventos (`view`, `click`, `slot_sell`, `affiliate`, `cost`, `payout`).
  - [ ] Calcular hashes SHA-256 assinados (LogLine) por evento.
  - [ ] Exportar `.csv` e `.logline` semanais (`ledger_week_<date>.csv`).
  - [ ] Testes unitários (soma valores, reconciliação).
- [ ] **PR I2 — Coleta de métricas de audiência (`viewers.sqlite`)**
  - [ ] Registrar sessões (join/leave, duração, região, device, bandwidth, engagement_score).
  - [ ] Calcular derivados (`retention_5min`, `retention_30min`, `avg_duration`).
  - [ ] Gerar heatmap geográfico (PNG) e relatórios JSON.
  - [ ] Garantir anonimização (sem PII, modo ghost).
- [ ] **PR I3 — Motor de programação adaptativa**
  - [ ] Ajustar `planner` com base em retention, geo, new vs returning, desire vectors.
  - [ ] Implementar `desire_vector` por vídeo (integração com LogLine LLM local).
  - [ ] Atualizar `curation_score` dinamicamente e logs de decisão.
  - [ ] Testes de simulação (cenários com quedas/altos de retenção).
- [ ] **PR I4 — Gestão de micro-spots e slots premium**
  - [ ] Representar contratos `.lll` com duração, estilo visual, sponsor, value_eur.
  - [ ] Injetar microspots a cada 25–40 min sem quebrar ritmo (respeitar fades).
  - [ ] Registrar entregas no ledger e gerar relatórios financeiros diários.
  - [ ] Ferramentas CLI para agendamento/cancelamento de spots.
- [ ] **PR I5 — Dashboards de monetização e tendências**
  - [ ] `/monitor/` com gráficos de receita/hora, engajamento, heatmap.
  - [ ] `trends_weekly.json` com top tags, desire vectors médios por região.
  - [ ] Integração com agente curador (`agent_curador.lll`) para reprogramação automática.
  - [ ] Alertas quando ROI < limiar ou custos > orçamento.

---

## Epic J — Maintenance, Security & Long-Term Resilience
- [ ] **PR J1 — Automação de backups (hot/warm/cold)**
  - [ ] Scripts `backup_hot.sh` (1h), `backup_warm.sh` (6h), `backup_cold.sh` (24h) com destinos locais + B2.
  - [ ] Verificações `rclone check` e relatórios (`verify.log`).
  - [ ] Política de retenção (hot 24h, warm 72h, cold 30d) com limpeza automática.
  - [ ] Testar restauração (warm/cold) regularmente.
- [ ] **PR J2 — Self-check diário e autocorreção**
  - [ ] Implementar `/vvtv/system/bin/selfcheck.sh` (integridade DB, disco, temperatura, serviços, NTP).
  - [ ] Automatizar correções (limpar cache, reiniciar serviço, ajustar relógio).
  - [ ] Gerar relatórios JSON (`selfcheck_<date>.json`) e alertas se falha.
  - [ ] Integrar com watchdog para escalonamento.
- [ ] **PR J3 — Segurança computável**
  - [ ] Gerenciar identidades LogLine para cada nó (assinaturas, rotação de chaves, `logline sign`).
  - [ ] Configurar sandboxing (namespaces, cgroups, `chattr +i` em scripts críticos).
  - [ ] Firewall rules (allow 1935/8080/22 via Tailscale) + logging tentativas externas.
  - [ ] Auditoria mensal (`lynis audit system`) com armazenamento de resultados.
- [ ] **PR J4 — Monitoramento físico e aging de hardware**
  - [ ] Registrar ciclos preventivos (ventoinhas trimestrais, SSD SMART, UPS, pasta térmica, rotação Tailscale keys).
  - [ ] Criar scripts de inspeção (`check_power.sh`, `check_thermal.sh`).
  - [ ] Dashboard com indicadores de temperatura, consumo, desgaste SSD.
  - [ ] Documentar procedimentos de manutenção física (checklists diários/mensais/trimestrais).
- [ ] **PR J5 — Disaster recovery runbook automatizado**
  - [ ] Scripts `standby.sh` e `resume.sh` (hibernação/retorno com snapshot).
  - [ ] `logline shutdown --ritual=vvtv` + geração do frame final e manifesto.
  - [ ] `logline revive` para ressuscitar snapshot (testes em hardware alternativo).
  - [ ] Documentar procedimentos legais/institucionais (transferência de custódia, licenças).

---

## Epic K — Incident Response & Risk Management
- [ ] **PR K1 — Implementar Incident Playbook scripts**
  - [ ] Criar `/vvtv/system/bin` scripts descritos (check_queue, inject_emergency_loop, browser_diagnose, takedown, restart_encoder, switch_cdn, integrity_check, etc.).
  - [ ] Garantir idempotência e logging estruturado (`incident_log.md`).
  - [ ] Adicionar testes básicos (shellcheck, execução dry-run onde aplicável).
  - [ ] Documentar uso rápido (README/cheat-sheet).
- [ ] **PR K2 — Risk register operacional**
  - [ ] Transpor matriz de riscos (R1–R15) para formato vivo (Markdown/JSON) com owners, SLA, mitigação.
  - [ ] Automatizar lembretes de revisão (cron) e checklist de ações (auditorias, testes de buffer, sandbox integrity, etc.).
  - [ ] Integrar com métricas/alertas (ex: buffer <2h → risco R5).
  - [ ] Documentar processo de revisão mensal/quarterly.
- [ ] **PR K3 — Sistema de comunicação e escalonamento**
  - [ ] Integrar Telegram/email para alertas por gravidade (crítico, alto, médio, baixo).
  - [ ] Criar templates de postmortem (`VVTV POSTMORTEM — INCIDENT <ID>`).
  - [ ] Automatizar geração de relatórios de incidente (resumo, causa-raiz, ações preventivas).
  - [ ] Registrar histórico em vault (`incident_history/`).

---

## Epic L — Documentação, QA e Compliance
- [ ] **PR L1 — Documentação técnica contínua**
  - [ ] Reorganizar `/vvtv/docs` com guias (`deployment.md`, `failover.md`, `compliance_policies.md`, `maintenance.md`).
  - [ ] Incluir diagramas (fluxo de dados, rede, state machine, diretórios) em formato atualizável (Mermaid/Draw.io).
  - [ ] Documentar decisões de design (ADR) quando divergir do dossiê.
  - [ ] Publicar checklist de conformidade (GDPR, DRM, CSAM, ética).
- [ ] **PR L2 — Suite de testes automatizados**
  - [ ] `cargo test` cobertura >70% (unitários e componentes principais).
  - [ ] Testes end-to-end (mock pipeline) com assets de 10–30s.
  - [ ] Análise `cargo clippy` (zero warnings) e `cargo fmt` obrigatório.
  - [ ] Integração opcional com CI local (GitHub Actions/local runner) sem dependência cloud crítica.
- [ ] **PR L3 — Manual do operador & runbooks físicos**
  - [ ] Especificar rotinas diárias/semanais/mensais (baseadas no Apêndice H).
  - [ ] Instruções físicas (botão STOP STREAM, limpeza, inspeção visual, padrões de iluminação).
  - [ ] Guia rápido para hibernação/revival com imagens do frame final.
  - [ ] Checklist legal (licenças, consentimento, políticas de privacidade).
- [ ] **PR L4 — Auditoria e compliance contínua**
  - [ ] Ferramentas para verificar logs de consentimento/licenças (`license_audit.md`).
  - [ ] Scripts para detectar DRM/EME e abortar (registrar ocorrência).
  - [ ] Integração com mecanismos de hash CSAM (ex: PhotoDNA, se disponível localmente).
  - [ ] Relatórios de conformidade para stakeholders (PDF/Markdown).

---

## Epic M — Decommission & Resurrection Protocols
- [ ] **PR M1 — Ritual de desligamento computável**
  - [ ] Implementar `check_shutdown_ready.sh` para validar pré-requisitos.
  - [ ] Executar `logline shutdown --ritual=vvtv` (finaliza stream, congela fila, exporta bancos, gera snapshot, assina).
  - [ ] Capturar frame final (`final_frame.jpg`) com metadados (timestamp, origem, VMAF, signature).
  - [ ] Armazenar manifesto final (`Last Transmission Manifest`).
- [ ] **PR M2 — Modo hibernação e guardião**
  - [ ] Implementar `sleepguardd` (daemon 24h) para verificar integridade durante hibernação.
  - [ ] Garantir storage read-only, watchdogs suspensos, logs congelados.
  - [ ] Validar réveil via `resume.sh` mantendo estado intacto.
  - [ ] Documentar rituais simbólicos (closing bell, auto-descrição).
- [ ] **PR M3 — Protocolo de ressurreição completo**
  - [ ] `logline revive` automatizado (descompacta, restaura, verifica assinatura, reativa Tailscale, reinicia stream).
  - [ ] Testar revival em hardware alternativo (Mac Mini secundário / VM) e validar primeiro frame = frame final preservado.
  - [ ] Registrar genealogia (`generation`, `ancestor_signature`).
  - [ ] Documentar transferência de custódia e licença LogLine Heritage.

---

## Epic N — Benchmarks, Otimizações e Observabilidade
- [ ] **PR N1 — Benchmarking automatizado**
  - [ ] Scripts para medir throughput transcode (fps), loudnorm overhead, remux speed.
  - [ ] Scripts de rede (download speed, CDN origin pull latência, Tailscale overhead).
  - [ ] Relatórios comparando com baseline (G.1–G.7) e alertas se abaixo do esperado.
  - [ ] Integração com dashboard (`benchmarks.json`).
- [ ] **PR N2 — Observabilidade avançada**
  - [ ] Implementar coletores (Prometheus-style sem rede, JSON local) para CPU, memória, IO, temperatura.
  - [ ] Exportar métricas para visualização offline (Grafana local / HTML custom) sem depender da nuvem.
  - [ ] Alertas com thresholds (CPU>90%, temp>75°C, disk>80%, latency>7s).
  - [ ] Histórico 30 dias com compressão e arquivamento.
- [ ] **PR N3 — Otimizações progressivas**
  - [ ] Estratégia de preset adaptativo (fast/veryfast) baseado em buffer atual.
  - [ ] Preparar suporte a QuickSync/VideoToolbox e GPU transcode.
  - [ ] Analisar custos CDN vs P2P (WebTorrent) e preparar PoC.
  - [ ] Elaborar roadmap de expansão (multi-node, NAS dedicado, edge caching).

---

## Epic O — Governance & Cultural Preservation
- [ ] **PR O1 — Licenciamento e políticas institucionais**
  - [ ] Registrar licença LogLine Heritage para snapshots e conteúdos.
  - [ ] Criar `custodian.lll` para sucessão (transferência de chaves).
  - [ ] Definir políticas de abertura pública em caso de dissolução (domínio público).
  - [ ] Documentar acordos com parceiros e produtores (contratos computáveis).
- [ ] **PR O2 — Arquivo histórico e museu computável**
  - [ ] Automatizar exportação mensal (`vv_system_legacy_bundle_<date>.tar.zst`).
  - [ ] Catalogar snapshots (hashes públicos) e armazenar no vault.
  - [ ] Criar visualização narrativa (linha do tempo, frames marcantes, notas de operador).
  - [ ] Preparar site/portal de arqueologia digital (opcional) usando dados preservados.

---

### ✅ Checklist Meta de Projeto
- [ ] Garantir que todo PR inclua testes (unitário/integrado) e atualização da documentação relevante.
- [ ] Manter status público (README/STATUS.md) atualizado com progresso por Epic/PR.
- [ ] Atualizar esta Tasklist.md após cada entrega para refletir novas descobertas ou ajustes.
- [ ] Preparar release notes cumulativas (por Epic) quando milestones forem atingidos.

