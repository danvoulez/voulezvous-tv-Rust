# 🗺️ ROADMAP: PRs D7–D11 — Completando Epic D & Otimizações

> **Planejamento Completo das Implementações Pendentes**  
> **Autor:** System Analysis  
> **Data:** 2025-10-21  
> **Status:** Proposta aprovada para implementação

---

## 📋 ÍNDICE

1. [Visão Geral](#visão-geral)
2. [PR D7 — Discovery Loop Completo](#pr-d7--discovery-loop-completo)
3. [PR D8 — Error Handling & Antibot Resilience](#pr-d8--error-handling--antibot-resilience)
4. [PR D9 — QA Tooling & Validation](#pr-d9--qa-tooling--validation)
5. [PR D10 — Performance Optimizations](#pr-d10--performance-optimizations)
6. [PR D11 — Documentation Update](#pr-d11--documentation-update)
7. [Cronograma de Implementação](#cronograma-de-implementação)
8. [Critérios de Aceitação Global](#critérios-de-aceitação-global)

---

## 🎯 VISÃO GERAL

### Status Atual
- **Epic D**: 66% completo (4/6 PRs originais)
- **Gaps identificados**: Discovery Loop, Error Handling, QA Tooling
- **Oportunidades**: Performance optimizations (VideoToolbox, SQLite WAL)
- **Débito técnico**: Documentação desatualizada

### Objetivo
Completar Epic D e otimizar sistema para **autonomia completa 24/7**.

### PRs Planejados

| PR | Título | Estimativa | Prioridade | Dependências |
|----|--------|------------|------------|--------------|
| **D7** | Discovery Loop Completo | 2-3 dias | 🔴 CRÍTICA | Nenhuma |
| **D8** | Error Handling & Antibot | 2 dias | 🔴 CRÍTICA | D7 |
| **D9** | QA Tooling & Validation | 1-2 dias | 🟡 ALTA | D7, D8 |
| **D10** | Performance Optimizations | 1 dia | 🟢 MÉDIA | Nenhuma |
| **D11** | Documentation Update | 0.5 dia | 🟢 MÉDIA | D7-D10 |

**Total:** 6.5-8.5 dias de desenvolvimento

---

## 📦 PR D7 — Discovery Loop Completo

### 🎯 Objetivo

Implementar o ciclo completo de descoberta automática de conteúdo conforme VVTV INDUSTRIAL DOSSIER.md (linhas 614-620).

### 📋 Descrição

**Gap identificado:** O sistema atual implementa PBD (Play-Before-Download) para **URLs individuais**, mas **não tem capacidade de descobrir URLs autonomamente** via busca na web.

**Solução:** Implementar o "Ciclo do worker":
```
init_profile → open_start_url → simulate_human_idle(3–8s) → search(term) →
scroll_collect(results ~ N) → open_candidate → play_before_download() →
capture_target() → record_plan() → close_tab → next
```

### 🔧 Componentes a Implementar

#### 1. ContentSearcher (`searcher.rs`)
```rust
pub struct ContentSearcher {
    config: Arc<SearchConfig>,
    automation: Arc<BrowserAutomation>,
}

impl ContentSearcher {
    pub async fn search(&self, query: &str) -> BrowserResult<Vec<Candidate>> {
        // 1. Navigate to search engine (Google/Bing/DuckDuckGo)
        // 2. Type query with human simulation
        // 3. Submit search
        // 4. Scroll and collect results (multiple iterations)
        // 5. Parse DOM to extract candidate URLs
        // 6. Filter video-likely candidates
    }
}
```

**Features:**
- ✅ Multi-engine support (Google, Bing, DuckDuckGo)
- ✅ Human-simulated typing and scrolling
- ✅ JavaScript-based result extraction
- ✅ Domain whitelist filtering
- ✅ Video content heuristics

#### 2. DiscoveryLoop (`discovery_loop.rs`)
```rust
pub struct DiscoveryLoop {
    searcher: ContentSearcher,
    pbd: PlayBeforeDownload,
    planner: Arc<Mutex<dyn PlanStore>>,
    config: DiscoveryConfig,
}

impl DiscoveryLoop {
    pub async fn run(&mut self, query: &str) -> BrowserResult<DiscoveryStats> {
        // 1. Search for candidates
        // 2. Rate-limit between candidates
        // 3. For each: PBD + create plan
        // 4. Return statistics
    }
}
```

**Features:**
- ✅ Rate limiting (configurable delays)
- ✅ Error handling (continue on failure)
- ✅ Statistics tracking
- ✅ Dry-run mode

#### 3. CLI Integration
```bash
vvtvctl discover --query="creative commons documentary" --max-plans=10
```

**Arguments:**
- `--query`: Search term
- `--max-plans`: Maximum plans to create
- `--search-engine`: google|bing|duckduckgo
- `--dry-run`: Test without creating plans
- `--debug`: Verbose logging

#### 4. Configuration (`browser.toml`)
```toml
[discovery]
search_engine = "google"
search_delay_ms = [2000, 5000]
scroll_iterations = 3
max_results_per_search = 20
candidate_delay_ms = [8000, 15000]
filter_domains = ["youtube.com", "vimeo.com", "dailymotion.com"]
```

### ✅ Critérios de Aceitação

**Funcional:**
- [ ] ContentSearcher busca em 3 engines (Google, Bing, DDG)
- [ ] Scroll funciona e coleta múltiplos resultados (>10)
- [ ] Filtra candidatos relevantes (video detection >80% accuracy)
- [ ] DiscoveryLoop processa múltiplos candidatos com rate limiting
- [ ] PLANs são criados corretamente no database
- [ ] Dry-run funciona sem side-effects

**Qualidade:**
- [ ] Zero bot detection (smoke test manual)
- [ ] Código compila sem warnings (`cargo clippy`)
- [ ] Formatação correta (`cargo fmt`)
- [ ] Logs estruturados com `tracing`

**Testes:**
- [ ] Unit tests para ContentSearcher (parsing, filtering)
- [ ] Unit tests para DiscoveryLoop (rate limiting, error handling)
- [ ] Integration test (mock browser, predefined HTML)
- [ ] E2E test manual (real browser, real search)

**Documentação:**
- [ ] Docstrings em todas funções públicas
- [ ] README atualizado com exemplo de uso
- [ ] CHANGELOG entry

### 📁 Arquivos Criados/Modificados

**Novos:**
- `vvtv-core/src/browser/searcher.rs` (~400 linhas)
- `vvtv-core/src/browser/discovery_loop.rs` (~300 linhas)
- `vvtv-core/tests/discovery_loop_test.rs` (~200 linhas)

**Modificados:**
- `vvtv-core/src/browser/mod.rs` (exports)
- `vvtv-core/src/config/browser.toml` (+discovery section)
- `vvtvctl/src/commands/mod.rs` (+discover command)
- `vvtvctl/src/commands/discover.rs` (novo)

**Documentação:**
- `SPEC_PR_D7_DISCOVERY_LOOP.md` (já existe, 967 linhas)

### 🧪 Plano de Testes

#### Test 1: Unit - Search Engine URL Generation
```rust
#[test]
fn test_google_search_url() {
    let config = SearchConfig { search_engine: SearchEngine::Google, /* ... */ };
    let searcher = ContentSearcher::new(/* ... */);
    assert_eq!(searcher.get_search_url(), "https://www.google.com");
}
```

#### Test 2: Unit - Video Content Detection
```rust
#[test]
fn test_video_heuristic() {
    let candidate = Candidate {
        url: "https://youtube.com/watch?v=abc".to_string(),
        title: Some("Amazing Documentary".to_string()),
        /* ... */
    };
    assert!(searcher.is_likely_video(&candidate));
}
```

#### Test 3: Integration - Mock Search Flow
```rust
#[tokio::test]
async fn test_search_mock() {
    let mock_browser = MockBrowserAutomation::with_html(GOOGLE_RESULTS_HTML);
    let searcher = ContentSearcher::new(config, Arc::new(mock_browser));
    let results = searcher.search("test").await.unwrap();
    assert!(results.len() > 5);
}
```

#### Test 4: E2E - Real Discovery (Manual)
```bash
# Smoke test manual (requer browser + network)
vvtvctl discover --query="creative commons music video" \
  --max-plans=3 --dry-run --debug

# Validar:
# - Navegação para Google funciona
# - Busca é digitada corretamente
# - Resultados são coletados
# - Candidatos são filtrados
# - Nenhum PLAN é criado (dry-run)
```

### 📊 Métricas de Sucesso

| Métrica | Target | Como Medir |
|---------|--------|------------|
| Candidates per search | ≥10 | `searcher.search()` output |
| Video detection accuracy | >80% | Manual review (20 samples) |
| PBD success rate | >70% | `plans_created / candidates_processed` |
| Search time | <15s | `searcher.search()` duration |
| Discovery time (5 candidates) | <3min | End-to-end test |
| Bot detection rate | 0% | Monitor for CAPTCHAs |

### ⏱️ Estimativa de Tempo

- **Dia 1 (4h):** ContentSearcher (Google + parsing)
- **Dia 1 (4h):** Multi-engine support + filtering
- **Dia 2 (4h):** DiscoveryLoop + CLI integration
- **Dia 2 (4h):** Unit tests + integration tests
- **Dia 3 (2h):** E2E test + bug fixes
- **Dia 3 (2h):** Documentation + code review

**Total: 20h (2.5 dias)**

### 🔗 Dependências

**Crates novos:**
- Nenhuma (usar existing: chromiumoxide, tokio, serde, rand)

**Código existente:**
- ✅ BrowserAutomation (já implementado)
- ✅ HumanMotionController (já implementado)
- ✅ PlayBeforeDownload (já implementado)
- ✅ SqlitePlanStore (já implementado)

---

## 📦 PR D8 — Error Handling & Antibot Resilience

### 🎯 Objetivo

Tornar o sistema resistente a bot detection e capaz de se recuperar de falhas automaticamente.

### 📋 Descrição

**Gap identificado:** Sistema atual não tem:
- Fingerprint masking (Canvas, WebGL, Audio)
- Retry logic estruturada
- IP rotation automática
- Categorização de erros

**Impacto:** Vulnerável a bloqueios de antibot, difícil debugging de falhas.

### 🔧 Componentes a Implementar

#### 1. Fingerprint Masking (`fingerprint.rs`)

**Canvas Fingerprinting:**
```rust
pub struct FingerprintMasker {
    config: FingerprintConfig,
}

impl FingerprintMasker {
    pub async fn inject_canvas_noise(&self, page: &Page) -> BrowserResult<()> {
        page.evaluate_on_new_document(r#"
            const originalToDataURL = HTMLCanvasElement.prototype.toDataURL;
            HTMLCanvasElement.prototype.toDataURL = function() {
                const ctx = this.getContext('2d');
                const imageData = ctx.getImageData(0, 0, this.width, this.height);
                // Add subtle noise
                for (let i = 0; i < imageData.data.length; i += 4) {
                    imageData.data[i] += Math.floor(Math.random() * 10) - 5;
                }
                ctx.putImageData(imageData, 0, 0);
                return originalToDataURL.apply(this, arguments);
            };
        "#).await?;
        Ok(())
    }
}
```

**WebGL Fingerprinting:**
```rust
pub async fn mask_webgl(&self, page: &Page) -> BrowserResult<()> {
    page.evaluate_on_new_document(r#"
        const getParameter = WebGLRenderingContext.prototype.getParameter;
        WebGLRenderingContext.prototype.getParameter = function(param) {
            // Randomize vendor/renderer
            if (param === 37445) return 'Intel Inc.';
            if (param === 37446) return 'Intel Iris OpenGL Engine';
            return getParameter.apply(this, arguments);
        };
    "#).await?;
    Ok(())
}
```

**Audio Context:**
```rust
pub async fn mask_audio_context(&self, page: &Page) -> BrowserResult<()> {
    page.evaluate_on_new_document(r#"
        const AudioContext = window.AudioContext || window.webkitAudioContext;
        const originalGetChannelData = AudioBuffer.prototype.getChannelData;
        AudioBuffer.prototype.getChannelData = function(channel) {
            const data = originalGetChannelData.call(this, channel);
            // Add noise
            for (let i = 0; i < data.length; i++) {
                data[i] += Math.random() * 0.0001 - 0.00005;
            }
            return data;
        };
    "#).await?;
    Ok(())
}
```

#### 2. Error Categorization (`error_handler.rs`)

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum BrowserErrorCategory {
    PlayerNotFound,
    HDUnavailable,
    BotDetection,
    ManifestInvalid,
    NetworkTimeout,
    Unknown(String),
}

pub struct ErrorCategorizer;

impl ErrorCategorizer {
    pub fn categorize(error: &BrowserError) -> BrowserErrorCategory {
        // Classify error by message/context
        match error {
            BrowserError::ElementNotFound(msg) if msg.contains("play") => {
                BrowserErrorCategory::PlayerNotFound
            }
            BrowserError::PlaybackFailed(msg) if msg.contains("captcha") => {
                BrowserErrorCategory::BotDetection
            }
            // ... more patterns
            _ => BrowserErrorCategory::Unknown(error.to_string()),
        }
    }
}
```

#### 3. Retry Logic com Backoff (`retry.rs`)

```rust
pub struct RetryPolicy {
    max_attempts: usize,
    delays: Vec<Duration>,  // [10min, 45min, 24h]
    backoff: BackoffStrategy,
}

pub async fn with_retry<F, T>(
    policy: &RetryPolicy,
    mut operation: F,
) -> Result<T>
where
    F: FnMut() -> BoxFuture<'static, Result<T>>,
{
    let mut attempt = 0;
    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                attempt += 1;
                if attempt >= policy.max_attempts {
                    return Err(e);
                }
                
                let delay = policy.delays.get(attempt - 1)
                    .cloned()
                    .unwrap_or_else(|| Duration::from_secs(3600 * 24));
                
                warn!("Attempt {}/{} failed: {}. Retrying in {:?}", 
                      attempt, policy.max_attempts, e, delay);
                
                tokio::time::sleep(delay).await;
            }
        }
    }
}
```

#### 4. IP Rotation via Tailscale Exit Nodes

```rust
pub struct IpRotator {
    exit_nodes: Vec<String>,
    current_index: usize,
}

impl IpRotator {
    pub async fn rotate(&mut self) -> Result<()> {
        self.current_index = (self.current_index + 1) % self.exit_nodes.len();
        let exit_node = &self.exit_nodes[self.current_index];
        
        // Use Tailscale CLI to switch exit node
        Command::new("tailscale")
            .args(&["set", "--exit-node", exit_node])
            .status()
            .await?;
        
        info!("Switched to exit node: {}", exit_node);
        
        // Wait for IP change
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        Ok(())
    }
}
```

#### 5. Failure Logging

```rust
// curator_failures.log
pub struct FailureLogger {
    file: File,
}

impl FailureLogger {
    pub async fn log_failure(&mut self, failure: &BrowserFailure) -> Result<()> {
        let entry = serde_json::json!({
            "timestamp": Utc::now().to_rfc3339(),
            "url": failure.url,
            "category": format!("{:?}", failure.category),
            "error_message": failure.error.to_string(),
            "attempt": failure.attempt,
            "action_taken": format!("{:?}", failure.action),
        });
        
        writeln!(self.file, "{}", entry)?;
        self.file.flush()?;
        
        Ok(())
    }
}
```

### ✅ Critérios de Aceitação

- [ ] Canvas/WebGL/Audio fingerprinting implementado
- [ ] Testes manuais: Nenhum bot detection em 20 searches
- [ ] Error categorization funciona (5 categorias principais)
- [ ] Retry logic com backoff (3 tentativas configuráveis)
- [ ] IP rotation funciona (Tailscale exit nodes)
- [ ] Failure log estruturado (`curator_failures.log`)
- [ ] Métricas registradas em `metrics.sqlite`

### 📁 Arquivos

**Novos:**
- `vvtv-core/src/browser/fingerprint.rs` (~300 linhas)
- `vvtv-core/src/browser/error_handler.rs` (~200 linhas)
- `vvtv-core/src/browser/retry.rs` (~150 linhas)
- `vvtv-core/src/browser/ip_rotator.rs` (~100 linhas)

**Modificados:**
- `vvtv-core/src/browser/automation.rs` (integrate fingerprint masking)
- `vvtv-core/src/config/browser.toml` (+fingerprint, retry sections)

### ⏱️ Estimativa: 2 dias (16h)

---

## 📦 PR D9 — QA Tooling & Validation

### 🎯 Objetivo

Ferramentas para validar human simulation e diagnosticar falhas.

### 🔧 Componentes

#### 1. Smoke Test Runner
```bash
vvtvctl qa smoke-test --domain=youtube.com --headed
```

**Features:**
- Headed/headless modes
- Per-domain test profiles
- Screenshot capture
- Success/failure reporting

#### 2. Video Recording
```rust
pub struct SessionRecorder {
    output_dir: PathBuf,
}

impl SessionRecorder {
    pub async fn record_session(&self, page: &Page, duration: Duration) -> Result<PathBuf> {
        // Use ffmpeg to record browser window
        let output = self.output_dir.join(format!("session_{}.mp4", Utc::now().timestamp()));
        
        Command::new("ffmpeg")
            .args(&[
                "-f", "avfoundation",
                "-i", "1",  // Screen capture
                "-t", &duration.as_secs().to_string(),
                "-c:v", "libx264",
                output.to_str().unwrap(),
            ])
            .status()
            .await?;
        
        Ok(output)
    }
}
```

#### 3. Metrics Dashboard
```rust
// Simple HTML dashboard
pub fn generate_dashboard(metrics: &BrowserMetrics) -> String {
    format!(r#"
        <!DOCTYPE html>
        <html>
        <head><title>VVTV Browser Metrics</title></head>
        <body>
            <h1>Browser Automation Metrics</h1>
            <table>
                <tr><td>PBD Success Rate</td><td>{:.1}%</td></tr>
                <tr><td>Avg Search Time</td><td>{:.1}s</td></tr>
                <tr><td>Bot Detection</td><td>{}</td></tr>
                <tr><td>Proxy Rotations</td><td>{}</td></tr>
            </table>
        </body>
        </html>
    "#,
        metrics.pbd_success_rate * 100.0,
        metrics.avg_search_time,
        metrics.bot_detections,
        metrics.proxy_rotations,
    )
}
```

### ✅ Critérios de Aceitação

- [ ] Smoke test runner funciona (headed + headless)
- [ ] Video recording captura sessões
- [ ] Metrics dashboard HTML gerado
- [ ] CLI: `vvtvctl qa --report` mostra estatísticas

### ⏱️ Estimativa: 1.5 dias (12h)

---

## 📦 PR D10 — Performance Optimizations

### 🎯 Objetivo

Otimizações de performance identificadas na análise.

### 🔧 Otimizações

#### 1. VideoToolbox Support (M1/M2)
```rust
// processor/transcode.rs
pub async fn transcode(&self, input: &Path) -> Result<PathBuf> {
    let use_hw = self.config.use_hardware_accel && is_apple_silicon();
    
    let codec = if use_hw { "h264_videotoolbox" } else { "libx264" };
    
    let mut cmd = Command::new("ffmpeg");
    if use_hw {
        cmd.args(&["-hwaccel", "videotoolbox"]);
    }
    // ... rest
}

fn is_apple_silicon() -> bool {
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    { true }
    #[cfg(not(all(target_arch = "aarch64", target_os = "macos")))]
    { false }
}
```

**Config:**
```toml
# processor.toml
[transcode]
use_hardware_accel = true  # NEW
```

#### 2. SQLite WAL Mode
```sql
-- Adicionar a schemas/plans.sql (e outros)
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA cache_size = -64000;  -- 64MB
PRAGMA temp_store = MEMORY;
PRAGMA mmap_size = 30000000000;  -- 30GB
```

**Script de otimização:**
```bash
#!/bin/bash
# scripts/optimize_databases.sh
for db in /vvtv/data/*.sqlite; do
  sqlite3 "$db" "PRAGMA optimize; VACUUM; ANALYZE;"
  echo "✅ Optimized $db"
done
```

#### 3. Shell Completions
```rust
// vvtvctl/src/main.rs
use clap::CommandFactory;
use clap_complete::{generate, Shell};

#[derive(Subcommand)]
enum Commands {
    // ... existing
    Completions {
        #[arg(value_enum)]
        shell: Shell,
    },
}

// Handler
fn generate_completions(shell: Shell) {
    generate(shell, &mut Cli::command(), "vvtvctl", &mut std::io::stdout());
}
```

### ✅ Critérios de Aceitação

- [ ] VideoToolbox funciona (benchmark: 3x+ speedup)
- [ ] SQLite WAL ativado (teste concorrência)
- [ ] Optimize script roda diariamente (cron)
- [ ] Shell completions funcionam (bash + zsh)

### ⏱️ Estimativa: 1 dia (8h)

---

## 📦 PR D11 — Documentation Update

### 🎯 Objetivo

Atualizar documentação para refletir mudanças dos PRs D7-D10.

### 📝 Documentos a Atualizar

#### 1. Tasklist.md
```markdown
## Epic D — Browser Automation & Human Simulation
- [x] PR D1 — Wrapper Chromium/CDP ✅
- [x] PR D2 — Motor de simulação humana ✅
- [x] PR D3 — Play-Before-Download (PBD) ✅
- [x] PR D4 — Metadata extraction ✅
- [x] PR D5 — Error handling & antibot resilience ✅  # ATUALIZADO
- [x] PR D6 — QA tooling para browser ✅  # ATUALIZADO
- [x] PR D7 — Discovery Loop completo ✅  # NOVO
- [x] PR D8 — (merged in D5)  # REORGANIZADO
- [x] PR D9 — (merged in D6)  # REORGANIZADO
```

#### 2. AGENTS.md
**Seção FASE 2: Módulo Browser Automation (Curator)**

Adicionar:
```markdown
### Discovery Loop (Novo)

**Implementar ciclo completo de descoberta:**

```rust
pub struct DiscoveryLoop {
    searcher: ContentSearcher,
    pbd: PlayBeforeDownload,
    planner: Arc<Mutex<dyn PlanStore>>,
}

impl DiscoveryLoop {
    pub async fn run(&mut self, query: &str) -> Result<DiscoveryStats> {
        // 1. Search web for candidates
        // 2. Filter video content
        // 3. PBD each candidate
        // 4. Create plans
    }
}
```

**Validação:**
```bash
vvtvctl discover --query="creative commons" --max-plans=5 --dry-run
```
```

#### 3. VVTV INDUSTRIAL DOSSIER.md
**Bloco II: Browser Automation e Simulação Humana (linhas 400-600)**

Adicionar seção:
```markdown
### Discovery Loop (Implementado)

O sistema agora possui capacidade de descoberta autônoma:

**Fluxo:**
1. Query de busca (ex: "creative commons documentary")
2. ContentSearcher busca em Google/Bing/DuckDuckGo
3. Extração de candidatos via DOM scraping
4. Filtragem heurística (video detection)
5. PBD em cada candidato (com rate limiting)
6. Criação de PLANs no database

**Configuração:**
```toml
[discovery]
search_engine = "google"
max_results_per_search = 20
candidate_delay_ms = [8000, 15000]
```

**CLI:**
```bash
vvtvctl discover --query="..." --max-plans=10
```
```

#### 4. README.md Principal
Adicionar Quick Start:
```markdown
## Quick Start — Discovery

Descobrir conteúdo automaticamente:

```bash
# Install
cargo build --release

# Discover
./target/release/vvtvctl discover \
  --query="creative commons documentary" \
  --max-plans=10

# View plans
./target/release/vvtvctl plan list

# Process
./target/release/vvtvctl processor run
```
```

#### 5. CHANGELOG.md
```markdown
## [Unreleased]

### Added (PR D7-D11)
- **Discovery Loop**: Automatic content discovery via web search
- **ContentSearcher**: Multi-engine search (Google, Bing, DuckDuckGo)
- **Antibot Resilience**: Canvas/WebGL/Audio fingerprint masking
- **Error Categorization**: Structured error handling with retry logic
- **IP Rotation**: Tailscale exit node switching
- **QA Tooling**: Smoke tests, video recording, metrics dashboard
- **VideoToolbox**: Hardware-accelerated transcode (3-5x speedup on M1/M2)
- **SQLite WAL**: Improved concurrency with Write-Ahead Logging
- **Shell Completions**: bash/zsh completions for vvtvctl

### Changed
- Tasklist.md: Updated Epic D status (100% complete)
- AGENTS.md: Added Discovery Loop section
- VVTV INDUSTRIAL DOSSIER.md: Documented Discovery implementation

### Fixed
- Bot detection vulnerability (fingerprint masking)
- Retry logic for transient failures
- SQLite concurrency bottleneck
```

### ✅ Critérios de Aceitação

- [ ] Tasklist.md atualizado (Epic D 100%)
- [ ] AGENTS.md atualizado (Fase 2 completa)
- [ ] VVTV INDUSTRIAL DOSSIER.md atualizado (Discovery Loop)
- [ ] README.md com Quick Start
- [ ] CHANGELOG.md com entries de D7-D10

### ⏱️ Estimativa: 0.5 dia (4h)

---

## 📅 CRONOGRAMA DE IMPLEMENTAÇÃO

### Semana 1

**Dia 1-2 (PR D7):**
- ✅ ContentSearcher implementation
- ✅ DiscoveryLoop implementation
- ✅ Unit + integration tests

**Dia 3-4 (PR D8):**
- ✅ Fingerprint masking
- ✅ Error categorization + retry logic
- ✅ IP rotation integration

**Dia 5 (PR D9):**
- ✅ Smoke test runner
- ✅ Video recording
- ✅ Metrics dashboard

### Semana 2

**Dia 1 (PR D10):**
- ✅ VideoToolbox integration
- ✅ SQLite WAL + optimizations
- ✅ Shell completions

**Dia 2 (PR D11):**
- ✅ Documentation updates
- ✅ CHANGELOG
- ✅ Final review

**Dia 3:**
- ✅ Integration testing
- ✅ Smoke tests
- ✅ Deploy to production

---

## ✅ CRITÉRIOS DE ACEITAÇÃO GLOBAL

### Funcionalidade

**Discovery Loop:**
- [ ] Sistema descobre conteúdo automaticamente via web search
- [ ] Cria 10+ PLANs por hora autonomamente
- [ ] Zero bot detection em 24h de operação

**Resilience:**
- [ ] Se recupera de 95% das falhas automaticamente
- [ ] IP rotation funciona quando necessário
- [ ] Retry logic limita attempts (max 3)

**Performance:**
- [ ] Transcode 3-5x mais rápido com VideoToolbox
- [ ] SQLite suporta 10+ concurrent readers sem lock
- [ ] Discovery loop processa 5 candidates em <3min

### Qualidade

- [ ] Zero warnings `cargo clippy`
- [ ] Formatação `cargo fmt`
- [ ] 100% testes passam
- [ ] Coverage >70%

### Documentação

- [ ] Tasklist.md 100% atualizado
- [ ] AGENTS.md reflete novo estado
- [ ] DOSSIER atualizado com Discovery
- [ ] README com Quick Start
- [ ] CHANGELOG completo

---

## 🎯 RESULTADO ESPERADO

Após implementação dos PRs D7-D11:

**✅ Epic D: 100% Completo**
- 7 PRs implementados (D1-D7)
- Discovery Loop funcionando
- Antibot resilience
- QA tooling
- Performance optimizada

**✅ Sistema Autonomo 24/7**
- Descobre conteúdo automaticamente
- Se recupera de falhas
- Opera sem intervenção humana
- Documentação completa

**✅ Pronto para Epic F (Broadcaster)**
- Foundation sólida
- Conteúdo sendo descoberto/processado
- Ready para playout

---

## 📊 DASHBOARD DE PROGRESSO

```
Epic D: Browser Automation & Human Simulation
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ 100%

✅ D1: Chromium/CDP Wrapper
✅ D2: Human Simulation
✅ D3: Play-Before-Download
✅ D4: Metadata Extraction
🔲 D5: Error Handling (→ D8)
🔲 D6: QA Tooling (→ D9)
🔲 D7: Discovery Loop
🔲 D10: Performance Optimizations
🔲 D11: Documentation Update

Estimativa total: 6.5-8.5 dias
Status: 🟡 4/7 completo (57%)
Next: Implementar PR D7
```

---

## 📞 APROVAÇÃO

**Roadmap completo pronto para implementação.**

**Próxima ação:** Começar implementação do PR D7 (Discovery Loop).

---

> *"Um roadmap claro é metade do caminho andado."*  
> — PR D7-D11 Roadmap, 2025-10-21

