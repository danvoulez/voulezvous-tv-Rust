# 🎯 EXECUTIVE SUMMARY — VVTV Business Logic Integration

> **Data:** 2025-10-21  
> **Contexto:** Integração do "Cartão do Dono" (Blueprint Business Logic) ao Motor Rust  
> **Status:** Especificação completa ✅ | Implementação pendente ⏳

---

## 📊 RESUMO EXECUTIVO

Integramos o **Blueprint do Cartão Perfurado** (lógica de negócio declarativa) ao sistema VVTV, criando um **modelo híbrido** onde:

- **95% é Rust determinístico** (motor pesado)
- **5% é LLM consultivo** (azeite, refinamento)
- **Autopilot D+1** aprende e ajusta automaticamente

**Metáfora:** A máquina robusta faz o trabalho. O LLM sussurra sugestões. O Dono perfura o cartão que define o rumo.

---

## 🎛️ O CARTÃO DO DONO

### Conceito Central

**Um único arquivo YAML** (`business_logic.yaml`) define **toda a lógica de negócio**:

```yaml
knobs:
  boost_bucket: "high-retention-core"
  music_mood_focus: ["downtempo", "electro-pop"]
  plan_selection_bias: 0.10

selection:
  method: "softmax"
  temperature: 0.6
  seed_strategy: "slot_hash"

autopilot:
  enabled: true
  curator_feedback_loop:
    enabled: true
```

**Hot reload sem restart:**
```bash
vvtvctl business-logic reload
```

---

## 🏗️ ARQUITETURA HÍBRIDA

```
┌─────────────────────────────────────────────────┐
│         RUST ENGINE (95%)                       │
│  • Softmax selection (T=0.6, seed per slot)    │
│  • Scoring (6 fatores)                          │
│  • Diversity enforcement                        │
│  • Queue management (FIFO + bump + ratio)      │
│  • Emergency loop                               │
│  • QC thresholds                                │
└─────────────────────────────────────────────────┘
                      ↕
┌─────────────────────────────────────────────────┐
│         LLM CURADOR (5%)                        │
│  • expand_queries (800ms SLA)                   │
│  • rerank_candidates (900ms SLA)                │
│  • recovery_plan (1500ms SLA)                   │
│  • Advice vs Apply (confidence threshold)      │
└─────────────────────────────────────────────────┘
                      ↕
┌─────────────────────────────────────────────────┐
│         AUTOPILOT D+1 (Feedback Loop)           │
│  • Daily cycle (03:00 UTC)                      │
│  • Read metrics D-1 + curator signals           │
│  • Calculate safe adjustments                   │
│  • Apply canary (20%, 60 min)                   │
│  • Validate KPIs → Commit or Rollback           │
└─────────────────────────────────────────────────┘
```

---

## 🎯 PAPÉIS E RESPONSABILIDADES (RACI)

| Item | Responsável | Accountable | Frequência |
|------|-------------|-------------|------------|
| **Knobs macro (Cartão)** | Dan | Dan | Mensal / D+2 |
| Negatives/boosts leve | Autopilot + Curador | Dan (observa) | Diário |
| QC/Moderação/Guardrails | Time técnico | Dan | Raro (evidência) |
| PRs baixo risco | VVTV Bot | Dan (override) | Diário |
| Revisão mensal | Autopilot (gera) | 1 aprovador | Mensal (dia 1) |

**Princípio:** Separação clara de responsabilidades. Dono comanda macro. Autopilot refina micro. Sistema garante guardrails.

---

## 🤖 LLM: BÚSSOLA, NÃO COMANDO

### 5 Hooks com SLA

1. **expand_queries** (800ms): Adiciona keywords quando busca falha
2. **site_tactics** (1200ms): Táticas de navegação em sites difíceis
3. **rerank_candidates** (900ms): Reordena top-K por diversidade
4. **recovery_plan** (1500ms): Recovery quando catalogue < 80%
5. **enrich_metadata** (1000ms): Normaliza título, infere mood

### Regras de Ouro

