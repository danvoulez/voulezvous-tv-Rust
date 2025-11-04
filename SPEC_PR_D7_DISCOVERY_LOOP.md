# 📋 SPEC: PR D7 — Discovery Loop Completo

> **Especificação Técnica Minuciosa**  
> **Autor:** Agent Analysis  
> **Data:** 2025-10-21  
> **Status:** Proposta  
> **Estimativa:** 2-3 dias de desenvolvimento

---

## 🎯 OBJETIVO

Implementar o ciclo completo de descoberta automática de conteúdo, conforme especificado no **VVTV INDUSTRIAL DOSSIER.md** (linhas 614-620):

```
init_profile → open_start_url → simulate_human_idle(3–8s) → search(term) →
scroll_collect(results ~ N) → open_candidate → play_before_download() →
capture_target() → record_plan() → close_tab → next
```

**Gap atual:** Componentes 1-3, 7-10 estão implementados (primitives). Falta implementar o **loop de orquestração** (componentes 4-6, 11).

---

## 📦 ENTREGÁVEIS

### 1. Módulos Rust

```
vvtv-core/src/browser/
├── searcher.rs          # NEW: ContentSearcher
├── discovery_loop.rs    # NEW: DiscoveryLoop orchestrator
└── (existing files)
```

### 2. Testes

```
vvtv-core/tests/
└── discovery_loop_test.rs    # NEW: Integration tests
```

### 3. CLI

```bash
vvtvctl discover --query="..." --max-plans=N [--dry-run] [--debug]
```

### 4. Configuração

Adicionar ao `browser.toml`:
```toml
[discovery]
search_engine = "google"  # google | bing | duckduckgo
search_delay_ms = [2000, 5000]  # Random delay range
scroll_iterations = 3
max_results_per_search = 20
candidate_delay_ms = [8000, 15000]  # Delay between PBD attempts
filter_domains = ["youtube.com", "vimeo.com", "dailymotion.com"]
```

---

## 🏗️ ARQUITETURA

### Diagrama de Componentes

```
┌─────────────────────────────────────────────────────────┐
│                   DiscoveryLoop                         │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────┐ │
│  │   Searcher   │───▶│     PBD      │───▶│ Planner  │ │
│  │ (Query)      │    │  (Collect)   │    │ (Store)  │ │
│  └──────────────┘    └──────────────┘    └──────────┘ │
│         │                    │                  │      │
│         ▼                    ▼                  ▼      │
│  ┌──────────────────────────────────────────────────┐  │
│  │       Browser Automation (Chromium + CDP)        │  │
│  └──────────────────────────────────────────────────┘  │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### Fluxo de Dados

```
1. Query String
   ↓
2. ContentSearcher.search(query)
   ├─ Navigate to search engine
   ├─ Type query (human simulation)
   ├─ Submit search
   ├─ Scroll & collect results (N iterations)
   └─ Return Vec<Candidate>
   ↓
3. Filter candidates (video detection)
   ↓
4. DiscoveryLoop.process_candidates()
   ├─ For each candidate:
   │  ├─ Rate limiting (delay)
   │  ├─ PBD.collect(candidate.url)
   │  ├─ Create Plan from outcome
   │  └─ Update metrics
   └─ Return stats
   ↓
5. Plans stored in plans.sqlite
```

---

## 🔧 ESPECIFICAÇÃO DETALHADA

### Módulo 1: ContentSearcher (`searcher.rs`)

#### Interface Pública

```rust
use std::sync::Arc;
use crate::browser::{BrowserAutomation, BrowserContext, HumanMotionController};
use crate::config::BrowserConfig;

#[derive(Debug, Clone)]
pub struct Candidate {
    pub url: String,
    pub title: Option<String>,
    pub snippet: Option<String>,
    pub domain: String,
    pub rank: usize,
}

