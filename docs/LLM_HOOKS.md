# LLM Hooks & Circuit Breakers

O módulo `vvtv-core/src/llm/mod.rs` fornece a integração entre o Planner e os hooks LLM descritos no EPIC P.

## Componentes

- **`LlmOrchestrator`**: repositório de hooks (`expand_queries`, `site_tactics`, `rerank_candidates`, `recovery_plan`, `enrich_metadata`). Cada hook mantém `deadline`, `allowed_actions`, `budget_tokens` e um `CircuitBreaker`.
- **`LlmHookHandler`**: trait assíncrono implementável (ex.: `HttpLlmHandler` para chamadas HTTP ou mocks de teste).
- **`CircuitBreaker`**: janela de 50 chamadas, falha >10% → estado *open* por 5 minutos, retornando `AdviceOnly` em <10 ms.
- **`LlmInvocation`**: payload serializado (plan_id, score, rationale, tags, kind) enviado para o hook de rerank.
- **`LlmInvocationResult`**: devolve `order` opcional, `LlmAction` (source/model/reason/confidence) e `mode` (`AdviceOnly` ou `Apply`).

## Fluxo

1. `Planner` gera `Vec<LlmInvocation>` a partir dos candidatos pontuados.
2. `LlmOrchestrator::rerank_candidates` aplica timeout (`tokio::time::timeout`), circuit breaker e fallback rápido (`llm_action={source:..., model:fallback, reason:timeout}`) em caso de erro.
3. Quando `mode=Apply` e `order` é retornada, o Planner reordena os candidatos antes do Gumbel-Top-k.
4. Cada `PlanSelectionDecision` recebe anotação `llm_action{...}` + `llm_confidence` na rationale.

## Testes

- `#[tokio::test]` valida curto-circuito (circuit breaker abre após falhas) e parsing do `order` retornado.
- Mocks estáticos (`StaticHandler`) simulam respostas determinísticas.

## Configuração futura

- Handlers HTTP podem ser configurados com endpoint local (LLM on-premise).
- `BusinessLogic` pode controlar `allowed_actions` e `budget_tokens` por hook se necessário.
