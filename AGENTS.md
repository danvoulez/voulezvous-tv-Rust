# 🤖 AGENTS.md — Guia de Implementação VVTV para Agentes de IA

> **Documento de Referência para Agentes Autônomos**  
> Sistema: VoulezVous.TV Industrial Autonomous Streaming  
> Versão: 1.0  
> Última Atualização: 2025-10-20

---

## 📋 ÍNDICE

1. [Visão Geral do Projeto](#visão-geral-do-projeto)
2. [Princípios Fundamentais](#princípios-fundamentais)
3. [Arquitetura de Implementação](#arquitetura-de-implementação)
4. [Ordem de Construção (Fases)](#ordem-de-construção-fases)
5. [Módulos e Responsabilidades](#módulos-e-responsabilidades)
6. [Stack Tecnológica](#stack-tecnológica)
7. [Estrutura de Arquivos](#estrutura-de-arquivos)
8. [Padrões de Código](#padrões-de-código)
9. [Validação e Testes](#validação-e-testes)
10. [Deployment](#deployment)
11. [Referências](#referências)

---

## 🎯 VISÃO GERAL DO PROJETO

### O Que É VVTV?

**VoulezVous.TV** é um sistema de streaming autônomo 24/7 que opera **sem APIs**, usando browsers reais para simular comportamento humano e capturar conteúdo de forma ética e legal. O sistema:

- **Descobre** conteúdo via browser automation (Chromium + CDP)
- **Processa** vídeos com FFmpeg (transcode, normalization, HLS packaging)
- **Transmite** continuamente via RTMP → HLS → CDN
- **Monitora** qualidade e performance em tempo real
- **Adapta** programação baseada em métricas de audiência

### Por Que Este Projeto Existe?

- ✅ **Autonomia Total:** Opera 24/7 sem intervenção humana
- ✅ **Ético:** Sem APIs hackeadas, sem quebra de DRM, apenas conteúdo acessível publicamente
- ✅ **Resiliente:** Failover automático, emergency loops, self-healing
- ✅ **Computável:** Todo artefato é assinado, versionado e recuperável (LogLine OS)
- ✅ **Econômico:** Roda em hardware modesto (Mac Mini M1) com ROI 13×

### Filosofia Core: "Play-Before-Download" (PBD)

O mecanismo central é forçar **playback real no browser antes de baixar**. Isso garante:
- Captura do rendition HD correto (não manifests enganosos)
- Comportamento indistinguível de usuário humano
- Conformidade legal (se humano pode ver, sistema pode ver)

---

## 🧭 PRINCÍPIOS FUNDAMENTAIS

### 1. **Computable Everything**
- Todo estado é persistido em SQLite (plans, queue, metrics, economy)
- Todo artefato é assinado com LogLine signatures
- Todo snapshot é reproduzível via `logline revive`

### 2. **Human Simulation First**
- Movimentos de mouse seguem curvas Bézier com jitter
- Scroll tem bursts, overshoots, pausas naturais
- Timing varia (hesitation, idle, ociosidade)
- Erros de digitação simulados

### 3. **Fail-Safe by Design**
- Buffer de 6-8h garante continuidade mesmo com falhas
- Emergency loop injeta conteúdo seguro automaticamente
- Watchdogs reiniciam serviços sem downtime
- Failover para origem backup em <3s

### 4. **Quality Over Quantity**
- VMAF >85, SSIM >0.92 obrigatórios
- EBU R128 -14 LUFS para todo áudio
- Curadoria algorítmica (80% diverse, 20% trending)

### 5. **No API Hacking**
- Apenas browsers reais (Chromium)
- Apenas conteúdo publicamente acessível
- Abort se DRM detectado
- Abort se CSAM detectado

---

## 🏗️ ARQUITETURA DE IMPLEMENTAÇÃO

### Componentes Principais

```
┌─────────────────────────────────────────────────────────┐
│                    VVTV SYSTEM                          │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│  │   Curator    │→ │  Processor   │→ │ Broadcaster  │ │
│  │  (Browser)   │  │   (FFmpeg)   │  │ (RTMP→HLS)   │ │
│  └──────────────┘  └──────────────┘  └──────────────┘ │
│         ↓                  ↓                  ↓         │
│  ┌──────────────────────────────────────────────────┐  │
│  │           SQLite Databases (State)               │  │
│  │  plans.sqlite | queue.sqlite | metrics.sqlite   │  │
│  └──────────────────────────────────────────────────┘  │
│         ↓                                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│  │  Watchdog    │  │   Monitor    │  │   Economy    │ │
│  │ (Resilience) │  │   (QC)       │  │  (Analytics) │ │
│  └──────────────┘  └──────────────┘  └──────────────┘ │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### Fluxo de Dados Simplificado

1. **Curator** descobre URLs → cria PLANs (`status='planned'`)
2. **Planner** seleciona PLANs (T-4h window) → `status='selected'`
3. **Processor** baixa + transcoda → `status='edited'` + adiciona à fila
4. **Broadcaster** lê fila → transmite RTMP → NGINX gera HLS
5. **Monitor** captura frames → valida QC em tempo real
6. **Economy** registra métricas → adapta programação

---

## 🚧 ORDEM DE CONSTRUÇÃO (FASES)

### **FASE 0: Setup Inicial** (1-2 dias)

**Objetivo:** Preparar ambiente, estrutura de diretórios, dependências.

**Tasks:**
- [ ] Criar estrutura `/vvtv/{system,data,cache,storage,broadcast,docs,monitor,vault}`
- [ ] Instalar dependências: FFmpeg, SQLite, NGINX-RTMP, Chromium, Tailscale, Rust toolchain
- [ ] Criar usuário `vvtv` com permissões corretas
- [ ] Configurar NGINX-RTMP básico (porta 1935 → HLS porta 8080)
- [ ] Criar schemas SQLite para `plans.sqlite`, `queue.sqlite`, `metrics.sqlite`, `economy.sqlite`
- [ ] Escrever `vvtv.toml` config file

**Validação:**
```bash
# Estrutura existe?
ls -la /vvtv/

# Dependências instaladas?
ffmpeg -version && sqlite3 --version && chromium --version

# NGINX responde?
curl -I http://localhost:8080/hls/
```

**Referências:**
- `VVTV INDUSTRIAL DOSSIER.md` → Quick Start Guide (linhas 150-300)
- Apêndice D: Arquivos de Configuração

---

### **FASE 1: Módulo Planner (Core Foundation)** (2-3 dias)

**Objetivo:** Sistema básico de gestão de PLANs em SQLite.

**Implementar:**
```rust
// src/planner/mod.rs
pub struct Plan {
    pub plan_id: String,
    pub source_url: String,
    pub status: PlanStatus,
    pub created_at: DateTime<Utc>,
    pub metadata: PlanMetadata,
}

pub enum PlanStatus {
    Planned,
    Selected,
    Downloaded,
    Edited,
    Queued,
    Playing,
    Played,
    Failed,
}

pub trait PlanStore {
    fn create_plan(&mut self, url: &str, metadata: PlanMetadata) -> Result<Plan>;
    fn get_plan(&self, id: &str) -> Result<Option<Plan>>;
    fn update_status(&mut self, id: &str, status: PlanStatus) -> Result<()>;
    fn list_by_status(&self, status: PlanStatus) -> Result<Vec<Plan>>;
}
```

**Tasks:**
- [ ] Definir schema `plans.sqlite` (CREATE TABLE)
- [ ] Implementar `SqlitePlanStore` com CRUD operations
- [ ] Implementar state machine (transições válidas entre estados)
- [ ] Escrever testes unitários para cada operação
- [ ] CLI tool: `vvtv-planner list --status=planned`

**Validação:**
```bash
# Criar plan de teste
sqlite3 /vvtv/data/plans.sqlite \
  "INSERT INTO plans (plan_id, source_url, status) VALUES ('test-001', 'https://example.com/video', 'planned');"

# Listar
sqlite3 /vvtv/data/plans.sqlite "SELECT * FROM plans WHERE status='planned';"
```

**Referências:**
- Diagrama 3: Estados de Plano (State Machine) — linhas 3819-3878
- Bloco III: Processamento — linhas 700-900

---

### **FASE 2: Módulo Browser Automation (Curator)** (5-7 dias)

**Objetivo:** Browser automation com human simulation e PBD.

**Implementar:**
```rust
// src/curator/browser.rs
pub struct HumanSimulator {
    config: HumanSimConfig,
}

impl HumanSimulator {
    pub async fn move_mouse(&self, target: Point) -> Result<()> {
        // Gerar curva Bézier
        let curve = bezier::generate_curve(current_pos, target, self.config.jitter);
        // Animar ao longo da curva
        for point in curve {
            cdp::mouse_move(point).await?;
            tokio::time::sleep(Duration::from_millis(16)).await; // 60fps
        }
        Ok(())
    }

    pub async fn scroll(&self, delta: i32) -> Result<()> {
        // Scroll em bursts com overshoots
        let burst = rand::thread_rng().gen_range(self.config.scroll_burst);
        // ... implementação
    }
}

pub struct PlayBeforeDownload {
    browser: Browser,
    simulator: HumanSimulator,
}

impl PlayBeforeDownload {
    pub async fn execute(&mut self, url: &str) -> Result<HDManifest> {
        // 1. Navegar para página
        self.browser.navigate(url).await?;
        
        // 2. Simular humano: scroll, idle
        self.simulator.scroll_to_player().await?;
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // 3. Clicar em play
        let play_button = self.browser.wait_for_selector(".play").await?;
        self.simulator.click(play_button).await?;
        
        // 4. Aguardar playback (forçar HD)
        tokio::time::sleep(Duration::from_secs(8)).await;
        
        // 5. Interceptar network requests → capturar manifest HD
        let manifest = self.browser.get_hd_manifest().await?;
        
        Ok(manifest)
    }
}
```

**Tasks:**
- [ ] Integrar `chromiumoxide` ou `headless_chrome` crate
- [ ] Implementar curvas Bézier para mouse (`bezier.rs`)
- [ ] Implementar scroll natural com overshoots
- [ ] Implementar fingerprint randomization (canvas, WebGL)
- [ ] Implementar CDP network interception para capturar manifests
- [ ] Implementar PBD workflow completo
- [ ] Implementar proxy rotation (se configurado)
- [ ] CLI tool: `vvtv-curator discover --url=<URL>`

**Validação:**
```bash
# Testar PBD manualmente
vvtv-curator discover --url="https://vimeo.com/123456" --debug

# Output esperado:
# ✅ Navegação: OK
# ✅ Player detectado: OK
# ✅ Playback iniciado: OK
# ✅ Manifest HD capturado: https://vod.vimeo.com/.../master.m3u8
```

### Discovery Loop (Implementado)

- **ContentSearcher**: multi-engine (Google, Bing, DuckDuckGo) com heurísticas de vídeo (tags, duração, sinalização "creative commons").
- **DiscoveryLoop**: controle de cadência (delays configuráveis), estatísticas (`plans_per_run`, `hd_hit_rate`) e `dry-run` para inspeção.
- **PlanStore**: criação de PLANs com origem `discovery-loop`, atualizando métricas no `plans.sqlite` sob WAL.
- **CLI**: `vvtvctl discover --query "creative commons" --max-plans 10 --dry-run` gera relatórios estruturados.
- **Observabilidade**: rotacionar proxies automaticamente (`ip_rotation`) e registrar falhas em `curator_failures.log`.

#### Resiliência & QA — Atualizações Fase 2

- **Fingerprints**: manter canvas/WebGL/audio masking ativos; validar nightly via `docs/qa/nightly-smoke.md`.
- **Retry policy**: seguir agenda 10min → 45min → 24h e trocar IP após detecção de bot.
- **QA tooling**: usar `vvtvctl qa smoke-test` + `vvtvctl qa report` diariamente; dashboard HTML precisa ser anexado ao relatório do Discovery Loop.
- **Shell completions**: operadores podem gerar com `vvtvctl completions bash|zsh` para reduzir erros manuais.
- **SQLite**: todos os bancos operam em WAL + PRAGMAs (`cache_size`, `mmap_size`, `busy_timeout`). Executar `scripts/optimize_databases.sh /vvtv/data` após longas jornadas.

**Referências:**
- Bloco II: Browser Automation e Simulação Humana — linhas 400-600
- Apêndice D.2: browser.toml — linhas 4064-4161

---

### **FASE 3: Módulo Processor (Download + Transcode)** (5-7 dias)

**Objetivo:** Pipeline completo de processamento de vídeo.

**Implementar:**
```rust
// src/processor/mod.rs
pub struct Processor {
    config: ProcessorConfig,
    plan_store: Arc<Mutex<SqlitePlanStore>>,
}

impl Processor {
    pub async fn process_plan(&mut self, plan_id: &str) -> Result<ProcessedAsset> {
        // 1. Reabrir URL + PBD (confirmar HD)
        let manifest = self.reopen_and_pbd(&plan.source_url).await?;
        
        // 2. Download (HLS/DASH/progressive)
        let raw_file = self.download(&manifest).await?;
        
        // 3. Remux ou Transcode
        let master = if self.can_remux(&raw_file)? {
            self.remux(&raw_file).await?
        } else {
            self.transcode(&raw_file).await?
        };
        
        // 4. Loudness normalization (EBU R128 -14 LUFS)
        let normalized = self.loudnorm(&master).await?;
        
        // 5. Package HLS (720p + 480p variants)
        let hls_variants = self.package_hls(&normalized).await?;
        
        // 6. QC Pre (ffprobe + checksums)
        self.qc_pre(&hls_variants).await?;
        
        // 7. Stage to /storage/ready/
        let asset = self.stage(&plan_id, &hls_variants).await?;
        
        // 8. Update plan status → 'edited'
        self.plan_store.lock().unwrap().update_status(&plan_id, PlanStatus::Edited)?;
        
        // 9. Add to queue
        self.queue_for_playout(&plan_id, &asset).await?;
        
        Ok(asset)
    }

    async fn transcode(&self, input: &Path) -> Result<PathBuf> {
        let output = self.config.cache_dir.join("output.mp4");
        
        let mut cmd = Command::new("ffmpeg");
        cmd.args(&[
            "-i", input.to_str().unwrap(),
            "-c:v", "libx264",
            "-preset", &self.config.preset,
            "-crf", &self.config.crf.to_string(),
            "-profile:v", "high",
            "-level", "4.2",
            "-pix_fmt", "yuv420p",
            "-c:a", "aac",
            "-b:a", "160k",
            "-movflags", "+faststart",
            output.to_str().unwrap(),
        ]);
        
        let status = cmd.status().await?;
        if !status.success() {
            return Err(anyhow!("FFmpeg transcode failed"));
        }
        
        Ok(output)
    }

    async fn loudnorm(&self, input: &Path) -> Result<PathBuf> {
        // Two-pass EBU R128
        // Pass 1: Analyze
        let stats = self.ffmpeg_loudnorm_analyze(input).await?;
        
        // Pass 2: Normalize
        let output = self.ffmpeg_loudnorm_apply(input, &stats).await?;
        
        Ok(output)
    }
}
```

**Tasks:**
- [ ] Implementar download manager (preferir `aria2` ou `ffmpeg` nativo)
- [ ] Implementar detecção de tipo de stream (HLS/DASH/progressive)
- [ ] Implementar FFmpeg wrapper para transcode
- [ ] Implementar FFmpeg wrapper para remux (`-c copy`)
- [ ] Implementar EBU R128 two-pass loudness normalization
- [ ] Implementar HLS packaging (720p + 480p profiles)
- [ ] Implementar QC pré (ffprobe validation, checksums SHA256)
- [ ] Implementar staging para `/storage/ready/<plan_id>/`
- [ ] CLI tool: `vvtv-processor run --plan=<ID>`

**Validação:**
```bash
# Processar plan de teste
vvtv-processor run --plan=test-001

# Verificar output
ls -lh /vvtv/storage/ready/test-001/
# Esperado:
# master.mp4
# hls_720p.m3u8 + segments
# hls_480p.m3u8 + segments
# checksums.json
# manifest.json

# Validar loudness
ffmpeg -i /vvtv/storage/ready/test-001/master.mp4 -af ebur128 -f null -
# Esperado: Integrated loudness: -14.0 LUFS ±1.5
```

**Referências:**
- Bloco III: Processamento de Mídia — linhas 700-950
- Apêndice D.3: processor.toml — linhas 4163-4248
- Apêndice G.2: Benchmarks de Processamento — linhas 4975-5007

---

### **FASE 4: Módulo Broadcaster (Playout)** (3-5 dias)

**Objetivo:** Playout contínuo da fila → RTMP → HLS.

**Implementar:**
```rust
// src/broadcaster/mod.rs
pub struct Broadcaster {
    config: BroadcasterConfig,
    queue: Arc<Mutex<PlayoutQueue>>,
}

impl Broadcaster {
    pub async fn run(&mut self) -> Result<()> {
        loop {
            // 1. Pegar próximo asset da fila
            let asset = self.queue.lock().unwrap().pop_next()?;
            
            // 2. Atualizar status → 'playing'
            self.update_status(&asset.plan_id, PlayStatus::Playing).await?;
            
            // 3. Stream via FFmpeg → RTMP
            let mut encoder = self.spawn_encoder(&asset).await?;
            
            // 4. Monitor encoder health
            tokio::select! {
                _ = encoder.wait() => {
                    // Encoder terminou normalmente
                    self.update_status(&asset.plan_id, PlayStatus::Played).await?;
                }
                _ = self.watchdog_trigger.recv() => {
                    // Watchdog detectou freeze
                    encoder.kill().await?;
                    self.restart_encoder().await?;
                }
            }
            
            // 5. Check buffer
            if self.queue.lock().unwrap().buffer_hours() < 3.0 {
                warn!("Buffer baixo! Triggering emergency loop...");
                self.inject_emergency_loop().await?;
            }
        }
    }

    async fn spawn_encoder(&self, asset: &Asset) -> Result<Child> {
        let mut cmd = Command::new("ffmpeg");
        cmd.args(&[
            "-re",  // Read input at native frame rate
            "-i", &asset.master_path.to_str().unwrap(),
            "-c:v", "libx264",
            "-preset", "veryfast",
            "-b:v", "4M",
            "-maxrate", "4.5M",
            "-bufsize", "9M",
            "-pix_fmt", "yuv420p",
            "-g", "60",
            "-c:a", "aac",
            "-b:a", "160k",
            "-ar", "48000",
            "-f", "flv",
            &self.config.rtmp_origin,
        ]);
        
        let child = cmd.spawn()?;
        Ok(child)
    }
}
```

**Tasks:**
- [ ] Implementar fila de playout em `queue.sqlite`
- [ ] Implementar política FIFO + curation bump
- [ ] Implementar FFmpeg encoder para RTMP streaming
- [ ] Implementar watchdog para detectar freeze (última modificação de segment)
- [ ] Implementar emergency loop injection
- [ ] Implementar failover para encoder backup
- [ ] Configurar NGINX-RTMP para ingestão
- [ ] Daemon service: `vvtv-broadcaster daemon`

**Validação:**
```bash
# Iniciar broadcaster
vvtv-broadcaster daemon &

# Verificar RTMP ingest
ffprobe rtmp://localhost/live/main

# Verificar HLS output
curl http://localhost:8080/hls/live.m3u8
ls -lth /vvtv/broadcast/hls/*.ts | head -5

# Testar watchdog: matar encoder e verificar restart automático
pkill -9 ffmpeg
sleep 5
pgrep -f "ffmpeg.*rtmp" # Deve retornar PID (restarted)
```

**Referências:**
- Bloco IV: Queue, Playout e Broadcaster — linhas 950-1150
- Apêndice D.4: broadcaster.toml — linhas 4250-4288
- Apêndice E.4: restart_encoder.sh — linhas 4485-4537

---

### **FASE 5: Módulo Monitor (QC em Tempo Real)** (3-4 dias)

**Objetivo:** Capturar frames do stream e validar qualidade continuamente.

**Implementar:**
```rust
// src/monitor/mod.rs
pub struct Monitor {
    config: MonitorConfig,
    metrics_store: Arc<Mutex<MetricsStore>>,
}

impl Monitor {
    pub async fn run(&mut self) -> Result<()> {
        let mut interval = tokio::time::interval(Duration::from_secs(self.config.capture_interval));
        
        loop {
            interval.tick().await;
            
            // 1. Capturar frame do stream
            let frame = self.capture_frame().await?;
            
            // 2. Validar QC (básico: não é tela preta/congelada)
            let qc = self.validate_frame(&frame).await?;
            
            // 3. Registrar métrica
            self.metrics_store.lock().unwrap().record_qc(qc)?;
            
            // 4. Se falha crítica, alertar
            if qc.is_critical() {
                self.send_alert(&qc).await?;
            }
        }
    }

    async fn capture_frame(&self) -> Result<Frame> {
        // Usar FFmpeg para capturar 1 frame do HLS
        let output = self.config.capture_dir.join(format!("frame_{}.jpg", Utc::now().timestamp()));
        
        let status = Command::new("ffmpeg")
            .args(&[
                "-i", &self.config.hls_url,
                "-vframes", "1",
                "-q:v", "2",
                output.to_str().unwrap(),
            ])
            .status()
            .await?;
        
        if !status.success() {
            return Err(anyhow!("Frame capture failed"));
        }
        
        Ok(Frame { path: output, timestamp: Utc::now() })
    }
}
```

**Tasks:**
- [ ] Implementar captura periódica de frames (FFmpeg snapshot)
- [ ] Implementar detecção de tela preta (histogram analysis)
- [ ] Implementar detecção de frame congelado (compare hashes)
- [ ] Implementar registro de métricas em `metrics.sqlite`
- [ ] Implementar sistema de alertas (Telegram/email)
- [ ] Dashboard HTML simples para visualização
- [ ] Daemon service: `vvtv-monitor daemon`

**Validação:**
```bash
# Iniciar monitor
vvtv-monitor daemon &

# Verificar capturas
ls -lth /vvtv/monitor/captures/*.jpg | head -5

# Verificar métricas
sqlite3 /vvtv/data/metrics.sqlite "SELECT * FROM qc_checks ORDER BY timestamp DESC LIMIT 10;"
```

**Referências:**
- Bloco V: Quality Control — linhas 1150-1300
- Bloco VII: Monitoramento — linhas 1500-1650

---

### **FASE 6: Módulo Watchdog (Resilience)** (2-3 dias)

**Objetivo:** Self-healing automático para serviços críticos.

**Implementar:**
```rust
// src/watchdog/mod.rs
pub struct Watchdog {
    config: WatchdogConfig,
    services: Vec<Service>,
}

impl Watchdog {
    pub async fn run(&mut self) -> Result<()> {
        let mut interval = tokio::time::interval(Duration::from_secs(self.config.interval));
        
        loop {
            interval.tick().await;
            
            for service in &self.services {
                match self.check_health(service).await {
                    Ok(_) => {
                        debug!("Service {} healthy", service.name);
                    }
                    Err(e) => {
                        error!("Service {} unhealthy: {}", service.name, e);
                        self.restart_service(service).await?;
                    }
                }
            }
        }
    }

    async fn check_health(&self, service: &Service) -> Result<()> {
        match service.kind {
            ServiceKind::Broadcaster => {
                // Verificar se FFmpeg está rodando
                if !self.is_process_running("ffmpeg.*rtmp")? {
                    return Err(anyhow!("Encoder not running"));
                }
                
                // Verificar se segments estão sendo gerados
                let last_segment = self.get_last_segment_time()?;
                if last_segment.elapsed() > Duration::from_secs(30) {
                    return Err(anyhow!("No new segments in 30s"));
                }
                
                Ok(())
            }
            ServiceKind::Nginx => {
                // Verificar se NGINX responde
                reqwest::get("http://localhost:8080/status").await?;
                Ok(())
            }
            // ... outros serviços
        }
    }

    async fn restart_service(&self, service: &Service) -> Result<()> {
        warn!("Restarting service: {}", service.name);
        
        // Executar script de restart
        let status = Command::new(&service.restart_script)
            .status()
            .await?;
        
        if status.success() {
            info!("Service {} restarted successfully", service.name);
        } else {
            error!("Failed to restart service {}", service.name);
        }
        
        Ok(())
    }
}
```

**Tasks:**
- [ ] Implementar health checks para cada serviço (broadcaster, nginx, processor)
- [ ] Implementar restart automático via scripts shell
- [ ] Implementar limite de tentativas (max 3 restarts em 5 min)
- [ ] Implementar escalation (se falhar, alertar humano)
- [ ] Systemd integration (criar `.service` files)
- [ ] Daemon service: `vvtv-watchdog daemon`

**Validação:**
```bash
# Iniciar watchdog
vvtv-watchdog daemon &

# Simular falha: matar encoder
pkill -9 ffmpeg

# Watchdog deve detectar e reiniciar em ~30s
tail -f /vvtv/system/logs/watchdog.log
# Esperado:
# [ERROR] Service broadcaster unhealthy: No new segments in 30s
# [WARN] Restarting service: broadcaster
# [INFO] Service broadcaster restarted successfully
```

**Referências:**
- Bloco VI: Failover e Resiliência — linhas 1300-1450
- Apêndice B: Incident Playbook — linhas 2950-3200

---

### **FASE 7: Módulo Economy (Analytics)** (2-3 dias)

**Objetivo:** Ledger local e métricas de monetização.

**Implementar:**
```rust
// src/economy/mod.rs
pub struct EconomyLedger {
    db: SqliteConnection,
}

impl EconomyLedger {
    pub fn record_view(&mut self, timestamp: DateTime<Utc>, viewer_ip: IpAddr) -> Result<()> {
        // Registrar view (anonimizado)
        let geo = self.ip_to_geo(viewer_ip)?;
        
        self.db.execute(
            "INSERT INTO views (timestamp, country, city) VALUES (?, ?, ?)",
            params![timestamp, geo.country, geo.city],
        )?;
        
        Ok(())
    }

    pub fn calculate_revenue(&self, period: Period) -> Result<Revenue> {
        // CPM * impressions
        let views = self.count_views(period)?;
        let passive_revenue = (views as f64 / 1000.0) * self.config.cpm;
        
        // Micro-spots
        let spots = self.count_spots(period)?;
        let spot_revenue = spots as f64 * self.config.spot_rate;
        
        // Total
        Ok(Revenue {
            passive: passive_revenue,
            spots: spot_revenue,
            total: passive_revenue + spot_revenue,
        })
    }

    pub fn get_top_content(&self, limit: usize) -> Result<Vec<ContentStats>> {
        // Query para conteúdo mais assistido (retention, skips, etc)
        self.db.query_row(
            "SELECT plan_id, SUM(watch_time_s) as total_watch FROM views GROUP BY plan_id ORDER BY total_watch DESC LIMIT ?",
            params![limit],
            // ...
        )
    }
}
```

**Tasks:**
- [ ] Implementar schema `economy.sqlite` (views, spots, affiliates)
- [ ] Implementar registro de views (anonimizado, GDPR-compliant)
- [ ] Implementar cálculo de receita (CPM, spots, afiliados)
- [ ] Implementar métricas de conteúdo (top viewed, retention, skips)
- [ ] Implementar geo-heatmap (agregado por país/cidade)
- [ ] Implementar export para CSV/JSON
- [ ] CLI tool: `vvtv-economy report --period=30d`

**Validação:**
```bash
# Gerar relatório
vvtv-economy report --period=7d

# Output esperado:
# ════════════════════════════════════
#   VVTV ECONOMY REPORT (7 days)
# ════════════════════════════════════
# Total Views: 15,234
# Unique Viewers: 1,203
# Avg Watch Time: 12m 34s
# 
# Revenue:
#   Passive (CPM $2.00): $30.47
#   Micro-spots (14): $70.00
#   TOTAL: $100.47
# 
# Top Content:
#   1. plan-abc123 (234 views, 45m avg)
#   2. plan-def456 (198 views, 38m avg)
```

**Referências:**
- Bloco VIII: Economia Computável — linhas 1650-1850
- Apêndice G.6: Benchmarks Econômicos — linhas 5085-5110

---

### **FASE 8: Scripts Operacionais** (2 dias)

**Objetivo:** Shell scripts para operações manuais e troubleshooting.

**Tasks:**
- [ ] Implementar `/vvtv/system/bin/check_stream_health.sh`
- [ ] Implementar `/vvtv/system/bin/check_queue.sh`
- [ ] Implementar `/vvtv/system/bin/inject_emergency_loop.sh`
- [ ] Implementar `/vvtv/system/bin/run_download_cycle.sh`
- [ ] Implementar `/vvtv/system/bin/restart_encoder.sh`
- [ ] Implementar `/vvtv/system/bin/switch_cdn.sh`
- [ ] Implementar `/vvtv/system/bin/selfcheck.sh`
- [ ] Implementar backup scripts (hot/warm/cold)
- [ ] Tornar todos executáveis: `chmod +x /vvtv/system/bin/*.sh`

**Validação:**
```bash
# Testar cada script
/vvtv/system/bin/check_stream_health.sh
/vvtv/system/bin/check_queue.sh
/vvtv/system/bin/selfcheck.sh
```

**Referências:**
- Apêndice E: Scripts Shell Operacionais — linhas 4292-4706

---

### **FASE 9: Deployment e CDN** (3-4 dias)

**Objetivo:** Deploy completo com Cloudflare CDN e Tailscale VPN.

**Tasks:**
- [ ] Configurar Tailscale mesh (voulezvous.ts.net)
- [ ] Configurar Cloudflare DNS: `voulezvous.tv` → Lisboa origin
- [ ] Configurar Cloudflare Workers para cache rules (m3u8: no-cache, segments: 60s TTL)
- [ ] Configurar Railway backup origin (secondary)
- [ ] Configurar Backblaze B2 para storage backup
- [ ] Implementar sync automático entre origins (rsync via Tailscale)
- [ ] Configurar firewall rules (allow RTMP/HLS apenas de Tailscale + CDN IPs)
- [ ] Testar failover manual: switch primary → backup
- [ ] Configurar monitoring externo (uptimerobot ou similar)

**Validação:**
```bash
# Testar acesso via CDN
curl -I https://voulezvous.tv/hls/live.m3u8

# Testar Tailscale connectivity
tailscale ping backup-origin

# Testar failover
/vvtv/system/bin/switch_cdn.sh backup
curl -I https://voulezvous.tv/hls/live.m3u8  # Deve funcionar
/vvtv/system/bin/switch_cdn.sh primary
```

**Referências:**
- Bloco I: Infraestrutura Computável — linhas 200-400
- Diagrama 2: Arquitetura de Rede — linhas 3770-3817
- Apêndice E.5: switch_cdn.sh — linhas 4539-4604

---

### **FASE 10: Testes de Integração e Stress** (3-5 dias)

**Objetivo:** Validar sistema completo em condições reais.

**Tasks:**
- [ ] Teste end-to-end: URL descoberta → processamento → playout → CDN
- [ ] Teste de buffer underflow: parar processor, verificar emergency loop
- [ ] Teste de failover: matar encoder, verificar restart automático
- [ ] Teste de qualidade: validar VMAF/SSIM/LUFS em amostras
- [ ] Teste de concorrência: 2 transcodes + 2 browsers simultaneamente
- [ ] Teste de uptime: rodar 72h contínuas sem intervenção
- [ ] Teste de recovery: corromper DB, restaurar de backup
- [ ] Stress test: queue com 100+ items, verificar performance
- [ ] Load test CDN: simular 1000 viewers, medir latência

**Validação:**
```bash
# Teste end-to-end automatizado
./tests/e2e_test.sh

# Output esperado:
# ✅ Phase 1: Discovery (curator) - PASSED
# ✅ Phase 2: Processing (processor) - PASSED
# ✅ Phase 3: Queueing - PASSED
# ✅ Phase 4: Broadcasting - PASSED
# ✅ Phase 5: CDN delivery - PASSED
# ✅ Phase 6: QC validation - PASSED
# 
# Total duration: 8m 23s
# All tests PASSED ✅
```

**Referências:**
- Apêndice G: Benchmarks e Performance — linhas 4956-5139
- Apêndice H: Troubleshooting Expandido — linhas 5142-5573

---

## 📦 MÓDULOS E RESPONSABILIDADES

### Tabela de Módulos

| Módulo | Linguagem | Responsabilidade | Entrada | Saída |
|--------|-----------|------------------|---------|-------|
| `vvtv-planner` | Rust | Gestão de PLANs e state machine | URLs, status updates | plans.sqlite |
| `vvtv-curator` | Rust + Chromium | Browser automation, PBD | URLs | PLANs (com manifest HD) |
| `vvtv-processor` | Rust + FFmpeg | Download, transcode, QC | PLANs (selected) | Assets em /storage/ready/ |
| `vvtv-broadcaster` | Rust + FFmpeg | Playout, RTMP streaming | queue.sqlite | RTMP stream |
| `vvtv-monitor` | Rust | QC em tempo real | HLS stream | metrics.sqlite |
| `vvtv-watchdog` | Rust | Health checks, restarts | Service status | Auto-healing |
| `vvtv-economy` | Rust | Analytics, ledger | Métricas | Relatórios |
| `nginx-rtmp` | NGINX | RTMP ingest → HLS | RTMP stream | HLS segments |

### Dependências entre Módulos

```
curator → planner (cria PLANs)
planner → processor (seleciona PLANs)
processor → planner (atualiza status)
processor → queue (adiciona assets)
broadcaster → queue (lê assets)
broadcaster → nginx (envia RTMP)
monitor → metrics (registra QC)
watchdog → todos (health checks)
economy → metrics (lê para analytics)
```

---

## 🛠️ STACK TECNOLÓGICA

### Core

- **Linguagem:** Rust (stable, edition 2021)
- **Build:** Cargo
- **Async runtime:** Tokio
- **Database:** SQLite 3 + `rusqlite` crate
- **Config:** TOML (`toml` crate)
- **Logging:** `tracing` + `tracing-subscriber`

### Media Processing

- **FFmpeg:** 6.0+ (com `libx264`, `libfdk_aac`, `loudnorm` filter)
- **Probing:** `ffprobe` via `std::process::Command`
- **Download:** `aria2c` ou `reqwest` + `tokio::fs`

### Browser Automation

- **Browser:** Chromium (headless)
- **Protocol:** Chrome DevTools Protocol (CDP)
- **Crate:** `chromiumoxide` ou `headless_chrome`
- **Math:** `bezier` crate para curvas, `rand` para jitter

### Networking

- **HTTP client:** `reqwest` + `rustls`
- **RTMP ingest:** NGINX-RTMP module
- **HLS serving:** NGINX HTTP
- **VPN:** Tailscale (WireGuard-based)
- **CDN:** Cloudflare (primary), Backblaze B2 + Bunny CDN (backup)

### Deployment

- **OS:** macOS (primary), Linux (backup)
- **Process management:** `systemd` (Linux) ou `launchd` (macOS)
- **Orchestration:** Shell scripts + Rust daemons

---

## 📁 ESTRUTURA DE ARQUIVOS

### Código Fonte (Rust workspace)

```
/Users/voulezvous/voulezvous-tv-industrial/
├── Cargo.toml              # Workspace root
├── Cargo.lock
├── .gitignore
├── README.md
├── AGENTS.md               # Este documento
├── VVTV INDUSTRIAL DOSSIER.md  # Spec completa
│
├── crates/
│   ├── vvtv-common/        # Shared types, utils
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── config.rs   # Config loading (TOML)
│   │       ├── types.rs    # Plan, Asset, etc
│   │       └── db.rs       # SQLite helpers
│   │
│   ├── vvtv-planner/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs     # CLI entry point
│   │       ├── lib.rs
│   │       ├── plan.rs     # Plan struct + methods
│   │       ├── store.rs    # SqlitePlanStore
│   │       └── state.rs    # State machine
│   │
│   ├── vvtv-curator/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── lib.rs
│   │       ├── browser.rs  # Browser wrapper
│   │       ├── human_sim.rs # Mouse, scroll, keyboard
│   │       ├── pbd.rs      # Play-Before-Download
│   │       └── cdp.rs      # CDP helpers
│   │
│   ├── vvtv-processor/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── lib.rs
│   │       ├── download.rs
│   │       ├── ffmpeg.rs   # FFmpeg wrappers
│   │       ├── transcode.rs
│   │       ├── loudnorm.rs
│   │       ├── hls.rs      # HLS packaging
│   │       └── qc.rs       # Quality control
│   │
│   ├── vvtv-broadcaster/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── lib.rs
│   │       ├── queue.rs    # Playout queue
│   │       ├── encoder.rs  # FFmpeg RTMP encoder
│   │       └── failover.rs
│   │
│   ├── vvtv-monitor/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── lib.rs
│   │       ├── capture.rs  # Frame capture
│   │       └── qc.rs       # Live QC
│   │
│   ├── vvtv-watchdog/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── lib.rs
│   │       └── health.rs   # Health checks
│   │
│   └── vvtv-economy/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── lib.rs
│           ├── ledger.rs   # Economy ledger
│           └── analytics.rs
│
└── tests/
    ├── e2e_test.sh         # End-to-end test script
    └── integration/
        ├── test_curator.rs
        ├── test_processor.rs
        └── test_broadcaster.rs
```

### Sistema Runtime (/vvtv/)

Estrutura completa definida em **Diagrama 4** do dossiê (linhas 3880-3960).

---

## 🎨 PADRÕES DE CÓDIGO

### Rust Style Guide

- **Formatting:** Use `rustfmt` (padrão)
- **Linting:** Use `clippy` e corrija todos warnings
- **Naming:**
  - `snake_case` para funções, variáveis, módulos
  - `PascalCase` para structs, enums, traits
  - `SCREAMING_SNAKE_CASE` para constantes
- **Error Handling:**
  - Use `Result<T, anyhow::Error>` para erros
  - Use `?` operator para propagação
  - Log erros antes de retornar: `error!("Failed to X: {}", e)`
- **Async:**
  - Use `async/await` com `tokio`
  - Prefira `tokio::spawn` para tarefas concorrentes
  - Use `Arc<Mutex<T>>` para shared state (minimize locks)

### Logging Conventions

```rust
use tracing::{debug, info, warn, error};

// DEBUG: Informações detalhadas para debugging
debug!("Processing plan: {}", plan_id);

// INFO: Eventos importantes normais
info!("Plan {} transitioned to status: {}", plan_id, status);

// WARN: Situações anormais mas recuperáveis
warn!("Buffer below target: {}h (target: 6h)", buffer_hours);

// ERROR: Falhas que requerem atenção
error!("Failed to transcode plan {}: {}", plan_id, e);
```

### Configuration Pattern

Todos os módulos devem:
1. Ler config de `/vvtv/system/<module>.toml`
2. Usar `serde` + `toml` crate para parsing
3. Validar config no startup (fail fast)
4. Logar config aplicada (exceto secrets)

```rust
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct ProcessorConfig {
    pub cache_dir: PathBuf,
    pub preset: String,
    pub crf: u8,
    // ...
}

impl ProcessorConfig {
    pub fn load() -> Result<Self> {
        let content = std::fs::read_to_string("/vvtv/system/processor.toml")?;
        let config: Self = toml::from_str(&content)?;
        config.validate()?;
        info!("Config loaded: {:?}", config);
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.crf > 51 {
            return Err(anyhow!("Invalid CRF: must be 0-51"));
        }
        // ... outras validações
        Ok(())
    }
}
```

### Database Pattern

Use `rusqlite` com prepared statements:

```rust
use rusqlite::{Connection, params};

pub struct SqlitePlanStore {
    conn: Connection,
}

impl SqlitePlanStore {
    pub fn create_plan(&mut self, url: &str) -> Result<Plan> {
        let plan_id = uuid::Uuid::new_v4().to_string();
        
        self.conn.execute(
            "INSERT INTO plans (plan_id, source_url, status, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![plan_id, url, "planned", Utc::now()],
        )?;
        
        Ok(self.get_plan(&plan_id)?.unwrap())
    }

    pub fn get_plan(&self, id: &str) -> Result<Option<Plan>> {
        let mut stmt = self.conn.prepare(
            "SELECT plan_id, source_url, status, created_at FROM plans WHERE plan_id = ?1"
        )?;
        
        let plan = stmt.query_row(params![id], |row| {
            Ok(Plan {
                plan_id: row.get(0)?,
                source_url: row.get(1)?,
                status: row.get(2)?,
                created_at: row.get(3)?,
            })
        }).optional()?;
        
        Ok(plan)
    }
}
```

---

## ✅ VALIDAÇÃO E TESTES

### Testes Unitários

Cada módulo deve ter testes unitários em `src/<module>/tests.rs` ou `#[cfg(test)]`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_state_transition() {
        let mut plan = Plan::new("https://example.com/video");
        assert_eq!(plan.status, PlanStatus::Planned);
        
        plan.transition_to(PlanStatus::Selected).unwrap();
        assert_eq!(plan.status, PlanStatus::Selected);
        
        // Transição inválida deve falhar
        assert!(plan.transition_to(PlanStatus::Played).is_err());
    }
}
```

Rodar testes:
```bash
cargo test --all
```

### Testes de Integração

Testes que envolvem múltiplos módulos ou I/O externo em `tests/integration/`:

```rust
// tests/integration/test_processor.rs
use vvtv_processor::Processor;
use tempfile::tempdir;

#[tokio::test]
async fn test_transcode_workflow() {
    let temp_dir = tempdir().unwrap();
    let config = ProcessorConfig {
        cache_dir: temp_dir.path().to_path_buf(),
        preset: "ultrafast".to_string(),
        crf: 23,
        // ...
    };
    
    let mut processor = Processor::new(config);
    
    // Usar vídeo de teste
    let input = PathBuf::from("tests/fixtures/sample.mp4");
    let output = processor.transcode(&input).await.unwrap();
    
    // Validar output existe e é válido
    assert!(output.exists());
    let probe = ffprobe(&output).await.unwrap();
    assert_eq!(probe.codec, "h264");
}
```

### Teste End-to-End

Script bash que valida todo o pipeline:

```bash
#!/bin/bash
# tests/e2e_test.sh

set -e

echo "🧪 VVTV E2E Test"

# 1. Setup
export VVTV_TEST_MODE=1
./scripts/setup_test_env.sh

# 2. Criar plan de teste
PLAN_ID=$(vvtv-planner create --url="https://example.com/test-video")
echo "✅ Plan created: $PLAN_ID"

# 3. Simular seleção
vvtv-planner select --id=$PLAN_ID
echo "✅ Plan selected"

# 4. Processar (mock PBD + transcode)
vvtv-processor run --plan=$PLAN_ID --mock-pbd
echo "✅ Processing complete"

# 5. Verificar asset em /storage/ready/
if [ -f "/vvtv/storage/ready/$PLAN_ID/master.mp4" ]; then
    echo "✅ Asset staged"
else
    echo "❌ Asset not found"
    exit 1
fi

# 6. Verificar na fila
QUEUE_COUNT=$(sqlite3 /vvtv/data/queue.sqlite "SELECT COUNT(*) FROM playout_queue WHERE plan_id='$PLAN_ID'")
if [ "$QUEUE_COUNT" -eq "1" ]; then
    echo "✅ Asset in queue"
else
    echo "❌ Asset not in queue"
    exit 1
fi

echo "🎉 E2E Test PASSED"
```

---

## 🚀 DEPLOYMENT

### Preparação do Ambiente

1. **Provisionar servidor:**
   - Mac Mini M1 (primary) ou Linux x86_64
   - 16 GB RAM mínimo, 32 GB recomendado
   - 512 GB SSD mínimo, 1 TB recomendado
   - 1 Gbps internet

2. **Instalar dependências:**
   ```bash
   # macOS
   brew install ffmpeg sqlite3 nginx-full aria2 tailscale rust
   
   # Linux (Debian/Ubuntu)
   apt-get install -y ffmpeg sqlite3 nginx-extras aria2 curl
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   curl -fsSL https://tailscale.com/install.sh | sh
   ```

3. **Criar estrutura:**
   ```bash
   sudo mkdir -p /vvtv/{system,data,cache,storage,broadcast,docs,monitor,vault}
   # ... (ver Quick Start Guide)
   ```

### Build e Instalação

```bash
# 1. Clone repo
cd /Users/voulezvous/voulezvous-tv-industrial/

# 2. Build release
cargo build --release --all

# 3. Instalar binários
sudo cp target/release/vvtv-* /vvtv/system/bin/

# 4. Copiar configs
sudo cp configs/*.toml /vvtv/system/

# 5. Copiar scripts
sudo cp scripts/*.sh /vvtv/system/bin/
sudo chmod +x /vvtv/system/bin/*.sh

# 6. Setup databases
sqlite3 /vvtv/data/plans.sqlite < schemas/plans.sql
sqlite3 /vvtv/data/queue.sqlite < schemas/queue.sql
sqlite3 /vvtv/data/metrics.sqlite < schemas/metrics.sql
sqlite3 /vvtv/data/economy.sqlite < schemas/economy.sql

# 7. Ajustar permissions
sudo chown -R vvtv:vvtv /vvtv
```

### Systemd Services (Linux)

```ini
# /etc/systemd/system/vvtv-broadcaster.service
[Unit]
Description=VVTV Broadcaster
After=network.target nginx.service

[Service]
Type=simple
User=vvtv
WorkingDirectory=/vvtv
ExecStart=/vvtv/system/bin/vvtv-broadcaster daemon
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Habilitar e iniciar:
```bash
sudo systemctl enable vvtv-broadcaster
sudo systemctl start vvtv-broadcaster
sudo systemctl status vvtv-broadcaster
```

Criar services similares para: `vvtv-processor`, `vvtv-curator`, `vvtv-monitor`, `vvtv-watchdog`.

### Monitoring e Logging

```bash
# Logs via journalctl (systemd)
journalctl -u vvtv-broadcaster -f

# Ou logs diretos
tail -f /vvtv/system/logs/broadcast.log

# Dashboard
open http://localhost:9000/dashboard.html
```

---

## 📚 REFERÊNCIAS

### Documentos Principais

1. **VVTV INDUSTRIAL DOSSIER.md** — Especificação técnica completa (5,580 linhas)
   - Blocos I-IX: Arquitetura detalhada
   - Apêndices A-H: Configs, scripts, benchmarks, troubleshooting

2. **Este documento (AGENTS.md)** — Guia de implementação para agentes

### Recursos Externos

**Rust:**
- [The Rust Book](https://doc.rust-lang.org/book/)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [Rusqlite Docs](https://docs.rs/rusqlite/)

**FFmpeg:**
- [FFmpeg Documentation](https://ffmpeg.org/documentation.html)
- [EBU R128 Loudness](https://tech.ebu.ch/docs/r/r128.pdf)
- [HLS Specification (Apple)](https://developer.apple.com/streaming/)

**Browser Automation:**
- [Chrome DevTools Protocol](https://chromedevtools.github.io/devtools-protocol/)
- [Chromiumoxide Crate](https://docs.rs/chromiumoxide/)

**CDN:**
- [Cloudflare Workers Docs](https://developers.cloudflare.com/workers/)
- [NGINX-RTMP Module](https://github.com/arut/nginx-rtmp-module)

### Seções Críticas do Dossiê

- **Quick Start Guide:** linhas 150-300
- **Play-Before-Download:** linhas 500-600
- **Human Simulation:** linhas 450-550
- **FFmpeg Pipeline:** linhas 750-850
- **State Machine:** linhas 3819-3878
- **Troubleshooting:** linhas 5142-5573
- **Benchmarks:** linhas 4956-5139

---

## 🎯 OBJETIVOS DE SUCESSO

### Critérios de Aceitação

Um sistema VVTV está **completo e funcional** quando:

✅ **Autonomia:**
- Roda 24/7 por 7+ dias sem intervenção humana
- Buffer mantém-se entre 4-8h consistentemente
- Watchdog auto-recupera de todas falhas comuns

✅ **Qualidade:**
- VMAF >85, SSIM >0.92 em 95% dos assets
- Loudness -14 LUFS ±1.5 em 100% dos assets
- <1% de frames pretos/congelados no stream

✅ **Resiliência:**
- Failover encoder <45s downtime
- Failover CDN <33s downtime
- Emergency loop ativa <2s quando buffer crítico

✅ **Performance:**
- Processa 8-10h de conteúdo por dia (Mac Mini M1)
- Latência viewer-to-origin: 5-9s
- CDN 99.9% uptime

✅ **Conformidade:**
- Nenhum conteúdo DRM processado
- Nenhum conteúdo CSAM processado
- Logs completos de todas ações (auditável)

### Métricas de Qualidade de Código

- ✅ Todos testes unitários passam (`cargo test`)
- ✅ Teste E2E passa
- ✅ Zero warnings do `clippy`
- ✅ Coverage >70% (medido com `cargo-tarpaulin`)
- ✅ Documentação inline para funções públicas (`///`)

---

## 💡 DICAS PARA AGENTES

### 1. **Comece Pequeno, Itere Rápido**
Não tente implementar tudo de uma vez. Siga as fases em ordem. Valide cada fase antes de prosseguir.

### 2. **Use Placeholders Inteligentes**
Se uma funcionalidade complexa está bloqueando, crie um mock simples:
```rust
// TODO: Implementar PBD real
async fn pbd_mock(_url: &str) -> Result<String> {
    Ok("https://mock-manifest.m3u8".to_string())
}
```

### 3. **Logue Tudo**
Quando debugando, logs são seus melhores amigos. Use `tracing` com níveis apropriados.

### 4. **Teste com Vídeos Curtos**
Para desenvolvimento, use vídeos de teste de 10-30s, não 1h. Acelera ciclos de teste.

### 5. **FFmpeg É Seu Amigo**
Aprenda a usar `ffmpeg` e `ffprobe` na linha de comando antes de wrappear em Rust. Teste comandos manualmente.

### 6. **SQLite É Simples**
Não over-engineer o schema. SQLite é rápido e confiável para este caso de uso.

### 7. **Resilience > Perfection**
Um sistema que se auto-recupera de 90% das falhas é melhor que um sistema perfeito que quebra irremediavelmente.

### 8. **Documente Decisões**
Se você tomar uma decisão de design diferente do dossiê, documente o porquê em comentários ou README.

### 9. **Security: Fail-Safe**
Se detectar DRM ou conteúdo ilegal, **abort imediatamente**. Log e alerte. Não tente processar.

### 10. **Leia o Dossiê Completo**
Este AGENTS.md é um guia de implementação, mas o **VVTV INDUSTRIAL DOSSIER.md** é a fonte de verdade para specs detalhadas.

---

## 🚦 STATUS TRACKING

Ao implementar, mantenha um checklist de status:

```markdown
## Implementation Status

### Phase 0: Setup
- [x] Directory structure
- [x] Dependencies installed
- [x] NGINX configured
- [x] SQLite schemas
- [ ] Config files

### Phase 1: Planner
- [x] Plan struct
- [x] SqlitePlanStore
- [x] State machine
- [ ] CLI tool
- [ ] Unit tests

### Phase 2: Curator
- [ ] Browser wrapper
- [ ] Human simulation
- [ ] PBD implementation
- [ ] CDP network intercept
- [ ] Tests

... (continue para todas as fases)
```

---

## 📞 SUPORTE

Se você é um **agente de IA** implementando este sistema e encontrar ambiguidades ou questões não resolvidas:

1. **Consulte o dossiê:** `VVTV INDUSTRIAL DOSSIER.md` tem 5,580 linhas de specs
2. **Consulte os apêndices:** Configs, scripts, troubleshooting estão todos lá
3. **Use bom senso:** Se uma decisão menor não está especificada, escolha a opção mais simples e resiliente
4. **Documente:** Se você fizer uma escolha, documente-a para humanos revisarem

---

## 🏁 CONCLUSÃO

Este sistema é **ambicioso mas realizável**. Cada componente é bem definido e testável isoladamente. A chave é:

- **Implementação incremental:** Fase por fase
- **Validação constante:** Cada fase deve funcionar antes de prosseguir
- **Resilience-first:** Falhas vão acontecer, prepare-se para elas
- **Qualidade técnica:** Siga os padrões, escreva testes, logue tudo

**Boa sorte, agente! 🚀**

```
logline sign --key voulezvous_foundation.pem AGENTS.md
sha256: [pending_signature]
```

---

> *"Código que se auto-recupera é código que vive."*  
> — VoulezVous Foundation, 2025