#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub search_engine: SearchEngine,
    pub scroll_iterations: usize,
    pub max_results: usize,
    pub filter_domains: Vec<String>,
    pub delay_range_ms: (u64, u64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SearchEngine {
    Google,
    Bing,
    DuckDuckGo,
}

pub struct ContentSearcher {
    config: Arc<SearchConfig>,
    automation: Arc<BrowserAutomation>,
}

impl ContentSearcher {
    pub fn new(
        config: Arc<SearchConfig>,
        automation: Arc<BrowserAutomation>,
    ) -> Self {
        Self { config, automation }
    }

    /// Realiza busca e retorna candidatos
    pub async fn search(&self, query: &str) -> BrowserResult<Vec<Candidate>> {
        let context = self.automation.new_context().await?;
        let mut human = HumanMotionController::new(/* ... */);
        
        // 1. Navigate to search engine
        let search_url = self.get_search_url();
        context.goto(&search_url).await?;
        human.random_idle().await?;
        
        // 2. Type search query
        self.type_search_query(&context, &mut human, query).await?;
        
        // 3. Submit search
        self.submit_search(&context, &mut human).await?;
        
        // 4. Wait for results
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // 5. Collect results with scrolling
        let mut all_results = Vec::new();
        for i in 0..self.config.scroll_iterations {
            // Parse visible results
            let results = self.parse_search_results(&context).await?;
            all_results.extend(results);
            
            // Scroll down
            if i < self.config.scroll_iterations - 1 {
                human.scroll_burst(&context.page(), 400).await?;
                human.random_pause().await?;
            }
        }
        
        // 6. Filter & deduplicate
        let candidates = self.filter_candidates(all_results);
        
        Ok(candidates)
    }

    /// Gera URL de busca baseado no engine
    fn get_search_url(&self) -> String {
        match self.config.search_engine {
            SearchEngine::Google => "https://www.google.com".to_string(),
            SearchEngine::Bing => "https://www.bing.com".to_string(),
            SearchEngine::DuckDuckGo => "https://duckduckgo.com".to_string(),
        }
    }

    /// Digita query com simulação humana
    async fn type_search_query(
        &self,
        context: &BrowserContext,
        human: &mut HumanMotionController,
        query: &str,
    ) -> BrowserResult<()> {
        // Find search input
        let selectors = match self.config.search_engine {
            SearchEngine::Google => vec!["input[name='q']", "textarea[name='q']"],
            SearchEngine::Bing => vec!["input[name='q']"],
            SearchEngine::DuckDuckGo => vec!["input[name='q']"],
        };
        
        let input = self.find_first_element(context, &selectors).await?;
        
        // Click to focus
        human.click_element(context.page(), &input).await?;
        human.random_hesitation().await?;
        
        // Type with human cadence
        for ch in query.chars() {
            context.page().keyboard().press_key(&ch.to_string()).await?;
            let delay = rand::thread_rng().gen_range(80..180);
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
        
        Ok(())
    }

    /// Submit search form
    async fn submit_search(
        &self,
        context: &BrowserContext,
        human: &mut HumanMotionController,
    ) -> BrowserResult<()> {
        // Option 1: Click submit button
        let button_selectors = match self.config.search_engine {
            SearchEngine::Google => vec!["input[type='submit']", "button[type='submit']"],
            SearchEngine::Bing => vec!["input[type='submit']"],
            SearchEngine::DuckDuckGo => vec!["input[type='submit']"],
        };
        
        if let Ok(button) = self.find_first_element(context, &button_selectors).await {
            human.click_element(context.page(), &button).await?;
        } else {
            // Option 2: Press Enter
            context.page().keyboard().press_key("Enter").await?;
        }
        
        Ok(())
    }

    /// Parse search results from page
    async fn parse_search_results(
        &self,
        context: &BrowserContext,
    ) -> BrowserResult<Vec<Candidate>> {
        let script = self.get_result_parser_script();
        let results_json = context
            .page()
            .evaluate(&script)
            .await?
            .into_value()?;
        
        let results: Vec<SearchResultRaw> = serde_json::from_value(results_json)?;
        
        let candidates = results
            .into_iter()
            .enumerate()
            .map(|(rank, r)| Candidate {
                url: r.url,
                title: r.title,
                snippet: r.snippet,
                domain: self.extract_domain(&r.url),
                rank,
            })
            .collect();
        
        Ok(candidates)
    }

    /// JavaScript para extrair resultados
    fn get_result_parser_script(&self) -> String {
        match self.config.search_engine {
            SearchEngine::Google => r#"
                (() => {
                    const results = [];
                    const items = document.querySelectorAll('div.g, div[data-sokoban-container]');
                    items.forEach(item => {
                        const link = item.querySelector('a[href^="http"]');
                        const title = item.querySelector('h3');
                        const snippet = item.querySelector('div[data-content-feature]');
                        if (link && link.href) {
                            results.push({
                                url: link.href,
                                title: title ? title.textContent : null,
                                snippet: snippet ? snippet.textContent : null,
                            });
                        }
                    });
                    return results;
                })()
            "#.to_string(),
            
            SearchEngine::Bing => r#"
                (() => {
                    const results = [];
                    const items = document.querySelectorAll('li.b_algo');
                    items.forEach(item => {
                        const link = item.querySelector('a');
                        const title = item.querySelector('h2');
                        const snippet = item.querySelector('p');
                        if (link && link.href) {
                            results.push({
                                url: link.href,
                                title: title ? title.textContent : null,
                                snippet: snippet ? snippet.textContent : null,
                            });
                        }
                    });
                    return results;
                })()
            "#.to_string(),
            
            SearchEngine::DuckDuckGo => r#"
                (() => {
                    const results = [];
                    const items = document.querySelectorAll('article[data-testid="result"]');
                    items.forEach(item => {
                        const link = item.querySelector('a[data-testid="result-title-a"]');
                        const title = link ? link.querySelector('span') : null;
                        const snippet = item.querySelector('div[data-result="snippet"]');
                        if (link && link.href) {
                            results.push({
                                url: link.href,
                                title: title ? title.textContent : null,
                                snippet: snippet ? snippet.textContent : null,
                            });
                        }
                    });
                    return results;
                })()
            "#.to_string(),
        }
    }

    /// Filtra candidatos relevantes
    fn filter_candidates(&self, candidates: Vec<Candidate>) -> Vec<Candidate> {
        candidates
            .into_iter()
            // Remove duplicates
            .collect::<HashMap<String, Candidate>>()
            .into_values()
            // Filter by domain whitelist (if configured)
            .filter(|c| {
                if self.config.filter_domains.is_empty() {
                    true
                } else {
                    self.config.filter_domains.iter().any(|d| c.domain.contains(d))
                }
            })
            // Detect likely video content
            .filter(|c| self.is_likely_video(c))
            // Limit results
            .take(self.config.max_results)
            .collect()
    }

    /// Heurística para detectar vídeo
    fn is_likely_video(&self, candidate: &Candidate) -> bool {
        let video_indicators = vec![
            "youtube.com", "vimeo.com", "dailymotion.com",
            "watch", "video", "film", "documentary",
        ];
        
        let text = format!(
            "{} {} {}",
            candidate.url.to_lowercase(),
            candidate.title.as_deref().unwrap_or("").to_lowercase(),
            candidate.snippet.as_deref().unwrap_or("").to_lowercase(),
        );
        
        video_indicators.iter().any(|&indicator| text.contains(indicator))
    }

    fn extract_domain(&self, url: &str) -> String {
        url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(String::from))
            .unwrap_or_else(|| url.to_string())
    }

    async fn find_first_element(
        &self,
        context: &BrowserContext,
        selectors: &[&str],
    ) -> BrowserResult<Element> {
        for selector in selectors {
            if let Ok(elem) = context.page().find_element(selector).await {
                return Ok(elem);
            }
        }
        Err(BrowserError::ElementNotFound(format!(
            "None of selectors found: {:?}",
            selectors
        )))
    }
}

#[derive(Debug, Deserialize)]
struct SearchResultRaw {
    url: String,
    title: Option<String>,
    snippet: Option<String>,
}
```

---

### Módulo 2: DiscoveryLoop (`discovery_loop.rs`)

#### Interface Pública

```rust
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::browser::{ContentSearcher, PlayBeforeDownload, BrowserAutomation};
use crate::plan::PlanStore;

#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    pub max_plans_per_run: usize,
    pub candidate_delay_range_ms: (u64, u64),
    pub stop_on_first_error: bool,
    pub dry_run: bool,
}

