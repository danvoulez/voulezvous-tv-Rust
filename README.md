# VoulezVous.TV — Epic D Completion

Este repositório contém a implementação Rust dos módulos centrais do VoulezVous.TV.
A conclusão dos PRs **D7 → D11** consolida descoberta autônoma, resiliência antibot,
QA contínuo e otimizações de performance/documentação para operação 24/7.

## Entregas PR D7–D11

### D7 — Discovery Loop completo
- `vvtv-core/src/browser/searcher.rs`: motor multi-engine com heurísticas de vídeo,
  filtros de domínio e coleta incremental via CDP.
- `vvtv-core/src/browser/discovery_loop.rs`: orquestrador com rate limiting,
  estatísticas (`DiscoveryStats`) e persistência em `SqlitePlanStore`.
- `vvtvctl discover`: comando CLI para executar buscas (modo seco ou persistente).

### D8 — Resiliência antibot & tratamento de erros
- `vvtv-core/src/browser/fingerprint.rs`: mascaramento Canvas/WebGL/Audio injetado
  antes de cada navegação.
- `vvtv-core/src/browser/retry.rs` + `ip_rotator.rs`: categorização de falhas,
  retries com backoff e registro de rotações de proxy.
- `configs/browser.toml`: seção `[fingerprint]`/`[proxy]` ajustável por ambiente.

### D9 — Ferramental de QA e observabilidade
- `vvtv-core/src/monitor.rs`: `MetricsStore` + `DashboardGenerator` para HTML local.
- `docs/qa/nightly-smoke.md`: roteiro headless/headed, captura de evidências e
ações corretivas.
- `vvtvctl qa smoke-test|report`: comandos para validar domínios e gerar dashboards.

### D10 — Otimizações de performance
- `vvtv-core/src/processor/mod.rs`: fallback automático para VideoToolbox quando
  hardware Apple Silicon é detectado ou `VVTV_FORCE_APPLE_SILICON` está setado.
- `vvtv-core/src/sqlite.rs` + `sql/*.sql`: conexões inicializadas em WAL com
  PRAGMAs (`cache_size`, `mmap_size`, `busy_timeout`).
- `scripts/optimize_databases.sh`: rotina de manutenção (`wal_checkpoint`,
  `PRAGMA optimize`, `VACUUM`, `ANALYZE`).
- `vvtvctl completions <shell>`: geração de autocompletar para operadores.

### D11 — Documentação operacional
- `AGENTS.md`, `Tasklist.md` e `VVTV INDUSTRIAL DOSSIER.md` atualizados com o
  roadmap D7–D11 já concluído.
- `CHANGELOG.md`: registro das entregas Discovery Loop, antibot, QA e performance.
- README expandido com Quick Start, QA, otimizações e links de referência.

## Quick Start — Discovery Loop

```bash
# Compilar ferramentas
cargo build --release

# Executar descoberta (dry-run) a partir do diretório do projeto
./target/release/vvtvctl discover \
  --query "creative commons documentary" \
  --max-plans 10 \
  --dry-run
```

### QA rápido

```bash
# Smoke test (headless) em um player específico
./target/release/vvtvctl qa smoke-test \
  --url "https://exemplo.com/player" \
  --mode headless

# Gerar dashboard HTML com histórico do metrics.sqlite
./target/release/vvtvctl qa report --output artifacts/qa/dashboard.html
```

### Autocompletar de shell

```bash
# Bash
./target/release/vvtvctl completions bash > /etc/bash_completion.d/vvtvctl

# Zsh
./target/release/vvtvctl completions zsh > "${ZDOTDIR:-$HOME}/.zfunc/_vvtvctl"
```

## Otimização de bancos SQLite

Os arquivos `plans.sqlite`, `queue.sqlite` e `metrics.sqlite` operam em WAL com PRAGMAs
customizados. Execute o script abaixo após longas jornadas ou antes de backups frios:

```bash
./scripts/optimize_databases.sh /vvtv/data
```

O script aplica `wal_checkpoint(TRUNCATE)`, `PRAGMA optimize`, `VACUUM` e `ANALYZE`,
registrando o modo atual e o tamanho das páginas.

## Referências

- `docs/qa/nightly-smoke.md`: playbook noturno de QA.
- `VVTV INDUSTRIAL DOSSIER.md`: especificação completa do sistema.
- `PR_D7_TO_D11_ROADMAP.md`: roteiro detalhado dos incrementos Epic D.
