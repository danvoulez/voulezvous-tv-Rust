# Changelog

Todas as mudanças notáveis deste repositório serão documentadas aqui.

## [Unreleased]
### Added
- Suporte condicional a VideoToolbox no pipeline de transcode (`Processor::transcode_media`).
- Comando `vvtvctl completions <shell>` para gerar scripts de autocompletar (bash/zsh).
- Script `scripts/optimize_databases.sh` aplicando `wal_checkpoint`, `PRAGMA optimize`, `VACUUM` e `ANALYZE` nos bancos operacionais.
- Atualizações de documentação: Discovery Loop implementado, guia QA noturno incorporado, README Quick Start e Tasklist Epic D concluído.

### Changed
- Conexões SQLite (`plans`, `queue`, `metrics`, telemetria`) agora inicializam com WAL + PRAGMAs (`cache_size`, `temp_store`, `mmap_size`, `busy_timeout`).
- Schemas `sql/*.sql` passam a declarar os PRAGMAs de otimização antes das migrações `BEGIN`.

### Benchmarks & QA
- **Transcode fallback**: comando VideoToolbox acionado automaticamente quando `use_hardware_accel = true` e hardware Apple Silicon detectado (testado via override `VVTV_FORCE_APPLE_SILICON`).
- **SQLite otimizado**: execução de `scripts/optimize_databases.sh target/tmpdb` reduziu `page_count` de 46 → 25 em `big.sqlite` e converteu `journal_mode` para `wal`, confirmando aplicação dos PRAGMAs (checkpoint+vacuum+analyze).
