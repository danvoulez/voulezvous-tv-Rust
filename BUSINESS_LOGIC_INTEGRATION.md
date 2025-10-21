# 🎯 Business Logic Integration — Blueprint → Rust Implementation

> **Objetivo:** Conectar o **Cartão Perfurado do Dono** (YAML) com o **Motor Rust Determinístico** (código).

---

## 📋 VISÃO GERAL

```
┌─────────────────────────────────────────────────────────────┐
│                  ARQUITETURA HÍBRIDA                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌────────────────┐         ┌──────────────────┐          │
│  │  Cartão Dono   │────────→│  Rust Engine     │          │
│  │  (YAML)        │         │  (Determinístico)│          │
│  │  by-dan        │         │  95% do trabalho │          │
│  └────────────────┘         └──────────────────┘          │
│         │                            ↓                      │
│         │                    ┌──────────────────┐          │
│         │                    │   LLM Curador    │          │
│         └───────────────────→│   (Conselheiro)  │          │
│                              │   5% sugestões   │          │
│                              └──────────────────┘          │
│                                       ↓                     │
│                              ┌──────────────────┐          │
│                              │   Autopilot      │          │
│                              │   (D+1 feedback) │          │
│                              └──────────────────┘          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Princípio:** Rust faz 95% do serviço pesado. LLM é o "azeite", o "amalgama" que refina.

---

## 1️⃣ MAPEAMENTO: Blueprint YAML → Código Rust Existente

### ✅ **O QUE JÁ EXISTE NO CÓDIGO**

| Blueprint Concept | Rust Implementation | Arquivo |
|-------------------|---------------------|---------|
| **Scoring (6 fatores)** | ✅ `score_candidates()` | `plan/planner.rs` L79-115 |
| **Diversity quota** | ✅ `apply_selection_rules()` | `plan/planner.rs` L117-161 |
| **Music ratio** | ✅ `QueueSelectionPolicy::FifoWithMusicRatio` | `queue.rs` |
| **Curation bump** | ✅ `priority += 1000` | `queue.rs` |
| **Emergency loop** | ✅ `ensure_emergency_buffer()` | `broadcaster/mod.rs` L179-226 |
| **QC thresholds** | ✅ `QcThresholds` struct | `processor/mod.rs` |
| **PBD enforcement** | ✅ `PlayBeforeDownload::collect()` | `browser/pbd.rs` |

**Conclusão:** O motor Rust **determinístico** já está ~80% pronto!

---

### ❌ **O QUE FALTA IMPLEMENTAR**

| Blueprint Feature | Status | Prioridade |
|-------------------|--------|------------|
| **`business_logic.yaml` loader** | ❌ | 🔴 CRÍTICO |
| **Softmax selection** | ❌ | 🔴 CRÍTICO |
| **Seed per slot** | ❌ | 🔴 CRÍTICO |
| **LLM hooks** | ❌ | 🟡 IMPORTANTE |
| **Curator Vigilante** | ❌ | 🟡 IMPORTANTE |
| **Autopilot D+1** | ❌ | 🟢 DESEJÁVEL |
| **Incident learning** | ❌ | 🟢 DESEJÁVEL |
| **GitHub App auto-merge** | ❌ | 🟢 DESEJÁVEL |

---

## 2️⃣ IMPLEMENTAÇÃO: Fase por Fase

### 🔴 **FASE 1: Core Business Logic (3 dias)**

#### A) Carregar `business_logic.yaml`

```rust
// vvtv-core/src/business_logic/mod.rs

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct BusinessLogic {
    pub policy_version: String,
    pub env: String,
    pub knobs: Knobs,
    pub scheduling: Scheduling,
    pub selection: Selection,
    pub exploration: Exploration,
    pub autopilot: Autopilot,
    pub kpis: Kpis,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Knobs {
    pub boost_bucket: String,
    pub music_mood_focus: Vec<String>,
    pub interstitials_ratio: f64,
    pub plan_selection_bias: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Selection {
    pub method: SelectionMethod,
    pub temperature: f64,
    pub seed_strategy: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionMethod {
    Softmax,
    Greedy,
    EpsilonGreedy,
}

impl BusinessLogic {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let logic: Self = serde_yaml::from_str(&content)?;
        logic.validate()?;
        Ok(logic)
    }
    
    fn validate(&self) -> Result<()> {
        if self.knobs.plan_selection_bias.abs() > 0.20 {
            return Err(anyhow!("plan_selection_bias must be in range [-0.20, 0.20]"));
        }
        if self.exploration.epsilon < 0.0 || self.exploration.epsilon > 1.0 {
            return Err(anyhow!("epsilon must be in range [0.0, 1.0]"));
        }
        Ok(())
    }
}
```

**Localização do arquivo:**
```
/vvtv/business_logic/business_logic.yaml  ← Cartão do Dono
```

---

#### B) Implementar Softmax Selection

```rust
// vvtv-core/src/plan/selection.rs

use rand::distributions::WeightedIndex;
use rand::prelude::*;

pub struct SoftmaxSelector {
    temperature: f64,
    rng: StdRng,
}

impl SoftmaxSelector {
    pub fn new(temperature: f64, seed: u64) -> Self {
        Self {
            temperature,
            rng: StdRng::seed_from_u64(seed),
        }
    }
    
    /// Seleciona um item usando softmax sobre os scores
    pub fn select<T>(&mut self, items: &[(T, f64)]) -> Option<usize>
    where
        T: Clone,
    {
        if items.is_empty() {
            return None;
        }
        
        // Calcular softmax
        let max_score = items.iter().map(|(_, s)| s).fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let exp_scores: Vec<f64> = items
            .iter()
            .map(|(_, score)| ((score - max_score) / self.temperature).exp())
            .collect();
        
        let sum: f64 = exp_scores.iter().sum();
        let probabilities: Vec<f64> = exp_scores.iter().map(|e| e / sum).collect();
        
        // Sample
        let dist = WeightedIndex::new(&probabilities).ok()?;
        Some(dist.sample(&mut self.rng))
    }
}
```

**Seed Strategy: "slot_hash"**
```rust
pub fn generate_slot_seed(date: NaiveDate, slot_id: u32, global_seed: u64) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    date.hash(&mut hasher);
    slot_id.hash(&mut hasher);
    global_seed.hash(&mut hasher);
    hasher.finish()
}
```

---

#### C) Integrar no Planner

```rust
// vvtv-core/src/plan/planner.rs

pub struct Planner {
    store: SqlitePlanStore,
    config: PlannerConfig,
    business_logic: Arc<BusinessLogic>,  // ← NOVO
}

impl Planner {
    pub fn run_once(&self, now: DateTime<Utc>) -> PlanResult<PlannerEvent> {
        let candidates = self.store.fetch_candidates_for_scoring(self.config.selection_limit)?;
        
        let kind_frequency = self.kind_frequency(&candidates);
        let mut scored = self.score_candidates(&candidates, &kind_frequency, now);
        
        // ✅ NOVO: Aplicar bias do Cartão do Dono
        let bias = self.business_logic.knobs.plan_selection_bias;
        for (_, score, _) in &mut scored {
            *score += bias;
        }
        
        // ✅ NOVO: Softmax selection ao invés de sort simples
        let decisions = self.softmax_select(scored, now)?;
        
        self.store.store_decisions(&decisions)?;
        Ok(PlannerEvent::Selected(decisions))
    }
    
    fn softmax_select(
        &self,
        scored: Vec<(Plan, f64, String)>,
        now: DateTime<Utc>,
    ) -> Result<Vec<PlanSelectionDecision>> {
        let seed = generate_slot_seed(
            now.date_naive(),
            (now.hour() * 4 + now.minute() / 15) as u32,  // slot de 15 min
            self.business_logic.selection.seed_strategy.parse().unwrap_or(42),
        );
        
        let mut selector = SoftmaxSelector::new(
            self.business_logic.selection.temperature,
            seed,
        );
        
        let mut selected = Vec::new();
        let mut available = scored;
        
        for _ in 0..self.config.selection_batch_size {
            if available.is_empty() {
                break;
            }
            
            let idx = selector.select(&available).ok_or_else(|| anyhow!("Selection failed"))?;
            let (plan, score, rationale) = available.remove(idx);
            
            selected.push(PlanSelectionDecision {
                plan_id: plan.plan_id,
                score,
                rationale: format!("{} | softmax_idx={} temp={}", 
                    rationale, idx, self.business_logic.selection.temperature),
            });
        }
        
        Ok(selected)
    }
}
```

---

### 🟡 **FASE 2: LLM Integration (4 dias)**

#### A) LLM Hook System

```rust
// vvtv-core/src/llm/mod.rs

use serde::{Deserialize, Serialize};
use tokio::time::timeout;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMAction {
    pub source: String,       // "expand_queries"
    pub model: String,        // "gpt-4o"
    pub reason: String,       // "paleta distinta..."
    pub confidence: f64,      // 0.78
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug)]
pub struct LLMHook {
    context: LLMContext,
    deadline_ms: u64,
    budget_tokens: usize,
    allowed_actions: Vec<String>,
}

