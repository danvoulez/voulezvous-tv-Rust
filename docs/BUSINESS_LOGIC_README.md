# Business Logic Card (YAML)

Este documento resume o formato e o fluxo operacional do cartão do dono (`business_logic.yaml`).

## Estrutura

```yaml
policy_version: "2025.10"
env: "production"
knobs:
  boost_bucket: "music"
  music_mood_focus: ["focus", "midnight"]
  interstitials_ratio: 0.08
  plan_selection_bias: 0.0
scheduling:
  slot_duration_minutes: 15
  global_seed: 4242
  lock_curator_applies: false
selection:
  method: gumbel_top_k
  temperature: 0.85
  top_k: 12
  seed_strategy: slot_hash
exploration:
  epsilon: 0.12
autopilot:
  enabled: false
kpis:
  primary: ["selection_entropy"]
  secondary: ["curator_apply_budget_used_pct"]
```

### Campos principais

- **knobs.plan_selection_bias**: deslocamento aplicado ao score antes da seleção (range [-0.20, 0.20]).
- **scheduling.slot_duration_minutes**: janela usada para o seed determinístico (`generate_slot_seed_robust`).
- **selection.temperature/top_k**: controle fino da seleção Gumbel-Top-k.
- **selection.seed_strategy**: atualmente suportado `slot_hash` (combina data + slot + seed global).
- **scheduling.lock_curator_applies**: quando `true`, o Curator atua somente em modo *Advice*.

## CLI

```bash
# Mostrar cartão carregado
vvtvctl business-logic show --config configs/vvtv.toml

# Validar um cartão alternativo
vvtvctl business-logic validate --path /tmp/novo_cartao.yaml

# Recarregar (valida + confirma path)
vvtvctl business-logic reload
```

Saída em JSON (`--format json`) é apropriada para automações locais.

## Integração no Planner

- `Planner` recebe `Arc<BusinessLogic>` na construção.
- `selection_temperature` e `selection_top_k` alimentam o módulo `plan/selection`.
- Bias (`plan_selection_bias`) é aplicado antes da seleção.
- Seeds gerados via `generate_slot_seed_robust(now, slot_duration, window_id, global_seed)` são logados junto com os índices.

## Arquivos relevantes

- `configs/business_logic.yaml` – cartão exemplo.
- `vvtv-core/src/business_logic/mod.rs` – tipos e validação.
- `vvtvctl/src/lib.rs` – comandos `business-logic` no CLI.
- `vvtv-core/src/plan/selection/mod.rs` – Gumbel-Top-k e seed robusto.
