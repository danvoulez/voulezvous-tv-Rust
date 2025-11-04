# Changelog

Todas as mudanças notáveis deste repositório serão documentadas aqui.

## [Unreleased]
### Added
- **PR H1–H5 — Distribuição & CDN**: módulo `distribution`, replicação origin→Railway com failover automático, integrações Cloudflare/B2, scripts CLI (`origin_replication.sh`, `switch_cdn.sh`, `init_edge_node.sh`) e métricas de latência/segurança em `metrics.sqlite`.
- **PR D7 — Discovery Loop completo**: `ContentSearcher`, `DiscoveryLoop` e comando
  `vvtvctl discover` com estatísticas e modo dry-run.
- **PR D8 — Resiliência antibot**: `FingerprintMasker`, retries categorizados,
  rotação de proxy/telemetria e novas chaves em `browser.toml`.
- **PR D9 — Ferramental de QA**: `MetricsStore`, `DashboardGenerator`,
  `vvtvctl qa smoke-test|report` e playbook `docs/qa/nightly-smoke.md`.
- **PR D10 — Otimizações de performance**: fallback VideoToolbox,
  conexões SQLite em WAL + script `scripts/optimize_databases.sh`,
  comando `vvtvctl completions <shell>`.
- **PR D11 — Documentação operacional**: README expandido, Tasklist/AGENTS/Dossiê
  atualizados e roadmap D7–D11 registrado.

### Changed
- Conexões SQLite (`plans`, `queue`, `metrics`, telemetria`) agora inicializam com
  WAL + PRAGMAs (`cache_size`, `temp_store`, `mmap_size`, `busy_timeout`).
- Schemas `sql/*.sql` passam a declarar os PRAGMAs de otimização antes das migrações
  `BEGIN`.

### Benchmarks & QA
- **Transcode fallback**: comando VideoToolbox acionado automaticamente quando
  `use_hardware_accel = true` e hardware Apple Silicon detectado (testado via
  override `VVTV_FORCE_APPLE_SILICON`).
- **SQLite otimizado**: execução de `scripts/optimize_databases.sh target/tmpdb`
  reduziu `page_count` de 46 → 25 em `big.sqlite` e converteu `journal_mode` para
  `wal`, confirmando aplicação dos PRAGMAs (checkpoint+vacuum+analyze).