✅ **Toda ação LLM é marcada:**
```json
{
  "llm_action": {
    "source": "rerank_candidates",
    "model": "gpt-4o",
    "confidence": 0.78,
    "reason": "paleta distinta + duração ideal"
  }
}
```

✅ **Fallback obrigatório:**
- Timeout → `keep_original`
- Confidence < 0.62 → `advice` (não aplica)
- Confidence >= 0.62 → `apply` (se contexto permitir)

✅ **Nunca quebra o sistema:**
- Deadline rígido (timeout sempre respeitado)
- Fallback sempre definido
- Logs auditáveis em `logs/curator_vigilante/`

---

## 🎨 CURATOR VIGILANTE: CONSELHEIRO DE DIVERSIDADE

### Sinais Detectados

- **Palette similarity** (cosine > 0.85)
- **Tag duplication** (jaccard > 0.75)
- **Duration streak** (3+ consecutivos)
- **Bucket imbalance** (> 70% rolling)
- **Theme near-duplicates** (CLIP > 0.82)
- **Pose/scene repeat** (thumbnail hamming < 6)

### Decisão Inteligente

```
SE confidence >= 0.62
E diversity_gain >= 0.05
E não locked
  → APLICA mudança
SENÃO
  → ADVICE (registra em audit/)
```

### Logs Auditáveis

```jsonl
{
  "ts": "2025-10-21T21:28:12Z",
  "signal": "palette_similarity",
  "value": 0.87,
  "action": "reorder",
  "from_idx": 4,
  "to_idx": 7,
  "llm_action": { "model": "gpt-4o", "confidence": 0.78 },
  "diversity_gain": 0.08
}
```

---

## 🔄 AUTOPILOT D+1: APRENDIZADO CONTÍNUO

### Ciclo Diário (03:00 UTC)

```
1. COLETA métricas D-1
   └─ retention_5min, vmaf_avg, lufs_avg
   └─ Sinais do Curador (palette, tags, etc)

2. CALCULA ajustes seguros
   └─ Se palette > 0.80 → reduz boost em 0.04
   └─ Se diversity < 0.03 → aumenta epsilon em 0.01
   └─ Se retention < 0.38 → aumenta bias em 0.02

3. APLICA canary (20%, 60 min)
   └─ Monitora KPIs em tempo real
   └─ Rollback automático se regressão

4. COMMIT ou ROLLBACK
   └─ Se KPIs OK → commit em business_logic.yaml
   └─ Se KPIs NOK → rollback + alerta
```

### Exemplo de Ajuste

```json
{
  "date": "2025-10-22",
  "adjustments": [
    {
      "path": "keywords.videos.buckets.high-retention-core.boosts",
      "change": -0.03,
      "reason": "Palette similarity alta (0.84)"
    }
  ],
  "canary_result": "applied",
  "metrics_delta": { "retention_5min": +0.001, "vmaf_avg": +0.2 }
}
```

---

## 📚 INCIDENT LEARNING: LOOP DE FEEDBACK

### Registro de Incidentes

Quando algo dá errado, sistema registra **exatamente o que aconteceu** e **o que poderia ter evitado**:

```json
{
  "incident": "low_vmaf_playout",
  "ts": "2025-10-21T22:15:54Z",
  "plan_id": "pl_abc123",
  "slot_id": 18,
  "llm_warnings": ["advice ignored"],
  "original_path": "candidate#2 score 0.51 chosen",
  "recovery_candidate": "candidate#4 score 0.49 (vmaf est. 90)",
  "why_not_chosen": "confidence<0.62",
  "kpi_impact": { "vmaf_delta": -5, "retention_5min_delta_pp": -0.8 }
}
```

### Ações Automáticas

1. ✅ Entra como dado no **audit diário**
2. ✅ Se repetido 3x → **gating review** (escalona para humano)
3. ✅ Autopilot aprende a **ajustar thresholds** baseado em histórico

---

## 🚀 ROLLOUT EM 3 FASES

### Fase 1: Observer (Semana 1)
- Softmax T=0.6, epsilon=0
- LLM: apenas `expand_queries`
- Curator: **advice-only** (nunca aplica)
- **Objetivo:** Coletar baseline