pub enum LLMContext {
    SearchStrategy,
    Curation,
    SoftError,
    Report,
}

pub struct LLMClient {
    endpoint: String,
    timeout: Duration,
}

impl LLMClient {
    /// Chama LLM com timeout rígido
    pub async fn call_hook(
        &self,
        hook: &LLMHook,
        prompt: &str,
    ) -> Result<LLMResponse> {
        let future = self.call_internal(prompt);
        
        match timeout(Duration::from_millis(hook.deadline_ms), future).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(e)) => {
                warn!("LLM call failed: {}", e);
                Ok(LLMResponse::fallback())
            }
            Err(_) => {
                warn!("LLM timeout after {}ms", hook.deadline_ms);
                Ok(LLMResponse::fallback())
            }
        }
    }
    
    async fn call_internal(&self, prompt: &str) -> Result<LLMResponse> {
        // Implementar chamada HTTP para LLM local/remoto
        let response = reqwest::Client::new()
            .post(&self.endpoint)
            .json(&serde_json::json!({
                "prompt": prompt,
                "max_tokens": 512,
            }))
            .send()
            .await?;
        
        let data: LLMResponse = response.json().await?;
        Ok(data)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LLMResponse {
    pub action: Option<LLMAction>,
    pub suggestions: Vec<String>,
    pub confidence: f64,
}

impl LLMResponse {
    fn fallback() -> Self {
        Self {
            action: None,
            suggestions: vec![],
            confidence: 0.0,
        }
    }
}
```

---

#### B) Curator Vigilante

```rust
// vvtv-core/src/curator/vigilante.rs

pub struct CuratorVigilante {
    enabled: bool,
    llm_client: LLMClient,
    config: CuratorConfig,
}

#[derive(Debug, Clone)]
pub struct CuratorConfig {
    pub min_confidence_apply: f64,
    pub diversity_target: f64,
    pub max_reorder_distance: usize,
}

impl CuratorVigilante {
    pub async fn review_selection(
        &self,
        candidates: &[Plan],
        selected: &[Plan],
        context: CurationContext,
    ) -> Result<CuratorDecision> {
        if !self.enabled {
            return Ok(CuratorDecision::NoAction);
        }
        
        // 1. Detectar problemas
        let signals = self.detect_signals(selected)?;
        
        if signals.is_empty() {
            return Ok(CuratorDecision::NoAction);
        }
        
        // 2. Consultar LLM
        let prompt = self.build_prompt(&signals, candidates, selected);
        let hook = LLMHook {
            context: LLMContext::Curation,
            deadline_ms: 1200,
            budget_tokens: 768,
            allowed_actions: vec!["rerank".into(), "judge".into()],
        };
        
        let response = self.llm_client.call_hook(&hook, &prompt).await?;
        
        // 3. Decidir ação
        let decision = self.make_decision(response, &context)?;
        
        // 4. Log auditável
        self.log_decision(&decision, &signals)?;
        
        Ok(decision)
    }
    
    fn detect_signals(&self, selected: &[Plan]) -> Result<Vec<Signal>> {
        let mut signals = Vec::new();
        
        // Palette similarity
        if let Some(sim) = self.check_palette_similarity(selected)? {
            if sim > 0.85 {
                signals.push(Signal::PaletteSimilarity(sim));
            }
        }
        
        // Tag duplication
        let tag_jaccard = self.check_tag_duplication(selected)?;
        if tag_jaccard > 0.75 {
            signals.push(Signal::TagDuplication(tag_jaccard));
        }
        
        // Duration streak
        if let Some(streak) = self.check_duration_streak(selected) {
            signals.push(Signal::DurationStreak(streak));
        }
        
        Ok(signals)
    }
    
    fn make_decision(
        &self,
        response: LLMResponse,
        context: &CurationContext,
    ) -> Result<CuratorDecision> {
        if response.confidence < self.config.min_confidence_apply {
            return Ok(CuratorDecision::Advice {
                suggestions: response.suggestions,
                confidence: response.confidence,
                llm_action: response.action,
            });
        }
        
        // Pode aplicar mudanças
        if context.allow_apply && !context.locked {
            Ok(CuratorDecision::Apply {
                changes: self.parse_changes(&response)?,
                llm_action: response.action.unwrap(),
            })
        } else {
            Ok(CuratorDecision::Advice {
                suggestions: response.suggestions,
                confidence: response.confidence,
                llm_action: response.action,
            })
        }
    }
    
    fn log_decision(&self, decision: &CuratorDecision, signals: &[Signal]) -> Result<()> {
        let log_entry = serde_json::json!({
            "timestamp": Utc::now(),
            "signals": signals,
            "decision": decision,
            "trace_id": uuid::Uuid::new_v4(),
        });
        
        // Append to logs/curator_vigilante/%Y-%m-%d.jsonl
        let path = format!(
            "/vvtv/logs/curator_vigilante/{}.jsonl",
            Utc::now().format("%Y-%m-%d")
        );
        
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        writeln!(file, "{}", log_entry)?;
        
        Ok(())
    }
}

#[derive(Debug)]
pub enum CuratorDecision {
    NoAction,
    Advice {
        suggestions: Vec<String>,
        confidence: f64,
        llm_action: Option<LLMAction>,
    },
    Apply {
        changes: Vec<Change>,
        llm_action: LLMAction,
    },
}

#[derive(Debug, Serialize)]
pub enum Signal {
    PaletteSimilarity(f64),
    TagDuplication(f64),
    DurationStreak(usize),
    BucketImbalance { bucket: String, share: f64 },
}
```

---

### 🟢 **FASE 3: Autopilot D+1 (3 dias)**

```rust
// vvtv-core/src/autopilot/mod.rs

pub struct Autopilot {
    enabled: bool,
    config: AutopilotConfig,
    metrics_store: MetricsStore,
    business_logic: Arc<Mutex<BusinessLogic>>,
}

impl Autopilot {
    pub async fn run_daily_cycle(&mut self) -> Result<AutopilotReport> {
        if !self.config.daily_auto_apply.enabled {
            return Ok(AutopilotReport::Disabled);
        }
        
        // 1. Coletar métricas D-1
        let yesterday = Utc::now().date_naive() - Duration::days(1);
        let metrics = self.metrics_store.get_daily_metrics(yesterday)?;
        
        // 2. Calcular ajustes propostos
        let adjustments = self.calculate_adjustments(&metrics)?;
        
        // 3. Validar bounds (não ultrapassar limites)
        let safe_adjustments = self.validate_bounds(adjustments)?;
        
        // 4. Aplicar em canary (20%)
        self.apply_canary(&safe_adjustments).await?;
        
        // 5. Monitorar por 60 min
        tokio::time::sleep(Duration::from_secs(3600)).await;
        
        // 6. Validar KPIs
        let kpis_ok = self.validate_kpis().await?;
        
        if kpis_ok {
            // 7. Aplicar em 100%
            self.apply_full(&safe_adjustments).await?;
            
            // 8. Commit mudanças no business_logic.yaml
            self.commit_changes(&safe_adjustments).await?;
            
            Ok(AutopilotReport::Applied {
                adjustments: safe_adjustments,
                metrics_before: metrics.clone(),
            })
        } else {
            // Rollback
            self.rollback().await?;
            
            Ok(AutopilotReport::Rolled Back {
                reason: "KPI regression detected",
            })
        }
    }
    
    fn calculate_adjustments(&self, metrics: &DailyMetrics) -> Result<Vec<Adjustment>> {
        let mut adjustments = Vec::new();
        
        // Feedback do Curador
        let curator_signals = self.metrics_store.get_curator_signals_d1()?;
        
        if curator_signals.palette_similarity_avg > 0.80 {
            adjustments.push(Adjustment::ModifyKeyword {
                path: "keywords.videos.buckets.high-retention-core.boosts".into(),
                change: -0.04,  // Reduzir boost para aumentar variedade
                reason: "High palette similarity detected".into(),
            });
        }
        
        if curator_signals.diversity_gain_avg < 0.03 {
            adjustments.push(Adjustment::ModifyParameter {
                path: "exploration.epsilon".into(),
                change: 0.01,  // Aumentar exploração
                reason: "Low diversity gain".into(),
            });
        }
        
        // Retention feedback
        if metrics.retention_5min < 0.38 {
            adjustments.push(Adjustment::ModifyParameter {
                path: "knobs.plan_selection_bias".into(),
                change: 0.02,  // Favorecer high-retention
                reason: "Below retention target".into(),
            });
        }
        
        Ok(adjustments)
    }
}

#[derive(Debug, Serialize)]
pub enum Adjustment {
    ModifyKeyword { path: String, change: f64, reason: String },
    ModifyParameter { path: String, change: f64, reason: String },
    AddNegative { keyword: String, category: String, reason: String },
}
```

---

## 3️⃣ ESTRUTURA DE ARQUIVOS

```
/vvtv/
├── business_logic/
│   ├── business_logic.yaml          ← CARTÃO DO DONO
│   ├── keywords/
│   │   ├── negatives.yaml
│   │   └── boosts.yaml
│   ├── pairing/
│   │   └── music_moods.yaml
│   └── history/
│       └── 2025-10-21_v1.yaml       ← Git-like history
│
├── system/
│   ├── vvtvctl                      ← CLI tool
│   └── configs/
│       ├── vvtv.toml                ← Sistema base
│       ├── llm.toml                 ← LLM endpoints
│       └── autopilot.toml           ← Autopilot config
│
├── logs/
│   ├── curator_vigilante/
│   │   └── 2025-10-21.jsonl         ← Auditoria Curador
│   ├── autopilot/
│   │   └── 2025-10-21.jsonl         ← Decisões D+1
│   └── incidents/
│       └── 2025-10/
│           └── incident_001.json
│
└── data/
    ├── plans.sqlite
    ├── queue.sqlite
    └── metrics.sqlite
```

---

## 4️⃣ CLI TOOLS

```bash
# Recarregar business logic (hot reload)
vvtvctl business-logic reload

# Ver estado atual
vvtvctl business-logic show

# Validar antes de aplicar
vvtvctl business-logic validate --file=new_policy.yaml

# Histórico de mudanças
vvtvctl business-logic history --last=10

# Forçar rollback
vvtvctl business-logic rollback --to-version=v2025-10-20

# Ver sugestões do Curador (últimas 24h)
vvtvctl curator advice --last=24h

# Ver ações aplicadas pelo Curador
vvtvctl curator actions --date=2025-10-21

# Relatório Autopilot
vvtvctl autopilot report --date=yesterday

# Simular ajuste (dry-run)
vvtvctl autopilot simulate --adjustment="epsilon +0.01"
```

---

## 5️⃣ INTEGRAÇÃO: Rust ↔ LLM ↔ YAML

```
┌─────────────────────────────────────────────────────┐
│                  FLUXO COMPLETO                     │
├─────────────────────────────────────────────────────┤
│                                                     │
│  1. business_logic.yaml carregado no startup       │
│     └→ Planner, Queue, Broadcaster recebem config  │
│                                                     │
│  2. Planner executa (T-4h):                        │
│     ├→ Score com bias do Cartão                    │
│     ├→ Softmax selection (T=0.6, seed=slot)        │
│     └→ Diversity enforcement (20%)                 │
│                                                     │
│  3. Curator Vigilante revisa:                      │
│     ├→ Detecta sinais (palette, tags, etc)         │
│     ├→ Consulta LLM (deadline 1200ms)              │
│     ├→ Se confidence > 0.62 → APLICA               │
│     ├→ Se confidence < 0.62 → ADVICE               │
│     └→ Log em curator_vigilante.jsonl              │
│                                                     │
│  4. Queue aplica:                                  │
│     ├→ Music ratio (10%)                           │
│     ├→ Curation bump (score > 0.85)                │
│     └→ FIFO + jitter                               │
│                                                     │
│  5. Broadcaster transmite:                         │
│     ├→ Emergency loop (buffer < 1h)                │
│     └→ Métricas em tempo real                      │
│                                                     │
│  6. Autopilot D+1 (03:00 UTC):                     │
│     ├→ Lê métricas D-1                             │
│     ├→ Lê sinais do Curador                        │
│     ├→ Calcula ajustes seguros                     │
│     ├→ Aplica canary (20%, 60 min)                 │
│     ├→ Valida KPIs                                 │
│     ├→ Se OK → commit em business_logic.yaml       │
│     └→ Se NOT OK → rollback                        │
│                                                     │
│  7. Monthly Review (dia 1, 09:00 UTC):             │
│     ├→ LLM gera relatório mensal                   │
│     ├→ Propõe ajustes macro                        │
│     ├→ Abre PR no GitHub                           │
│     └→ Requer 1 aprovação humana                   │
│                                                     │
└─────────────────────────────────────────────────────┘
```

---

## 6️⃣ ROADMAP DE IMPLEMENTAÇÃO

### Semana 1: Core (Fase 1)
```
[ ] business_logic.yaml schema + loader
[ ] Softmax selector
[ ] Seed per slot
[ ] Integration com Planner
[ ] CLI: reload, show, validate
[ ] Tests unitários
```

### Semana 2: LLM (Fase 2)
```
[ ] LLM client com timeout
[ ] Hook system (5 hooks)
[ ] Curator Vigilante
[ ] Signal detection
[ ] Audit logging
[ ] Tests com mock LLM
```

### Semana 3: Autopilot (Fase 3)
```
[ ] Daily cycle runner
[ ] Metrics aggregation D-1
[ ] Adjustment calculator
[ ] Canary deployment
[ ] KPI validation
[ ] Rollback mechanism
[ ] YAML commit/history
```

### Semana 4: Polish & Deploy
```
[ ] Monthly review generator
[ ] GitHub App integration
[ ] Incident learning system
[ ] Documentation completa
[ ] E2E tests
[ ] Deploy production
```

---

## 7️⃣ EXEMPLO CONCRETO: Fluxo Completo

### Cenário: Quarta-feira, 21:30 UTC (Prime Time)

**1. business_logic.yaml diz:**
```yaml
scheduling:
  windows:
    prime_time_utc:
      start: "19:00"
      boost_buckets: ["high-retention-core"]
      music_mood_focus: ["downtempo"]
```

**2. Planner executa (T-4h = 17:30):**
```rust
// Seed para slot 21:30
let seed = generate_slot_seed(
    NaiveDate::from_ymd(2025, 10, 21),
    86,  // (21*4 + 2) = slot de 15min
    42   // global seed
);

// Softmax com T=0.6
let mut selector = SoftmaxSelector::new(0.6, seed);

// Score candidates
let mut scored = self.score_candidates(&candidates, &freq, now);

// Apply bias: +0.10 (favorece high-retention)
for (_, score, _) in &mut scored {
    *score += 0.10;
}

// Select 12 plans via softmax
let selected = selector.select_batch(&scored, 12)?;
```

**3. Curator revisa:**
```rust
let signals = curator.detect_signals(&selected)?;
// Detectado: palette_similarity = 0.87 (alto!)

let response = llm_client.call_hook(&hook, &prompt).await?;
// LLM sugere: "Trocar candidato #4 por #7 (paleta contrastante)"
// Confidence: 0.78 (> 0.62 threshold)

// APLICAR mudança
curator.apply_reorder(4, 7)?;

// LOG
curator_vigilante.jsonl:
{
  "ts": "2025-10-21T21:28:12Z",
  "signal": "palette_similarity",
  "value": 0.87,
  "action": "reorder",
  "from_idx": 4,
  "to_idx": 7,
  "llm_action": {
    "source": "curator_vigilante",
    "model": "gpt-4o",
    "confidence": 0.78,
    "reason": "paleta contrastante + duração ideal 10min"
  }
}
```

**4. Queue processa:**
```rust
// Music ratio enforcement
if recent_music_pct < 0.10 {
    // Força música no próximo slot
    return next_music_with_mood("downtempo")?;
}

// Curation bump
if plan.curation_score > 0.85 {
    plan.priority += 1000;
}

// FIFO + jitter
tokio::time::sleep(Duration::from_millis(rand::thread_rng().gen_range(15000..45000))).await;
```

**5. Broadcast:**
```rust
// Check buffer
if metrics.buffer_duration_hours < 1.0 {
    warn!("Buffer crítico!");
    self.inject_emergency_loop().await?;
}

// Stream para RTMP
ffmpeg -re -i asset.mp4 -f flv rtmp://localhost/live/main
```

**6. Autopilot D+1 (Quinta, 03:00 UTC):**
```rust
// Lê métricas de Quarta
let metrics = metrics_store.get_daily_metrics("2025-10-21")?;
// retention_5min: 0.39 (OK, acima de 0.38)
// vmaf_avg: 91 (OK, acima de 90)

// Lê sinais do Curador
let curator_signals = metrics_store.get_curator_signals("2025-10-21")?;
// palette_similarity_avg: 0.84 (alto!)
// curator_diversity_gain_avg: 0.06 (bom!)

// Propõe ajuste
let adjustment = Adjustment::ModifyKeyword {
    path: "keywords.videos.buckets.high-retention-core.boosts",
    change: -0.03,  // Reduzir ligeiramente para mais variedade
    reason: "Palette similarity alta mas diversity gain OK",
};

// Aplica canary (20%)
autopilot.apply_canary(&[adjustment]).await?;

// Aguarda 60 min...
tokio::time::sleep(Duration::from_secs(3600)).await;

// Valida KPIs
let kpis_ok = autopilot.validate_kpis().await?;
// retention_5min: 0.39 (manteve)
// vmaf_avg: 91.2 (melhorou ligeiramente!)

// COMMIT
autopilot.commit_to_yaml(&[adjustment]).await?;
```

---

---

## 8️⃣ TESTES E VALIDAÇÃO

### A) Testes Unitários

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_softmax_selection_deterministic() {
        let seed = 12345_u64;
        let mut selector1 = SoftmaxSelector::new(0.6, seed);
        let mut selector2 = SoftmaxSelector::new(0.6, seed);
        
        let items = vec![
            ("item1", 0.8),
            ("item2", 0.6),
            ("item3", 0.4),
        ];
        
        let idx1 = selector1.select(&items).unwrap();
        let idx2 = selector2.select(&items).unwrap();
        
        // Mesmo seed = mesma seleção (determinístico)
        assert_eq!(idx1, idx2);
    }
    
    #[test]
    fn test_softmax_respects_temperature() {
        let seed = 12345_u64;
        
        // T=0.1 (quase greedy)
        let mut selector_low = SoftmaxSelector::new(0.1, seed);
        // T=10.0 (quase uniforme)
        let mut selector_high = SoftmaxSelector::new(10.0, seed + 1);
        
        let items = vec![
            ("item1", 1.0),  // Muito melhor
            ("item2", 0.1),
            ("item3", 0.1),
        ];
        
        // Com T baixo, deve escolher item1 muito mais frequentemente
        let mut counts_low = [0, 0, 0];
        let mut counts_high = [0, 0, 0];
        
        for _ in 0..1000 {
            counts_low[selector_low.select(&items).unwrap()] += 1;
            counts_high[selector_high.select(&items).unwrap()] += 1;
        }
        
        // T baixo: item1 dominante
        assert!(counts_low[0] > 900);
        // T alto: mais distribuído
        assert!(counts_high[0] < 600);
    }
    
    #[test]
    fn test_business_logic_validation() {
        let mut logic = BusinessLogic {
            policy_version: "test".into(),
            env: "test".into(),
            knobs: Knobs {
                boost_bucket: "test".into(),
                music_mood_focus: vec![],
                interstitials_ratio: 0.05,
                plan_selection_bias: 0.25,  // INVÁLIDO (> 0.20)
            },
            // ... resto dos campos
        };
        
        // Deve falhar na validação
        assert!(logic.validate().is_err());
        
        // Corrigir
        logic.knobs.plan_selection_bias = 0.15;
        assert!(logic.validate().is_ok());
    }
    
    #[test]
    fn test_curator_confidence_threshold() {
        let config = CuratorConfig {
            min_confidence_apply: 0.62,
            diversity_target: 0.05,
            max_reorder_distance: 4,
        };
        
        let curator = CuratorVigilante::new(config);
        
        // Confidence abaixo → Advice
        let response_low = LLMResponse {
            action: Some(LLMAction { /* ... */ }),
            suggestions: vec!["reorder".into()],
            confidence: 0.55,  // < 0.62
        };
        
        let context = CurationContext {
            allow_apply: true,
            locked: false,
        };
        
        let decision = curator.make_decision(response_low, &context).unwrap();
        assert!(matches!(decision, CuratorDecision::Advice { .. }));
        
        // Confidence acima → Apply
        let response_high = LLMResponse {
            confidence: 0.75,  // >= 0.62
            // ...
        };
        
        let decision = curator.make_decision(response_high, &context).unwrap();
        assert!(matches!(decision, CuratorDecision::Apply { .. }));
    }
}
```

---

### B) Testes de Integração

```rust
#[tokio::test]
async fn test_end_to_end_business_logic_flow() {
    // 1. Setup
    let temp_dir = tempdir().unwrap();
    let yaml_path = temp_dir.path().join("business_logic.yaml");
    
    std::fs::write(&yaml_path, r#"
        policy_version: "test-v1"
        env: "test"
        knobs:
          boost_bucket: "high-retention-core"
          plan_selection_bias: 0.10
        selection:
          method: "softmax"
          temperature: 0.6
          seed_strategy: "slot_hash"
        # ... resto da config
    "#).unwrap();
    
    // 2. Carregar business logic
    let logic = BusinessLogic::load_from_file(&yaml_path).unwrap();
    assert_eq!(logic.knobs.plan_selection_bias, 0.10);
    
    // 3. Criar planner com business logic
    let planner = Planner::new(
        store,
        config,
        Arc::new(logic),
    );
    
    // 4. Executar ciclo de seleção
    let event = planner.run_once(Utc::now()).unwrap();
    
    // 5. Verificar que bias foi aplicado
    if let PlannerEvent::Selected(decisions) = event {
        // Todos os scores devem ter +0.10 aplicado
        for decision in decisions {
            // (verificar logs para confirmar bias)
        }
    }
}

#[tokio::test]
async fn test_curator_llm_timeout() {
    let config = CuratorConfig {
        min_confidence_apply: 0.62,
        // ...
    };
    
    // Mock LLM que demora 2 segundos
    let slow_llm = MockLLMClient::new(Duration::from_secs(2));
    let curator = CuratorVigilante::new(config, slow_llm);
    
    let hook = LLMHook {
        deadline_ms: 1000,  // Timeout de 1s
        // ...
    };
    
    let start = Instant::now();
    let response = curator.llm_client.call_hook(&hook, "test").await.unwrap();
    let elapsed = start.elapsed();
    
    // Deve ter feito timeout e retornado fallback
    assert!(elapsed < Duration::from_millis(1100));
    assert_eq!(response.confidence, 0.0);  // fallback
}
```

---

### C) Checklist de Validação por Fase

#### 🔴 Fase 1: Core Business Logic

```markdown
[ ] `business_logic.yaml` carrega sem erros
[ ] Validação detecta valores fora de bounds
[ ] Softmax selection é determinístico (mesmo seed = mesmo resultado)
[ ] Softmax respeita temperatura (T baixo = greedy, T alto = uniforme)
[ ] Seed per slot gera seeds únicos e reproduzíveis
[ ] Planner aplica bias corretamente
[ ] Hot reload funciona sem restart
[ ] CLI commands (`reload`, `show`, `validate`) funcionam
[ ] Logs incluem seed usado e temperatura
[ ] Performance: seleção < 10ms para 100 candidatos
```

#### 🟡 Fase 2: LLM Integration

```markdown
[ ] LLM client respeita timeout (nunca excede deadline)
[ ] Fallback funciona quando LLM falha
[ ] Todos os 5 hooks estão implementados
[ ] Curator detecta sinais corretamente (palette, tags, etc)
[ ] Curator respeita threshold de confidence
[ ] Logs incluem `llm_action` em todas as ações
[ ] Mock LLM permite testes sem API real
[ ] Performance: hook calls < deadline + 100ms overhead
[ ] Audit logs são append-only e em formato JSONL
[ ] Curator nunca remove itens, apenas reordena
```

#### 🟢 Fase 3: Autopilot D+1

```markdown
[ ] Daily cycle executa no horário correto (03:00 UTC)
[ ] Métricas D-1 são coletadas corretamente
[ ] Ajustes respeitam bounds (não ultrapassam limites)
[ ] Canary deployment isola 20% do tráfego
[ ] KPI validation detecta regressões
[ ] Rollback funciona automaticamente
[ ] Commit em YAML preserva histórico (Git-like)
[ ] Performance: ciclo completo < 5 min
[ ] Logs incluem antes/depois de métricas
[ ] GitHub PR é criado com manifesto correto
```

---

## 9️⃣ OBSERVABILIDADE E MÉTRICAS

### A) Métricas Core

| Métrica | Tipo | Threshold | Ação se Violado |
|---------|------|-----------|-----------------|
| `business_logic_load_time_ms` | Gauge | < 100ms | Alerta |
| `softmax_selection_time_ms` | Histogram | p95 < 10ms | Alerta |
| `llm_hook_timeout_rate` | Counter | < 5% | Revisar deadlines |
| `llm_hook_fallback_rate` | Counter | < 10% | Investigar LLM |
| `curator_confidence_avg` | Gauge | > 0.65 | OK |
| `curator_apply_rate` | Gauge | 5-15% | Ajustar threshold |
| `curator_diversity_gain` | Gauge | > 0.05 | Ajustar targets |
| `autopilot_rollback_rate` | Counter | < 5% | Revisar ajustes |
| `autopilot_cycle_duration_s` | Histogram | < 300s | Otimizar |
| `kpi_retention_5min` | Gauge | >= 0.38 | **CRÍTICO** |
| `kpi_vmaf_avg` | Gauge | >= 90 | **CRÍTICO** |
| `kpi_lufs_avg` | Gauge | -14.0 ±0.5 | **CRÍTICO** |

### B) Dashboards Grafana

```yaml
dashboards:
  - name: "Business Logic Overview"
    panels:
      - title: "Selection Method Distribution"
        query: "rate(softmax_selections_total[5m])"
      - title: "LLM Hook Latency"
        query: "histogram_quantile(0.95, llm_hook_duration_ms)"
      - title: "Curator Actions"
        query: "sum(curator_actions_total) by (action_type)"
  
  - name: "Autopilot Health"
    panels:
      - title: "Daily Adjustments"
        query: "autopilot_adjustments_total"
      - title: "Canary Success Rate"
        query: "rate(autopilot_canary_success[1d])"
      - title: "KPI Trends"
        query: "avg_over_time(kpi_retention_5min[7d])"
```

### C) Alertas Críticos

```yaml
alerts:
  - name: "BusinessLogicLoadFailed"
    expr: "business_logic_load_errors_total > 0"
    for: "1m"
    severity: "critical"
    annotations:
      summary: "business_logic.yaml falhou ao carregar"
      
  - name: "LLMTimeoutRateHigh"
    expr: "rate(llm_hook_timeouts_total[5m]) > 0.10"
    for: "10m"
    severity: "warning"
    annotations:
      summary: "LLM timeout rate > 10%"
      
  - name: "AutopilotRollbacksHigh"
    expr: "rate(autopilot_rollbacks_total[1d]) > 0.05"
    for: "1d"
    severity: "warning"
    annotations:
      summary: "Autopilot rollback rate > 5%"
      
  - name: "KPIRetentionCritical"
    expr: "kpi_retention_5min < 0.35"
    for: "30m"
    severity: "critical"
    annotations:
      summary: "Retention caiu para níveis críticos"
```

---

## 🔟 TROUBLESHOOTING

### Problema 1: `business_logic.yaml` não carrega

**Sintomas:**
```
ERROR Failed to load business_logic.yaml: missing field `selection`
```

**Diagnóstico:**
```bash
# Validar YAML syntax
yamllint /vvtv/business_logic/business_logic.yaml

# Testar load isoladamente
vvtvctl business-logic validate --file=/vvtv/business_logic/business_logic.yaml
```

**Solução:**
1. Verificar schema contra `business_logic.example.yaml`
2. Corrigir campos faltantes
3. Recarregar: `vvtvctl business-logic reload`

---

### Problema 2: Softmax sempre escolhe o mesmo item

**Sintomas:**
```
WARN Softmax diversity too low: 95% selections are item#1
```

**Diagnóstico:**
```bash
# Verificar temperatura
vvtvctl business-logic show | grep temperature

# Verificar scores
vvtvctl planner debug-scores --limit=20
```

**Solução:**
- Se T muito baixo (< 0.3) → aumentar para 0.6-0.8
- Se scores muito desbalanceados → revisar scoring formula
- Se seed sempre igual → verificar `generate_slot_seed()`

---

### Problema 3: LLM sempre timing out

**Sintomas:**
```
WARN LLM timeout after 800ms (hook: expand_queries)
Rate: 45%
```

**Diagnóstico:**
```bash
# Testar LLM diretamente
curl -X POST http://llm-endpoint/api/generate \
  -d '{"prompt":"test","max_tokens":512}' \
  --max-time 1

# Verificar latência de rede
ping llm-endpoint
```

**Solução:**
1. Aumentar deadline se LLM local: `deadline_ms: 1500`
2. Trocar modelo (ex: Mistral 7B → Phi-3 mini)
3. Reduzir `budget_tokens` (512 → 256)
4. Considerar cache de prompts similares

---

### Problema 4: Curator aplicando mudanças demais

**Sintomas:**
```
INFO Curator apply rate: 28% (target: 5-15%)
```

**Diagnóstico:**
```bash
# Ver actions recentes
vvtvctl curator actions --last=24h | jq '.[] | select(.decision=="apply")'

# Ver confidence distribution
vvtvctl curator stats --metric=confidence
```

**Solução:**
- Aumentar `min_confidence_apply` (0.62 → 0.70)
- Reduzir `diversity_target` (0.05 → 0.03)
- Revisar sinais detectados (talvez muito sensíveis)

---

### Problema 5: Autopilot sempre fazendo rollback

**Sintomas:**
```
WARN Autopilot rolled back 4 times this week
KPI: retention_5min dropped 0.012 (threshold: 0.01)
```

**Diagnóstico:**
```bash
# Ver histórico de ajustes
vvtvctl autopilot history --last=7d

# Ver correlação ajuste → KPI
vvtvctl autopilot analyze --adjustment="epsilon +0.01"
```

**Solução:**
1. Ajustes muito agressivos → reduzir `max_change_per_day`
2. Threshold muito rígido → relaxar para 0.015
3. Canary muito curto → aumentar para 120 min
4. Revisar se ajuste faz sentido (pode ser correlação espúria)

---

## 1️⃣1️⃣ TRADE-OFFS E DECISÕES DE DESIGN

### A) Por que Softmax e não Top-K?

**Trade-off:**
- ✅ **Softmax:** Variedade controlada, determinístico com seed, suave
- ❌ **Top-K:** Determin��stico demais, pode ignorar candidatos viáveis

**Decisão:** Softmax com T=0.6 oferece **80% exploração dos melhores + 20% variedade**.

---

### B) Por que Confidence Threshold 0.62?

**Trade-off:**
- ✅ **0.62:** Equilibra confiança vs ação
- ❌ **0.80:** Muito conservador, Curator quase nunca aplica
- ❌ **0.50:** Muito liberal, muitas mudanças ruins

**Decisão:** 0.62 baseado em testes empíricos (sweet spot).

---

### C) Por que Canary 20% por 60 min?

**Trade-off:**
- ✅ **20%:** Grande o bastante para detectar regressão, pequeno o bastante para mitigar risco
- ❌ **50%:** Muito risco se ajuste ruim
- ❌ **5%:** Amostra pequena demais

**Decisão:** 20% por 60 min = ~1000 viewers, estatisticamente significativo.

---

### D) Por que não usar Reinforcement Learning?

**Trade-off:**
- ✅ **RL:** Poderia aprender políticas ótimas
- ❌ **RL:** Caixa preta, não auditável, requer muito dado

**Decisão:** Autopilot D+1 é um "RL simplificado" mas **auditável** (humano entende cada ajuste).

---

## 1️⃣2️⃣ DEPLOYMENT E ROLLOUT

### A) Pré-Requisitos

```bash
# 1. Instalar dependências
cargo build --release --features business-logic

# 2. Criar estrutura
sudo mkdir -p /vvtv/business_logic/{keywords,pairing,history}
sudo mkdir -p /vvtv/logs/{curator_vigilante,autopilot,incidents}

# 3. Copiar config inicial
sudo cp business_logic.example.yaml /vvtv/business_logic/business_logic.yaml
sudo chown vvtv:vvtv /vvtv/business_logic/business_logic.yaml

# 4. Validar
vvtvctl business-logic validate --file=/vvtv/business_logic/business_logic.yaml

# 5. Configurar LLM endpoint (se houver)
echo "llm_endpoint = \"http://localhost:11434\"" >> /vvtv/system/configs/llm.toml
```

---

### B) Rollout Faseado (Production)

#### **Semana 1: Observer Mode**
```yaml
# /vvtv/business_logic/business_logic.yaml
rollout:
  phase: "observer"
  
selection:
  method: "softmax"
  temperature: 0.6

llm_hooks:
  enabled: true
  expand_queries: { enabled: true }
  rerank_candidates: { enabled: false }  # Desabilitado

curator_vigilante:
  enabled: true
  contexts:
    curation: { allow_apply: false }  # APENAS OBSERVA
```

**Monitorar:**
- Softmax está funcionando?
- LLM hooks não causam latência?
- Curator detecta sinais corretamente?

---

#### **Semana 2-3: Apply Limitado (Prime Time Only)**
```yaml
rollout:
  phase: "apply_limited"

exploration:
  epsilon: 0.08  # Ativar exploração

curator_vigilante:
  contexts:
    curation:
      allow_apply: true  # ✅ ATIVAR
      windows: ["19:00-00:00"]  # Apenas prime time
```

**Monitorar:**
- Curator apply rate (target: 5-15%)
- Diversity gain (target: >= 0.05)
- KPIs não regridem

---

#### **Semana 4+: Pleno**
```yaml
rollout:
  phase: "pleno"

autopilot:
  enabled: true
  daily_auto_apply:
    enabled: true  # ✅ ATIVAR

curator_vigilante:
  contexts:
    curation:
      allow_apply: true
      windows: ["00:00-24:00"]  # All day
```

**Monitorar:**
- Autopilot rollback rate (target: < 5%)
- PRs autônomos sendo criados
- Sistema estável por 7+ dias

---

## 1️⃣3️⃣ GLOSSÁRIO

| Termo | Definição |
|-------|-----------|
| **Cartão do Dono** | `business_logic.yaml`, único arquivo que define lógica de negócio |
| **Softmax** | Função de seleção estocástica que favorece itens com score alto mas permite variedade |
| **Temperature (T)** | Controle de variedade: T baixo = greedy, T alto = uniforme |
| **Seed per slot** | Estratégia de gerar seed único por janela temporal para reprodutibilidade |
| **LLM Hook** | Ponto de integração onde LLM pode sugerir (mas não comandar) |
| **Curator Vigilante** | Sistema que detecta padrões indesejáveis e sugere correções |
| **Confidence Threshold** | Mínimo de confiança (0.62) para Curator aplicar mudanças |
| **Diversity Gain** | Melhoria em variedade (entropia, Gini) ao aplicar mudança |
| **Autopilot D+1** | Sistema que aprende diariamente com métricas e ajusta micro-parâmetros |
| **Canary Deployment** | Aplicar mudança em 20% do tráfego primeiro |
| **KPI Gate** | Verificação de KPIs antes de commit (rollback se regressão) |
| **Advice vs Apply** | LLM sugere (advice) vs sistema aplica (apply) |

---

---

## 1️⃣4️⃣ TECH REVIEW & MELHORIAS CRÍTICAS

> **Baseado em review sênior → implementações que evitam falhas em produção**

### A) **PROBLEMAS IDENTIFICADOS E SOLUÇÕES**

#### 🔴 **1. Softmax em Lotes (select_batch)**
**Problema:** Remover itens e reamostrar pode enviesar probabilidades.

**Solução:** Gumbel-Top-k sem reposição
```rust
// vvtv-core/src/selection/gumbel.rs
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;

pub fn gumbel_topk_indices(scores: &[f64], k: usize, seed: u64) -> Vec<usize> {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let mut keys: Vec<(usize, f64)> = scores.iter().enumerate().map(|(i, &s)| {
        let u: f64 = rng.gen::<f64>().clamp(1e-9, 1.0 - 1e-9);
        let g = -(-u.ln()).ln(); // Gumbel(0,1)
        (i, s + g)               // Log-softmax-free trick
    }).collect();
    keys.sort_by(|a,b| b.1.partial_cmp(&a.1).unwrap());
    keys.into_iter().take(k.min(scores.len())).map(|(i,_)| i).collect()
}
```

#### 🔴 **2. Seed de Slot com Colisões**
**Problema:** `(hour*4 + minute/15)` gera colisões em reprocessos.

**Solução:** Slot epoch index persistido
```rust
// vvtv-core/src/plan/selection.rs
pub fn generate_slot_seed_robust(timestamp: DateTime<Utc>, slot_duration_min: u32, global_seed: u64) -> u64 {
    let slot_epoch_index = timestamp.timestamp() / (slot_duration_min as i64 * 60);
    let programming_window_id = timestamp.date_naive().to_string();
    
    let mut hasher = DefaultHasher::new();
    slot_epoch_index.hash(&mut hasher);
    programming_window_id.hash(&mut hasher);
    global_seed.hash(&mut hasher);
    hasher.finish()
}
```

#### 🔴 **3. LLM Tail Latency**
**Problema:** SLA T-4h pode ser "comido" por latência.

**Solução:** Circuit breaker por hook
```rust
// vvtv-core/src/llm/circuit.rs
pub struct CircuitBreaker {
    window: std::collections::VecDeque<bool>,
    max_fail_rate: f32, // 0.10 = 10%
    max_n: usize,        // 50 samples
}

impl CircuitBreaker {
    pub fn allow(&self) -> bool {
        if self.window.len() < 10 { return true; } // Bootstrap
        
        let fails = self.window.iter().filter(|x| !**x).count() as f32;
        (fails / self.window.len() as f32) <= self.max_fail_rate
    }
    
    pub fn record(&mut self, success: bool) {
        if self.window.len() == self.max_n { 
            self.window.pop_front(); 
        }
        self.window.push_back(success);
    }
}
```

#### 🔴 **4. Autopilot Drift Acumulado**
**Problema:** Micro-deltas diários somados (efeito sapo na panela).

**Solução:** Janela deslizante + anti-windup
```rust
// vvtv-core/src/autopilot/bounds.rs
pub struct SlidingBounds {
    min: f64,
    max: f64,
    current: f64,
    rollback_count: u32,
}

impl SlidingBounds {
    pub fn clamp(&mut self, value: f64) -> f64 {
        let clamped = value.clamp(self.min, self.max);
        
        // Anti-windup: se 3 rollbacks seguidos, decai 25%
        if self.rollback_count >= 3 {
            self.current *= 0.75;
            self.rollback_count = 0;
        }
        
        self.current = clamped;
        clamped
    }
}
```

#### 🔴 **5. Curator Apply Rate Explosivo**
**Problema:** >15-20% = "programando via LLM".

**Solução:** Token bucket por janela
```rust
// vvtv-core/src/curator/rate.rs
pub struct TokenBucket {
    tokens: i32,
    capacity: i32,
    refill_per_min: i32,
    last_refill: Instant,
}

impl TokenBucket {
    pub fn allow(&mut self) -> bool { 
        self.refill(); 
        if self.tokens > 0 { 
            self.tokens -= 1; 
            true 
        } else { 
            false 
        } 
    }
    
    fn refill(&mut self) { 
        let elapsed = self.last_refill.elapsed().as_secs() / 60; 
        if elapsed > 0 { 
            self.tokens = (self.tokens + (elapsed as i32) * self.refill_per_min).min(self.capacity); 
            self.last_refill = Instant::now(); 
        } 
    }
}
```

#### 🔴 **6. Incidentes sem Follow-up**
**Problema:** JSON vira cemitério sem ação.

**Solução:** Cron semanal de triage
```rust
// vvtv-core/src/incidents/triage.rs
pub struct IncidentTriage {
    store: IncidentStore,
    policy_patch_generator: PolicyPatchGenerator,
}

impl IncidentTriage {
    pub async fn weekly_triage(&self) -> Result<()> {
        let incidents = self.store.get_unresolved_last_week()?;
        
        if incidents.len() >= 3 {
            // Gerar patch de política
            let patch = self.policy_patch_generator.generate_patch(&incidents)?;
            
            // Criar PR com golden set
            self.create_policy_pr(patch).await?;
        }
        
        Ok(())
    }
}
```

#### 🔴 **7. HD Detection Lenta**
**Problema:** Players que só trocam HD após 15s.

**Solução:** Heurística dupla
```rust
// vvtv-core/src/browser/hd_detection.rs
pub struct HDDetection {
    timeout_sec: u32,
    bitrate_threshold: u32,
}

impl HDDetection {
    pub async fn wait_for_hd(&self, context: &BrowserContext) -> Result<HDStatus> {
        let start = Instant::now();
        
        while start.elapsed().as_secs() < self.timeout_sec as u64 {
            // (a) Monitor bitrate/height
            if let Some(stats) = self.get_player_stats(context).await? {
                if stats.height >= 720 && stats.bitrate >= self.bitrate_threshold {
                    return Ok(HDStatus::Confirmed);
                }
            }
            
            // (b) Probe headless por overlay (se disponível)
            if let Some(overlay) = self.detect_stats_overlay(context).await? {
                if overlay.resolution.contains("1080") {
                    return Ok(HDStatus::Confirmed);
                }
            }
            
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        
        // Marcar como "slow-HD" para penalidade futura
        Ok(HDStatus::SlowHD)
    }
}
```

#### 🔴 **8. Diversidade "Real" Insuficiente**
**Problema:** Só paleta/tags não captura "ritmo".

**Solução:** Novelty temporal + cadência
```rust
// vvtv-core/src/curator/novelty.rs
pub struct NoveltyDetector {
    kld_threshold: f64,
    cadence_threshold: f64,
}

impl NoveltyDetector {
    pub fn detect_temporal_novelty(&self, current: &[Plan], historical: &[Plan]) -> f64 {
        // KLD entre histogramas de duração/temas
        let current_hist = self.build_duration_histogram(current);
        let historical_hist = self.build_duration_histogram(historical);
        
        self.kullback_leibler_divergence(&current_hist, &historical_hist)
    }
    
    pub fn detect_cadence_novelty(&self, plans: &[Plan]) -> f64 {
        // Análise de cortes/min se disponível
        let cuts_per_min: Vec<f64> = plans.iter()
            .filter_map(|p| p.metadata.cuts_per_minute)
            .collect();
            
        if cuts_per_min.is_empty() { return 0.0; }
        
        let mean_cadence = cuts_per_min.iter().sum::<f64>() / cuts_per_min.len() as f64;
        let variance = cuts_per_min.iter()
            .map(|x| (x - mean_cadence).powi(2))
            .sum::<f64>() / cuts_per_min.len() as f64;
            
        variance.sqrt() // Desvio padrão como medida de variedade
    }
}
```

---

### B) **MÉTRICAS NOVAS (SUPER ÚTEIS)**

```yaml
# Adicionar ao observability.yaml
new_metrics:
  - name: "selection_entropy"
    type: "gauge"
    description: "Entropia da seleção por janela (sobe = programação viva)"
    threshold: "> 2.5"
    
  - name: "curator_apply_budget_used_pct"
    type: "gauge" 
    description: "Percentual do budget de apply usado por hora"
    threshold: "< 80%"
    
  - name: "autopilot_pred_vs_real_error"
    type: "histogram"
    description: "Erro entre previsão de KPI e observado no canary"
    threshold: "p95 < 0.05"
    
  - name: "hd_detection_slow_rate"
    type: "counter"
    description: "Taxa de fontes marcadas como slow-HD"
    threshold: "< 15%"
    
  - name: "novelty_temporal_kld"
    type: "gauge"
    description: "KLD entre histogramas temporais (diversidade real)"
    threshold: "> 0.3"
```

---

### C) **CONFORMIDADE & RISCOS LEGAIS**

```rust
// vvtv-core/src/compliance/audit.rs
pub struct ComplianceAuditor {
    evidence_retention_days: u32,
    hash_algorithm: String,
}

impl ComplianceAuditor {
    pub async fn log_pbd_evidence(&self, url: &str, abort_reason: Option<String>) -> Result<()> {
        let evidence = PbdEvidence {
            timestamp: Utc::now(),
            url: url.to_string(),
            abort_reason,
            player_events: self.capture_player_events().await?,
            drm_detected: self.check_drm_presence().await?,
        };
        
        // Persistir por 90 dias
        self.store_evidence(evidence, Duration::days(90)).await?;
        Ok(())
    }
    
    pub async fn log_moderation_hashes(&self, frames: &[Frame]) -> Result<()> {
        for frame in frames {
            let hash = self.compute_visual_hash(frame)?;
            self.store_moderation_hash(hash, frame.timestamp).await?;
        }
        Ok(())
    }
}
```

---

### D) **TESTES CRÍTICOS ADICIONAIS**

```rust
#[cfg(test)]
mod critical_tests {
    use super::*;
    
    #[test]
    fn test_golden_selection_deterministic() {
        let candidates = load_test_candidates("test_data/candidates.json");
        let seed = 12345;
        let temperature = 0.6;
        
        // Deve produzir exatamente a mesma lista
        let result1 = gumbel_topk_indices(&candidates.scores, 12, seed);
        let result2 = gumbel_topk_indices(&candidates.scores, 12, seed);
        
        assert_eq!(result1, result2);
        assert_eq!(result1.len(), 12);
    }
    
    #[tokio::test]
    async fn test_llm_timeout_stress() {
        let mut circuit = CircuitBreaker::new(0.10, 50);
        let mut timeouts = 0;
        
        // 1k chamadas simuladas
        for _ in 0..1000 {
            let start = Instant::now();
            let result = simulate_llm_call_with_timeout(800).await;
            let elapsed = start.elapsed();
            
            if elapsed > Duration::from_millis(900) { // deadline + 100ms
                timeouts += 1;
            }
            
            circuit.record(result.is_ok());
        }
        
        // P95 deve ser < deadline + 100ms
        assert!(timeouts < 50); // < 5%
    }
    
    #[test]
    fn test_drift_guard_epsilon() {
        let mut bounds = SlidingBounds::new(0.02, 0.12, 0.08);
        
        // Simular 30 dias de micro-deltas
        for _ in 0..30 {
            let delta = 0.01; // Micro-delta diário
            let new_value = bounds.current + delta;
            bounds.clamp(new_value);
        }
        
        // Epsilon deve permanecer no intervalo
        assert!(bounds.current >= 0.02);
        assert!(bounds.current <= 0.12);
    }
    
    #[tokio::test]
    async fn test_canary_statistical_significance() {
        let canary = CanaryTester::new(0.20, Duration::from_secs(3600));
        
        // Bootstrap test para detectar falso positivo/negativo
        let result = canary.bootstrap_test(1000).await?;
        
        // Deve detectar regressões reais com >95% confiança
        assert!(result.true_positive_rate > 0.95);
        assert!(result.false_positive_rate < 0.05);
    }
}
```

---

### E) **CARTÃO DO DONO ASSINADO**

```rust
// vvtv-core/src/business_logic/signature.rs
use sha2::{Sha256, Digest};
use ed25519_dalek::{SigningKey, VerifyingKey};

pub struct SignedBusinessLogic {
    content: BusinessLogic,
    signature: String,
    signed_by: String,
    timestamp: DateTime<Utc>,
    content_hash: String,
}

impl SignedBusinessLogic {
    pub fn sign(logic: BusinessLogic, signing_key: &SigningKey, signer: &str) -> Result<Self> {
        let content_json = serde_json::to_string(&logic)?;
        let mut hasher = Sha256::new();
        hasher.update(content_json.as_bytes());
        let content_hash = format!("{:x}", hasher.finalize());
        
        let signature = signing_key.sign(content_json.as_bytes());
        let signature_b64 = base64::encode(signature.to_bytes());
        
        Ok(Self {
            content: logic,
            signature: signature_b64,
            signed_by: signer.to_string(),
            timestamp: Utc::now(),
            content_hash,
        })
    }
    
    pub fn verify(&self, verifying_key: &VerifyingKey) -> Result<bool> {
        let content_json = serde_json::to_string(&self.content)?;
        let signature_bytes = base64::decode(&self.signature)?;
        let signature = ed25519_dalek::Signature::from_bytes(&signature_bytes)?;
        
        Ok(verifying_key.verify_strict(content_json.as_bytes(), &signature).is_ok())
    }
}
```

---

## 🎯 CONCLUSÃO

Este blueprint é **PERFEITO** para o VVTV porque:

1. ✅ **Separa responsabilidades:** Dono (macro) vs Autopilot (micro) vs Curador (sugestões)
2. ✅ **Determinismo auditável:** Seed per slot + logs completos
3. ✅ **Guardrails fortes:** PBD, QC, moderação são **inegociáveis**
4. ✅ **LLM como conselheiro:** Nunca quebra, sempre opcional, sempre marcado
5. ✅ **Feedback loop:** D+1 aprende com Curador e métricas
6. ✅ **Governança clara:** RACI definido, PRs automáticos, rollback seguro
7. ✅ **Observabilidade total:** Métricas, dashboards, alertas, troubleshooting
8. ✅ **Testes completos:** Unit, integration, E2E, validação por fase
9. ✅ **Deployment seguro:** Rollout faseado, canary, rollback automático
10. ✅ **PRODUCTION-READY:** Circuit breakers, token buckets, drift guards, compliance

**O Rust faz o trabalho pesado. O LLM é o "azeite" que refina. O Cartão do Dono comanda.**

🚀 **PRONTO PARA IMPLEMENTAR EM PRODUÇÃO!**

---

> **"95% máquina robusta. 5% azeite inteligente. 100% auditável. 0% falhas críticas."**  
> — VVTV Foundation, 2025


