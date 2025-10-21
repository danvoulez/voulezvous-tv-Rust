# 🗺️ Epic P — Business Logic & Intelligent Curation

> **Roadmap de Engenharia de Produção**  
> Versão: 1.0  
> Data: 2025-10-21  
> Status: Ready for Implementation

---

## 📋 ÍNDICE

1. [Roadmap Refinado](#roadmap-refinado)
2. [PR P1 — Core Business Logic Engine](#pr-p1--core-business-logic-engine)
3. [PR P2 — LLM Integration & Circuit Breakers](#pr-p2--llm-integration--circuit-breakers)
4. [PR P3 — Curator Vigilante & Token Bucket](#pr-p3--curator-vigilante--token-bucket)
5. [PR P4 — Autopilot D+1 & Drift Guards](#pr-p4--autopilot-d1--drift-guards)
6. [PR P5 — HD Detection & Compliance](#pr-p5--hd-detection--compliance)
7. [PR P6 — Observability & Production Metrics](#pr-p6--observability--production-metrics)
8. [PR P7 — Testing & Validation Suite](#pr-p7--testing--validation-suite)
9. [CI/CD Gates & Templates](#cicd-gates--templates)
10. [Riscos & Mitigações](#riscos--mitigações)
11. [RACI & DOR/DoD](#raci--dordo)
12. [Estimativas e Marcos](#estimativas-e-marcos)

---

## 🗺️ Roadmap Refinado (Dependências & Paralelismo)

```
P1 ─┬─> P2 ─┬─> P3
    │       └─> P6
    └─> P5  └─> P7
         └────────^
            (observability & tests suportam P5)
P4 depende de P1+P2 (hooks) + P6 (métricas) + P7 (tests)
```

### **Começo em Paralelo Viável:**

- **P1 (Core)** e **P5 (HD/Compliance)** podem andar juntos
- **P6 (Observability)** e **P7 (Testes)** começam cedo para "iluminar" P2–P5
- **P4 (Autopilot)** precisa de P1, P2 (hooks), P6 (métricas) e P7 (suíte)

---

## 🧩 PR P1 — Core Business Logic Engine (YAML → Rust)

**Objetivo:** Cartão do Dono carregado, validado, aplicado; seleção Gumbel-Top-k determinística; seed robusto por slot; logs auditáveis; CLI.

### **Itens (Refinados)**

- `BusinessLogic` com `serde_yaml`, `validate()` (bounds, enums, ranges)
- **Gumbel-Top-k** sem reposição (determinístico com `ChaCha20Rng`):
  - `selection/gumbel.rs::gumbel_topk_indices(scores:&[f64], k, seed)->Vec<usize>`
- `generate_slot_seed_robust(now, slot_duration, window_id, global_seed)`:
  - seed derivado de `slot_epoch_index = floor(ts/slot_duration)` + `window_id` persistido (tabela `programming_windows`)
- Integração no `Planner`:
  - Aplica `plan_selection_bias` antes da seleção
  - Registra `seed`, `T`, `k`, `indices` e `scores` em log estruturado
- CLI (`vvtvctl`): `business-logic reload|show|validate`

### **Critérios de Aceite**

- `business_logic.yaml` carrega em <100ms, valida bounds e enums
- Com `seed` fixo, a seleção retorna **a mesma ordem** (teste golden)
- Seleção de 12 itens leva p95 < 10ms (100 candidatos)
- Logs em JSONL incluem `seed`, `temperature`, `k`, `indices`, `scores_norm`

### **Definition of Done**

- Unit tests: loader (válido/erro), gumbel (determinismo), seed (fuso/DS)
- Bench micro (`criterion`) para seleção
- Doc `BUSINESS_LOGIC_README.md` (schema + exemplos)

**Estimativa:** 3–4 dias

---

## 🤖 PR P2 — LLM Integration & Circuit Breakers

**Objetivo:** Hooks com SLA, timeouts rígidos, fallback e circuit breaker por hook.

### **Itens**

- `LLMClient` com `timeout(hook.deadline_ms)` + `fallback()`
- **CircuitBreaker** por hook: janela 50 chamadas, fail rate >10% → **open** 5 min
- Hooks (5): `expand_queries`, `site_tactics`, `rerank_candidates`, `recovery_plan`, `enrich_metadata`
- **LLMAction** marking em outputs (source/model/reason/confidence)
- Stress test: 1k calls, p95 ≤ deadline+100ms

### **Critérios de Aceite**

- Quando circuit **open**, hook retorna `advice-only` em <10ms (short-circuit)
- Todos os artefatos influenciados por LLM incluem `llm_action{...}` nos logs
- Nenhum hook bloqueia o `Planner` além do `deadline_ms`

### **DoD**

- Unit tests: timeout, fallback, breaker open/half-open/close
- Fixture de mock LLM (latência configurável)
- Doc `LLM_HOOKS.md` com prompts e SLA

**Estimativa:** 4 dias

---

## 🎭 PR P3 — Curator Vigilante & Token Bucket

**Objetivo:** Sinais + decisão `Advice` vs `Apply` com limite de aplicações/hora.

### **Itens**

- Detectores: `palette_similarity`, `tag_duplication (Jaccard)`, `duration_streak`, `bucket_imbalance`
- **Novelty temporal (KLD)** e **cadência** (cortes/min) adicionados aos sinais
- `TokenBucket`: 6 applies/hora, refill 6/h, capacidade 6 (configurável)
- Threshold de confiança `0.62`; `max_reorder_distance=4`; `never_remove_items=true`
- Logs em `logs/curator_vigilante/*.jsonl` (inclui `llm_action`)

### **Critérios de Aceite**

- `apply_rate` controlado (target 5–15%); bloqueia ao estourar bucket
- Reordenação respeita `max_reorder_distance`
- Quando `locked=true`, apenas `advice`

### **DoD**

- Tests: cada detector, threshold, token bucket, janela de prime-only
- Doc `CURATOR_VIGILANTE.md` (sinais, thresholds, exemplos)

**Estimativa:** 4 dias

---

## 🔁 PR P4 — Autopilot D+1 & Drift Guards

**Objetivo:** Ciclo diário 03:00 UTC, micro-afinagens seguras, canary, rollback, anti-drift.

### **Itens**

- Runner diário (`cron` ou serviço interno)
- `SlidingBounds` com **anti-windup** (ex.: decair 25% após rollback ×3)
- Canary 20%/60min com `kpi-gate`
- Commit no `business_logic.yaml` (histórico `history/`)
- Weekly **incident triage**: abre PR com patch + golden set

### **Critérios de Aceite**

- `autopilot_pred_vs_real_error` p50 < 20% (erro entre previsto e observado)
- `rollback_rate` < 5% (janela 14 dias)
- Mudanças respeitam bounds do Cartão (validador)

### **DoD**

- Integration tests simulando 3 dias (aplica → canary → gate → commit/rollback)
- Doc `AUTOPILOT.md` (deltas permitidos, exemplos, gates)

**Estimativa:** 4–5 dias

---

## 📺 PR P5 — HD Detection & Compliance

**Objetivo:** Garantir HD de fato via PBD + evidências de conformidade.

### **Itens**

- Heurística dupla:
  - (a) **monitor bitrate/height** durante playback (headless)
  - (b) **stats overlay** quando existir (sem violar ToS)
- Marcar fontes `slow-HD` → **penalidade** em scoring/plan
- `ComplianceAuditor`: evidências (player events, razões de abort) guardadas 90 dias
- Hashes de thumbnails/frames (moderação) + logs PBD (timestamp, domínio, razão)

### **Critérios de Aceite**

- `hd_detection_slow_rate` mensurado; penalidade aplicada só após 2 ocorrências
- Evidências serializadas por 90 dias (`/logs/compliance/YYYY-MM/`)
- Nenhuma violação de ToS (somente "play" e leitura de telemetria visível)

### **DoD**

- Tests: classificação `slow-HD`, retenção de logs, penalidade aplicada
- Doc `COMPLIANCE.md` (o que coletamos, retenção, privacidade)

**Estimativa:** 4 dias (em paralelo com P1)

---

## 📊 PR P6 — Observability & Production Metrics

**Objetivo:** Medir o que interessa e ver.

### **Itens**

- Métricas novas:
  - `selection_entropy`
  - `curator_apply_budget_used_pct`
  - `autopilot_pred_vs_real_error`
  - `hd_detection_slow_rate`
  - `novelty_temporal_kld`
- 2 dashboards Grafana: **Business Logic Overview** & **Autopilot Health**
- 4 alertas críticos (como definiste)
- Guia de troubleshooting com 5 cenários (já tens rascunho — consolidar)

### **Critérios de Aceite**

- Todos os painéis renderizam com dados reais (sem NaN)
- Alertas disparam corretamente (teste de injeção)

### **DoD**

- Dashboards versionadas (JSON)
- Doc `OBSERVABILITY.md` (queries, thresholds, SLO)

**Estimativa:** 3 dias (começar cedo)

---

## 🧪 PR P7 — Testing & Validation Suite

**Objetivo:** Garantia de qualidade contínua.

### **Itens**

- Golden selection test
- LLM timeout stress test (1k calls)
- Drift guard test (30 dias simulados)
- Canary bootstrap test (>95% confiança)
- Checklists por fase (P1–P4)

### **Critérios de Aceite**

- CI verde com **tempo total < 12 min** (paralelizar jobs)
- Falhas geram relatórios legíveis (coverage & perf)

### **DoD**

- `make test`, `make bench`, `make e2e` padronizados
- PR template com seção **"Impacto em KPIs"**

**Estimativa:** 3 dias (contínuo, em paralelo)

---

## 🔐 CI/CD Gates & Templates

### **Gates por PR**

- **P1:** `business_logic_validate`, `golden_selection`, `perf_selector_p95`
- **P2:** `llm_timeout_p95`, `breaker_behavior`
- **P3:** `curator_limits`, `apply_budget`
- **P4:** `autopilot_sim_dryrun`, `kpi_gate_sim`
- **P5:** `hd_detector_tests`
- **P6:** `dashboards_smoke`, `alerts_fire_test`
- **P7:** `canary_bootstrap_significance`

### **Template de PR**

```markdown
### Mudanças
- ...

### Impacto em KPIs (estimado)
- retention_5min: +0.2pp (canary 20% / 60min)

### Risco & Mitigação
- ...

### Checklist
- [ ] Tests verdes
- [ ] Logs/metrics adicionados
- [ ] Bounds respeitados (validator)
```

---

## 🧭 Riscos & Mitigações

| Risco | Mitigação |
|-------|-----------|
| Exploração excessiva (queda de retenção) | Bounds em `epsilon`, canary + rollback |
| LLM latente | Circuit breaker + advice-only + cache prompts |
| Curator "mandando demais" | Token bucket + threshold 0.62 + apply window |
| Drift acumulado | SlidingBounds + anti-windup + weekly triage |
| HD não confirmado | Heurística dupla + penalidade `slow-HD` |

---

## 👥 RACI & DOR/DoD

- **R**ealização: time core (Rust) + suporte MLOps (LLM local)
- **A**ccountable: Dan (Cartão do Dono / bounds)
- **C**onsulted: Operações (streaming/NGINX/FFmpeg)
- **I**nformed: Conteúdo/Editorial

### **Definition of Ready (por PR)**

- Requisitos claros + bounds definidos + fixtures de dados

### **Definition of Done**

- Critérios de aceite cumpridos + docs + métricas + testes

---

## ⏱️ Estimativas e Marcos

- **Semana 1–2:** P1, P5, P6/P7 (parciais) ✅
- **Semana 3:** P2, P3 + observability alimentando decisões ✅
- **Semana 4:** P4 (autopilot) + polish + rollout faseado ✅

*(Buffer: +1 semana para hardening caso necessário)*

---

## 📎 Pequenos Refinamentos Pro Backlog

- `selection_entropy` por janela (aplicar watchdog: se cair por 48h, aumentar T em +0.05 via autopilot)
- `signed_snapshot` do Cartão do Dono (`sha256`, `signed_by`)
- `curator_anima_gini` no dashboard (faixa saudável 0.40–0.60)

---

## 🎯 Veredito

Planilha de engenharia **enxuta e poderosa**. Com esses **critérios de aceite**, **DoD** e **gates de CI**, você tem **previsibilidade** e **segurança** para escalar.

---

> **"Epic P: Onde a máquina encontra a alma com engenharia de produção."**  
> — VVTV Foundation, 2025