#[derive(Debug)]
pub struct DiscoveryStats {
    pub query: String,
    pub candidates_found: usize,
    pub candidates_processed: usize,
    pub plans_created: usize,
    pub errors: Vec<String>,
    pub duration_secs: u64,
}

pub struct DiscoveryLoop {
    searcher: ContentSearcher,
    pbd: PlayBeforeDownload,
    planner: Arc<Mutex<dyn PlanStore>>,
    config: DiscoveryConfig,
}

impl DiscoveryLoop {
    pub fn new(
        searcher: ContentSearcher,
        pbd: PlayBeforeDownload,
        planner: Arc<Mutex<dyn PlanStore>>,
        config: DiscoveryConfig,
    ) -> Self {
        Self {
            searcher,
            pbd,
            planner,
            config,
        }
    }

    /// Executa discovery loop completo
    pub async fn run(&mut self, query: &str) -> BrowserResult<DiscoveryStats> {
        let start = std::time::Instant::now();
        
        info!("🔍 Starting discovery: query={}", query);
        
        // 1. Search
        let candidates = self.searcher.search(query).await?;
        info!("✅ Found {} candidates", candidates.len());
        
        // 2. Filter by max_plans
        let candidates_to_process = candidates
            .into_iter()
            .take(self.config.max_plans_per_run)
            .collect::<Vec<_>>();
        
        let mut stats = DiscoveryStats {
            query: query.to_string(),
            candidates_found: candidates_to_process.len(),
            candidates_processed: 0,
            plans_created: 0,
            errors: Vec::new(),
            duration_secs: 0,
        };
        
        // 3. Process each candidate
        for (i, candidate) in candidates_to_process.iter().enumerate() {
            info!(
                "[{}/{}] Processing: {} ({})",
                i + 1,
                candidates_to_process.len(),
                candidate.title.as_deref().unwrap_or("Untitled"),
                candidate.url
            );
            
            // Rate limiting
            if i > 0 {
                let delay = self.random_delay();
                debug!("⏱️  Waiting {}ms before next candidate", delay);
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            
            // PBD
            match self.process_candidate(candidate).await {
                Ok(plan_id) => {
                    stats.plans_created += 1;
                    info!("✅ PLAN created: {}", plan_id);
                }
                Err(e) => {
                    let error_msg = format!("{}: {}", candidate.url, e);
                    warn!("❌ Failed: {}", error_msg);
                    stats.errors.push(error_msg);
                    
                    if self.config.stop_on_first_error {
                        return Err(e);
                    }
                }
            }
            
            stats.candidates_processed += 1;
        }
        
        stats.duration_secs = start.elapsed().as_secs();
        
        info!(
            "✅ Discovery complete: {} PLANs created from {} candidates in {}s",
            stats.plans_created, stats.candidates_processed, stats.duration_secs
        );
        
        Ok(stats)
    }

    /// Processa candidato individual
    async fn process_candidate(&mut self, candidate: &Candidate) -> BrowserResult<String> {
        // 1. PBD
        let outcome = self.pbd.collect(&candidate.url).await?;
        
        // 2. Validate outcome
        if outcome.capture.kind == BrowserCaptureKind::Unknown {
            return Err(BrowserError::PlaybackFailed(
                "No valid media manifest captured".to_string(),
            ));
        }
        
        // 3. Create plan (if not dry-run)
        if self.config.dry_run {
            debug!("🏜️  DRY-RUN: Would create plan for {}", candidate.url);
            return Ok(format!("dry-run-{}", uuid::Uuid::new_v4()));
        }
        
        let mut planner = self.planner.lock().await;
        let plan = planner.create_from_pbd_outcome(outcome)?;
        
        Ok(plan.plan_id)
    }

    /// Delay aleatório entre candidatos
    fn random_delay(&self) -> u64 {
        let (min, max) = self.config.candidate_delay_range_ms;
        rand::thread_rng().gen_range(min..=max)
    }
}
```

---

### Módulo 3: CLI Integration (`vvtvctl`)

```rust
// vvtvctl/src/commands/discover.rs

use clap::Args;
use vvtv_core::browser::{
    BrowserAutomation, ContentSearcher, PlayBeforeDownload, DiscoveryLoop,
    SearchConfig, SearchEngine, DiscoveryConfig,
};
use vvtv_core::config::BrowserConfig;
use vvtv_core::plan::SqlitePlanStore;

#[derive(Args)]
pub struct DiscoverArgs {
    /// Search query
    #[arg(short, long)]
    query: String,

