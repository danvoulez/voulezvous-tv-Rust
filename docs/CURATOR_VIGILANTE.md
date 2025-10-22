# Curator Vigilante

Implementação de sinais estéticos + token bucket em `vvtv-core/src/curation/mod.rs`.

## Sinais calculados

| Sinal                    | Descrição                                                      | Threshold |
|--------------------------|----------------------------------------------------------------|-----------|
| `palette_similarity`     | Similaridade média (cosseno) das `desire_vector` adjacentes    | ≥ 0.92    |
| `tag_duplication`        | Jaccard médio de tags consecutivas                             | ≥ 0.70    |
| `duration_streak`        | Comprimento da sequência com durações quase iguais             | ≥ 4 itens |
| `bucket_imbalance`       | Desbalanceamento entre `Plan.kind` (max-min) / (max+min)        | ≥ 0.55    |
| `novelty_temporal_kld`   | KLD vs. distribuição alvo (0-12h / 12-24h / >24h)               | ≥ 0.25    |
| `cadence_variation`      | Desvio padrão de `engagement_score` (proxy de cortes/min)       | ≤ 0.05    |

## Decisão

- **Confidence** = sinais acionados / total.
- `confidence ≥ 0.62` + `TokenBucket::take()` → modo `Apply` (caso `locked=false`).
- Caso bucket vazio ou `locked=true`, o Curator retorna `Advice` (sem reorder) e registra motivo em `notes`.

## Token Bucket

- Capacidade padrão: 6 applies/hora.
- Refill contínuo (`refill_per_hour` configurável) com `Instant`.
- Protege contra excesso de reorder mantendo taxa alvo 5–15%.

## Logging

- Entradas em `logs/curator_vigilante/curator_vigilante_<data>.jsonl`:
  ```json
  {
    "timestamp": "2025-10-21T03:15:00Z",
    "evaluation": {
      "decision": "apply",
      "confidence": 0.67,
      "signals": [...],
      "llm_action": {"source": "rerank_candidates", ...},
      "notes": ["token_bucket:apply"]
    },
    "order": ["plan-42", "plan-17", ...],
    "plans": ["plan-42", "plan-17", ...]
  }
  ```

## Integração no Planner

- `Planner` recebe `Arc<CuratorVigilante>` via `.with_curator(...)`.
- A ordem final respeita `max_reorder_distance=4`; caso o Curator mova um item, as rationales recebem `curator=apply`.
- Para travar aplicações automáticas, inicialize `CuratorVigilanteConfig` com `locked=true` (pode ser derivado de `business_logic` em quem constrói o Planner).

## Testes

- `curation::tests::token_bucket_refills` garante refill horário.
- `curation::tests::curator_detects_tag_duplication` valida gatilho de duplicação de tags.