### Fase 2: Apply Limitado (Semana 2-3)
- Epsilon=0.08 (exploração leve)
- Curator: `allow_apply=true` apenas em **prime time**
- Canary + rollback ativos
- Autopilot: apenas logs (não commit)

### Fase 3: Pleno (Semana 4+)
- Curator: aplica em prime e off-peak
- Autopilot: commit automático (baixo risco)
- `recovery_plan` ativo quando catalogue < 80%
- GitHub App: auto-merge PRs low-risk

---

## 📦 DELIVERABLES CRIADOS

### 1. Documentação

| Arquivo | Propósito |
|---------|-----------|
| **BUSINESS_LOGIC_INTEGRATION.md** | Mapeamento completo Blueprint → Rust |
| **business_logic.example.yaml** | Exemplo completo do Cartão (400+ linhas) |
| **AGENTS.md** | Atualizado com nova seção Business Logic |
| **EXECUTIVE_SUMMARY.md** | Este documento |

### 2. Estrutura de Implementação

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
├── logs/
│   ├── curator_vigilante/
│   │   └── 2025-10-21.jsonl         ← Auditoria Curador
│   ├── autopilot/
│   │   └── 2025-10-21.jsonl         ← Decisões D+1
│   └── incidents/
│       └── 2025-10/
│           └── incident_001.json
```

### 3. Código Rust (Especificado, não implementado)

| Módulo | Linhas | Status |
|--------|--------|--------|
| `business_logic/mod.rs` | ~300 | ⏳ Especificado |
| `plan/selection.rs` (Softmax) | ~150 | ⏳ Especificado |
| `llm/mod.rs` (Hooks) | ~400 | ⏳ Especificado |
| `curator/vigilante.rs` | ~500 | ⏳ Especificado |
| `autopilot/mod.rs` | ~600 | ⏳ Especificado |
| **TOTAL** | ~2,000 | ⏳ Especificado |

---

## 🎯 ROADMAP DE IMPLEMENTAÇÃO

### Semana 1: Core (Fase 1) — CRÍTICO
- [ ] `business_logic.yaml` schema + loader
- [ ] Softmax selector
- [ ] Seed per slot
- [ ] Integration com Planner
- [ ] CLI: `vvtvctl business-logic {reload|show|validate}`
- [ ] Tests unitários

### Semana 2: LLM (Fase 2) — IMPORTANTE
- [ ] LLM client com timeout
- [ ] Hook system (5 hooks)
- [ ] Curator Vigilante
- [ ] Signal detection
- [ ] Audit logging
- [ ] Tests com mock LLM

### Semana 3: Autopilot (Fase 3) — DESEJÁVEL
- [ ] Daily cycle runner
- [ ] Metrics aggregation D-1
- [ ] Adjustment calculator
- [ ] Canary deployment
- [ ] KPI validation
- [ ] Rollback mechanism
- [ ] YAML commit/history

### Semana 4: Polish & Deploy — FINAL
- [ ] Monthly review generator
- [ ] GitHub App integration
- [ ] Incident learning system
- [ ] Documentation completa
- [ ] E2E tests
- [ ] Deploy production

**Tempo estimado:** 3-4 semanas  
**Complexidade:** Média-Alta  
**Risco:** Médio (requer testes extensivos antes de prod)

---

## ✅ CRITÉRIOS DE SUCESSO

### Funcionais

1. ✅ `business_logic.yaml` carrega e valida em <100ms
2. ✅ Softmax selection executa em <10ms por batch
3. ✅ LLM hooks respeitam SLA (timeout sempre)
4. ✅ Curator aplica mudanças apenas se confidence >= 0.62
5. ✅ Autopilot D+1 completa ciclo em <5min
6. ✅ Rollback automático se KPI cai além de threshold
7. ✅ Todos logs são auditáveis (JSON/JSONL)

### Não-Funcionais

1. ✅ Zero downtime em hot reload
2. ✅ Fallback gracioso em falhas de LLM
3. ✅ Canary deployment protege produção
4. ✅ Incident learning acumula conhecimento
5. ✅ PRs autônomos seguem manifesto padrão
6. ✅ Documentação completa para operadores

### KPIs de Negócio

1. ✅ Diversity Gini: 0.40–0.60 em prime time
2. ✅ Retention 5min: >= 38%
3. ✅ VMAF avg: >= 90
4. ✅ LUFS avg: -14.0 ±0.5
5. ✅ Curator action ratio: 5–15% de ações aplicadas
6. ✅ Autopilot rollback rate: < 5%

---

## 🎓 LIÇÕES & PRINCÍPIOS

### 1. Separação de Responsabilidades

> "Dono perfura o cartão. Máquina executa. LLM sussurra. Autopilot aprende."

- **Dono:** Define macro (knobs, scheduling, windows)
- **Rust Engine:** Executa determinístico (95% do trabalho)
- **LLM Curador:** Sugere refinamentos (5% azeite)
- **Autopilot:** Aprende e ajusta micro diariamente

### 2. Guardrails Inegociáveis

> "PBD, QC e Moderação são absolutos. LLM nunca os toca."

- PBD obrigatório (enforce_pbd: true)
- Abort on DRM (abort_on_drm: true)
- QC thresholds fixos (VMAF >= 85, LUFS -14±0.5)
- Moderação CSAM/non-consensual (HARD_STOP)

### 3. Auditabilidade Total

> "Se não está em log auditável, não aconteceu."

- Toda ação LLM marcada com `llm_action{source, model, confidence}`
- Curator Vigilante registra em `logs/curator_vigilante/`
- Autopilot registra em `logs/autopilot/`
- Incidents registram em `incidents/%Y-%m/`

### 4. Fail-Safe por Design

> "Timeout sempre. Fallback sempre. Rollback sempre disponível."

- LLM hooks: deadline rígido → timeout → fallback
- Curator: confidence < threshold → advice (não aplica)
- Autopilot: canary 20% → validate KPIs → commit ou rollback
- Emergency loop: buffer < 1h → inject assets

### 5. Determinismo Estocástico

> "Variedade auditável. Seed per slot garante reprodutibilidade."

- Softmax com temperatura (T=0.6)
- Seed = hash(YYYYMMDD | slot_id | global_seed)
- ε-greedy (8% escolhe 2ª/3ª melhor)
- Logs incluem seed usado

---

## 🔗 REFERÊNCIAS

### Documentos Criados
- **BUSINESS_LOGIC_INTEGRATION.md** — Spec técnica completa (300+ linhas)
- **business_logic.example.yaml** — Exemplo completo do Cartão (400+ linhas)
- **AGENTS.md** — Guia de implementação atualizado
- **EXECUTIVE_SUMMARY.md** — Este documento

### Documentos Existentes
- **VVTV INDUSTRIAL DOSSIER.md** — Spec técnica do sistema (5,500+ linhas)
- **Tasklist.md** — Roadmap de implementação (Epics A-H)
- **PROJECT_STATUS_COMPLETE.md** — Status atual (73% completo)
- **BUSINESS_LOGIC_MAP.md** — Mapeamento de lógica existente

### Código Rust Existente
- `vvtv-core/src/plan/planner.rs` — Scoring já implementado
- `vvtv-core/src/queue.rs` — Music ratio + curation bump
- `vvtv-core/src/browser/pbd.rs` — PBD completo
- `vvtv-core/src/processor/mod.rs` — QC thresholds

---

## 🎉 CONCLUSÃO

Integramos o **"Cartão do Dono"** ao sistema VVTV, criando uma **orquestra computável** onde:

✅ **95% é máquina Rust pesada** (determinística, auditável, resiliente)  
✅ **5% é LLM azeite** (refinamento, sugestões, nunca comando)  
✅ **Autopilot D+1** aprende continuamente (feedback loop)  
✅ **Governança clara** (RACI definido, PRs autônomos)  
✅ **Rollout em fases** (observer → apply limitado → pleno)

**O resultado:** Um sistema que opera autonomamente mas **sempre sob supervisão declarativa do Dono via YAML**.

---

> **"A máquina não decide o rumo. O Cartão decide. A máquina executa brilhantemente."**  
> — VoulezVous Foundation, 2025