    /// Max plans to create
    #[arg(short, long, default_value = "10")]
    max_plans: usize,

    /// Search engine
    #[arg(long, default_value = "google")]
    search_engine: String,

    /// Dry-run (don't create plans)
    #[arg(long)]
    dry_run: bool,

    /// Debug mode (verbose logging)
    #[arg(long)]
    debug: bool,
}

pub async fn handle_discover(args: DiscoverArgs) -> anyhow::Result<()> {
    // 1. Load configs
    let browser_config = BrowserConfig::load()?;
    
    // 2. Initialize components
    let automation = Arc::new(BrowserAutomation::new(browser_config.clone()).await?);
    
    let search_config = Arc::new(SearchConfig {
        search_engine: parse_search_engine(&args.search_engine)?,
        scroll_iterations: 3,
        max_results: args.max_plans * 2, // Get more results to filter
        filter_domains: vec![],
        delay_range_ms: (2000, 5000),
    });
    
    let searcher = ContentSearcher::new(search_config, automation.clone());
    let pbd = PlayBeforeDownload::new(browser_config.clone());
    
    let planner = Arc::new(Mutex::new(SqlitePlanStore::open(&browser_config.plans_db_path)?));
    
    let discovery_config = DiscoveryConfig {
        max_plans_per_run: args.max_plans,
        candidate_delay_range_ms: (8000, 15000),
        stop_on_first_error: false,
        dry_run: args.dry_run,
    };
    
    let mut discovery = DiscoveryLoop::new(searcher, pbd, planner, discovery_config);
    
    // 3. Run discovery
    let stats = discovery.run(&args.query).await?;
    
    // 4. Print results
    println!("\n═══════════════════════════════════");
    println!("  DISCOVERY COMPLETE");
    println!("═══════════════════════════════════");
    println!("Query: {}", stats.query);
    println!("Candidates found: {}", stats.candidates_found);
    println!("Candidates processed: {}", stats.candidates_processed);
    println!("PLANs created: {}", stats.plans_created);
    println!("Duration: {}s", stats.duration_secs);
    
    if !stats.errors.is_empty() {
        println!("\n⚠️  Errors ({}):", stats.errors.len());
        for (i, error) in stats.errors.iter().enumerate().take(5) {
            println!("  {}. {}", i + 1, error);
        }
        if stats.errors.len() > 5 {
            println!("  ... and {} more", stats.errors.len() - 5);
        }
    }
    
    Ok(())
}

fn parse_search_engine(s: &str) -> anyhow::Result<SearchEngine> {
    match s.to_lowercase().as_str() {
        "google" => Ok(SearchEngine::Google),
        "bing" => Ok(SearchEngine::Bing),
        "duckduckgo" | "ddg" => Ok(SearchEngine::DuckDuckGo),
        _ => Err(anyhow!("Invalid search engine: {}", s)),
    }
}
```

---

## ✅ CRITÉRIOS DE ACEITAÇÃO

### 1. Funcionalidade

- [ ] ContentSearcher pode buscar em Google/Bing/DuckDuckGo
- [ ] Scroll funciona e coleta múltiplos resultados
- [ ] Filtra candidatos relevantes (vídeos)
- [ ] DiscoveryLoop processa múltiplos candidatos
- [ ] Rate limiting funciona (delays entre requests)
- [ ] PLANs são criados corretamente no database
- [ ] Dry-run funciona sem criar PLANs

### 2. Qualidade

- [ ] Simulação humana perfeita (nenhum bot detection)
- [ ] Código compila sem warnings (`cargo clippy`)
- [ ] Formatação correta (`cargo fmt`)
- [ ] Logs estruturados (tracing)
- [ ] Error handling robusto

### 3. Testes

- [ ] Testes unitários para ContentSearcher
- [ ] Testes unitários para DiscoveryLoop
- [ ] Teste de integração end-to-end
- [ ] Mock test (sem network, sem browser)

### 4. Documentação

- [ ] Docstrings em todas funções públicas
- [ ] README atualizado com exemplo de uso
- [ ] CHANGELOG entry

---

## 🧪 PLANO DE TESTES

### Test 1: Unit - SearchEngine Detection

```rust
#[test]
fn test_search_engine_url() {
    let config = SearchConfig {
        search_engine: SearchEngine::Google,
        // ...
    };
    let searcher = ContentSearcher::new(/* ... */);
    assert_eq!(searcher.get_search_url(), "https://www.google.com");
}
```

### Test 2: Unit - Video Detection

```rust
#[test]
fn test_is_likely_video() {
    let candidate = Candidate {
        url: "https://youtube.com/watch?v=123".to_string(),
        title: Some("Amazing Documentary".to_string()),
        // ...
    };
    let searcher = ContentSearcher::new(/* ... */);
    assert!(searcher.is_likely_video(&candidate));
}
```

### Test 3: Integration - Search Flow (Mock)

```rust
#[tokio::test]
async fn test_search_flow_mock() {
    // Mock browser that returns predefined HTML
    let mock_automation = MockBrowserAutomation::new();
    mock_automation.set_html(MOCK_GOOGLE_RESULTS_HTML);
    
    let searcher = ContentSearcher::new(/* mock config */, Arc::new(mock_automation));
    let results = searcher.search("test query").await.unwrap();
    
    assert!(results.len() > 0);
    assert!(results[0].url.starts_with("http"));
}
```

### Test 4: Integration - Discovery Loop (Dry-Run)

```rust
#[tokio::test]
async fn test_discovery_loop_dry_run() {
    let config = DiscoveryConfig {
        max_plans_per_run: 3,
        dry_run: true,
        // ...
    };
    
    let mut discovery = DiscoveryLoop::new(/* ... */);
    let stats = discovery.run("test query").await.unwrap();
    
    assert_eq!(stats.candidates_processed, 3);
    // Verify no actual PLANs were created in DB
}
```

### Test 5: E2E - Full Discovery (Real Browser, needs network)

```bash
#!/bin/bash
# tests/e2e_discovery.sh

# Mark as ignored (requires network + browser)
cargo test --test discovery_e2e -- --ignored

# Manual validation:
vvtvctl discover --query="creative commons documentary" --max-plans=2 --dry-run --debug
```

---

## 📊 MÉTRICAS DE SUCESSO

| Métrica | Target | Como Medir |
|---------|--------|------------|
| Candidates per search | ≥10 | `searcher.search()` output length |
| Video detection accuracy | >80% | Manual review of 20 candidates |
| PBD success rate | >70% | `stats.plans_created / stats.candidates_processed` |
| Search time | <15s | `searcher.search()` duration |
| Discovery time (5 candidates) | <3min | `stats.duration_secs` |
| Bot detection rate | 0% | Monitor for CAPTCHAs, blocks |
| Memory usage | <500MB | `cargo run` + monitor RSS |

---

## 🚀 ROADMAP DE IMPLEMENTAÇÃO

### Dia 1: ContentSearcher

**Manhã (4h):**
- [ ] Criar `searcher.rs`
- [ ] Implementar `ContentSearcher` struct
- [ ] Implementar `search()` para Google
- [ ] Implementar `parse_search_results()`
- [ ] Implementar `filter_candidates()`

**Tarde (4h):**
- [ ] Adicionar support para Bing
- [ ] Adicionar support para DuckDuckGo
- [ ] Testes unitários (video detection, filtering)
- [ ] Testes mock (search flow)

### Dia 2: DiscoveryLoop

**Manhã (4h):**
- [ ] Criar `discovery_loop.rs`
- [ ] Implementar `DiscoveryLoop` struct
- [ ] Implementar `run()`
- [ ] Implementar `process_candidate()`
- [ ] Integrar com PBD existing

**Tarde (4h):**
- [ ] Implementar rate limiting
- [ ] Implementar error handling
- [ ] Testes unitários (loop logic)
- [ ] Teste de integração (dry-run)

### Dia 3: CLI + Testing

**Manhã (4h):**
- [ ] Adicionar comando `discover` ao vvtvctl
- [ ] Implementar argument parsing
- [ ] Implementar output formatting
- [ ] Adicionar config loading

**Tarde (4h):**
- [ ] E2E test (manual, browser)
- [ ] Bug fixes
- [ ] Documentation (docstrings, README)
- [ ] Code review checklist

---

## 🔒 CONSIDERAÇÕES DE SEGURANÇA

### 1. Antibot Mitigation

- Usar Human Simulation em TODOS os passos
- Random delays entre ações
- Limitar requests por hora (sugestão: max 50 searches/hour)
- Rotação de User-Agent
- Viewport randomization

### 2. Privacy

- Não logar queries sensíveis
- Anonimizar URLs em logs (hash ou truncate)
- GDPR-compliant (não armazenar PII)

### 3. Rate Limiting

```rust
// Rate limiter suggestion
struct RateLimiter {
    max_requests_per_hour: usize,
    window: Vec<Instant>,
}

impl RateLimiter {
    async fn wait_if_needed(&mut self) {
        // Implement token bucket or sliding window
    }
}
```

---

## 📝 NOTAS DE IMPLEMENTAÇÃO

### Desafios Conhecidos

1. **Search engine layouts mudam:** Manter seletores atualizados
2. **Bot detection:** Investir em human simulation de qualidade
3. **Network errors:** Implementar retries com backoff
4. **Rate limits:** Delays generosos entre requests

### Notas da Implementação Atual (2025-10-21)

- O `ContentSearcher` abre as páginas de resultado usando os endpoints de vídeo (Google `tbm=vid`, Bing vídeos, DuckDuckGo `ia=video`) e aplica heurísticas de domínio + palavras-chave para filtrar candidatos.
- O `DiscoveryLoop` calcula `curation_score` inicial com base no ranking e resolução detectada, registra estatísticas (`DiscoveryStats`) e respeita `dry_run` sem persistir PLANs.
- O comando `vvtvctl discover` expõe `--query`, `--max-plans`, `--search-engine`, `--dry-run` e `--debug`, reaproveitando o mesmo Chrome profile manager.

### Dependências Rust

```toml
[dependencies]
chromiumoxide = "0.5"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
tracing = "0.1"
rand = "0.8"
url = "2"
uuid = { version = "1", features = ["v4"] }
```

### Performance Considerations

- Reuse browser context quando possível
- Paralelizar PBD se buffer permite (max 2 concurrent)
- Cache search results (1h TTL) para mesma query

---

## 🎯 CRITÉRIO DE DONE

**PR D7 está completo quando:**

✅ Todos os testes passam (`cargo test`)  
✅ Clippy sem warnings (`cargo clippy`)  
✅ Código formatado (`cargo fmt`)  
✅ CLI funciona: `vvtvctl discover --query="test" --max-plans=2 --dry-run`  
✅ Teste E2E manual executado e documentado  
✅ Docstrings em todas funções públicas  
✅ README atualizado com exemplo  
✅ Tasklist.md atualizado (PR D7 marcado como [x])  
✅ CHANGELOG entry adicionado  

---

## 📞 APROVAÇÃO

Este spec deve ser revisado e aprovado antes de iniciar implementação.

**Aprovador:** [Nome]  
**Data:** [YYYY-MM-DD]  
**Status:** [ ] Aprovado | [ ] Requer mudanças | [ ] Rejeitado

**Comentários:**
```
[Espaço para feedback]
```

---

> *"Discovery is not just about finding content. It's about finding the RIGHT content, ethically, autonomously, and continuously."*  
> — SPEC PR D7, 2025-10-21
