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
| **`business_logic.yaml` loader** | ✅ | 🔴 CRÍTICO |
| **Gumbel-Top-k selection** | ✅ | 🔴 CRÍTICO |
| **Seed per slot** | ✅ | 🔴 CRÍTICO |
| **LLM hooks** | ✅ | 🟡 IMPORTANTE |
| **Curator Vigilante** | ✅ | 🟡 IMPORTANTE |
| **Autopilot D+1** | ❌ | 🟢 DESEJÁVEL |
| **Incident learning** | ❌ | 🟢 DESEJÁVEL |
| **GitHub App auto-merge** | ❌ | 🟢 DESEJÁVEL |

---

## 2️⃣ IMPLEMENTAÇÃO: Fase por Fase

### 🔴 **FASE 1: Core Business Logic (3 dias)**

#### A) Carregar `business_logic.yaml`

- `BusinessLogic::load_from_file` (`vvtv-core/src/business_logic/mod.rs`) converte o YAML do cartão em tipos Rust com validação de bounds.
- O repositório inclui um cartão de exemplo em `configs/business_logic.yaml`; o caminho padrão é resolvido via `paths.business_logic` em `configs/vvtv.toml`.
- O CLI ganhou `vvtvctl business-logic show|validate|reload`, que expõe o cartão atual, valida o conteúdo e confirma recargas sem derrubar serviços.
- A estrutura mantém restrições operacionais (bias máximo, epsilon em [0,1]) e expõe temperatura/top-k/seed para o Planner.

---

#### B) Seleção determinística (Gumbel-Top-k + seed por slot)

- O módulo `vvtv-core/src/plan/selection/mod.rs` implementa `gumbel_topk_indices` e `generate_slot_seed_robust` (hash de data+slot+seed global) com `ChaCha20Rng`.
- `Planner::run_once` usa `BusinessLogic::selection_temperature` e `selection_top_k` para controlar o lote, aplica bias do YAML e registra `seed/indices/scores_norm` em `tracing`.
- Seeds mudam a cada janela de 15 minutos, garantindo reprodutibilidade e auditabilidade.

#### C) CLI & telemetria

- `vvtvctl business-logic show` imprime resumo (método, temperatura, top_k, bias) e `--format json` exporta para automações locais.
- `business-logic validate` falha com mensagem amigável se o YAML violar limites.
- `business-logic reload` confirma a recarga (pré-validação) sem acoplar ao ciclo do Planner.

---

#### C) Integração no Planner

- `Planner` agora exige `Arc<BusinessLogic>` e aplica `plan_selection_bias()` logo após o scoring.
- `Planner::apply_selection_strategy` consulta `selection_method()`; quando `GumbelTopK`, as pontuações são escaladas pela temperatura configurada, o seed é derivado por `generate_slot_seed_robust` e os índices são sorteados com `gumbel_topk_indices`.
- Logs estruturados (`target: "planner.selection"`) carregam `seed`, `temperature`, `top_k`, `indices` e `scores_norm` para auditoria.

```rust
let ordered = match method {
    SelectionMethod::GumbelTopK => {
        let temperature = self.business_logic.selection_temperature().max(1e-3);
        let scaled_scores: Vec<f64> = ordered.iter().map(|(_, score, _)| *score / temperature).collect();
        let seed = generate_slot_seed_robust(
            now,
            self.business_logic.slot_duration(),
            self.business_logic.window_id(),
            self.business_logic.global_seed(),
        );
        let mut rng = ChaCha20Rng::seed_from_u64(seed);
        let indices = gumbel_topk_indices(&scaled_scores, top_k, &mut rng);
        // ... mantém determinismo e diversidade
        indices
            .into_iter()
            .map(|index| ordered[index].clone())
            .collect::<Vec<_>>()
    }
    _ => {
        let mut copy = ordered.to_vec();
        copy.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        copy.truncate(top_k);
        copy
    }
};
```

- `SelectedCandidate::into_decision` anexa `llm_action{...}` e notas do Curator à rationale final, mantendo rastreabilidade completa.

### 🟡 **FASE 2: LLM Integration (4 dias)**

#### A) Orquestrador e Circuit Breaker

- `LlmHook` encapsula handler (`Arc<dyn LlmHookHandler>`), `allowed_actions`, `budget_tokens`, `deadline` e um `CircuitBreaker` dedicado.
- `CircuitBreaker` acompanha uma janela deslizante (`window_size = 50`, `failure_threshold = 0.10`) e alterna entre estados `Closed`, `HalfOpen` e `Open`.
- `HttpLlmHandler` envia `LlmHookRequest` via `reqwest` e devolve `LlmHookOutcome` (`action`, `mode`, `payload`).

```rust
let fut = self.handler.handle(request);
match timeout(self.deadline, fut).await {
    Ok(Ok(outcome)) => {
        self.breaker.record(now, true);
        outcome
    }
    Ok(Err(err)) => {
        warn!(target: "llm", hook = ?self.kind, "handler error: {err}");
        self.breaker.record(now, false);
        self.fallback("handler_error")
    }
    Err(_) => {
        warn!(target: "llm", hook = ?self.kind, "timeout after {:?}", self.deadline);
        self.breaker.record(now, false);
        self.fallback("timeout")
    }
}
```

#### B) Consumo no Planner

- `Planner::apply_llm` cria `Vec<LlmInvocation>` (plan_id, score, rationale, tags, kind) e delega para `LlmOrchestrator::rerank_candidates`.
- Quando `mode == Apply` e `order` é fornecida, os candidatos são reordenados deterministamente; em `AdviceOnly`, a ordem original é mantida.
- A anotação `llm_action{source:.. model:.. mode:.. reason:..}` e `llm_confidence` são adicionadas às rationales finais.

#### C) Observabilidade e Testes

- `tracing` (target `planner.llm`) registra `source`, `reason`, `mode` e evita silenciosamente quedas de SLA.
- Testes assíncronos (`circuit_breaker_short_circuits`, `orchestrator_rerank_parses_order`) cobrem timeout, fallback e parsing de ordem.
- Documentação operacional em `docs/LLM_HOOKS.md` descreve payload, limites de tokens e como plugar handlers HTTP reais.

### 🟠 **FASE 3: Curator Vigilante & Token Bucket (4 dias)**

- `CuratorVigilanteConfig::with_log_dir` define `confidence_threshold=0.62`, `max_reorder_distance=4`, `token_bucket_capacity=6`, `token_bucket_refill_per_hour=6` e `locked` opcional.
- `CuratorVigilante::review` avalia sinais e calcula confiança agregada:

```rust
let signals = self.evaluate_signals(now, candidates);
let triggered = signals.iter().filter(|signal| signal.triggered).count();
let confidence = if signals.is_empty() {
    0.0
} else {
    triggered as f64 / signals.len() as f64
};
```

- Quando `confidence ≥ threshold` e o `TokenBucket` libera crédito, a decisão vira `Apply` e a ordem retornada respeita `max_reorder_distance`; caso contrário retorna `Advice` (com notas como `token_bucket:exhausted` ou `curator_locked`).
- Logs diários em `logs/curator_vigilante/curator_vigilante_<data>.jsonl` capturam `evaluation`, `order`, `plans` e `llm_action` opcional.
- Testes incluídos: `token_bucket_refills` (garante refill horário) e `curator_detects_tag_duplication` (verifica sinal estético).

### 🔭 Próximos Passos (P4 em diante)

- **Autopilot D+1:** orquestrar ajustes seguros (canary 20%, rollback automático, commits versionados do cartão). *Status: a definir.*
- **Incident learning:** transformar incidentes repetidos em patches sugeridos para o cartão (integração com Dossiê + métricas P6).
- **GitHub App auto-merge:** pipeline que assina PRs aprovados pelo Autopilot e libera merge supervisionado.
- **Observability & Testing (P6/P7):** ampliar dashboards (planner/curator) e suíte de testes integrados cobrindo LLM/Curator end-to-end.

## 7️⃣ Fluxo Integrado (P1–P3 hoje)

1. `BusinessLogic::load_from_file` valida o cartão e injeta limites no `Planner`/CLI.
2. `Planner::run_once_async` pontua candidatos, aplica `plan_selection_bias`, consulta LLM (opcional) e executa Gumbel-Top-k determinístico.
3. Cada decisão selecionada recebe `llm_action{}` (quando aplicável) e segue para revisão do Curator.
4. `CuratorVigilante::review` registra sinais, aplica token bucket e reordena se necessário, com logs JSONL auditáveis.
5. `SqlitePlanStore::store_decisions` persiste o lote `selected`, permitindo que o Realizer continue o fluxo T-4h.

## 8️⃣ Checklist de Qualidade

- ✅ Loader YAML validado com testes (bounds, ranges, parsing determinístico).
- ✅ Seleção Gumbel-Top-k coberta por testes unitários (`deterministic_gumbel_topk`, `seed_changes_with_slot`).
- ✅ Circuit breaker e rerank testados com `tokio::test`.
- ✅ Curator: token bucket + sinal de tags testados.
- 🔜 Adicionar testes focados em `CuratorReview::order` e integração Planner ↔ Curator.
- 🔜 Criar cenários de erro para `HttpLlmHandler` via mocks HTTP.

## 9️⃣ Referências Rápidas

- `docs/BUSINESS_LOGIC_README.md` — schema completo e CLI.
- `docs/LLM_HOOKS.md` — hooks, SLA e exemplos de payload.
- `docs/CURATOR_VIGILANTE.md` — sinais, thresholds e formato de log.
- `EPIC-P.md` — roadmap atualizado com status dos PRs P1–P3.

