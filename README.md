# VoulezVous.TV — Epic D Completion

Este repositório contém a implementação Rust dos módulos centrais do VoulezVous.TV.
A conclusão do Epic D garante descoberta autônoma, resiliência antibot, QA contínuo e
pipeline de mídia otimizado (VideoToolbox/SQLite WAL).

## Quick Start — Discovery Loop

```bash
# Compilar ferramentas
cargo build --release

# Executar descoberta (dry-run) a partir do diretório do projeto
./target/release/vvtvctl discover \
  --query "creative commons documentary" \
  --max-plans 10 \
  --dry-run

# Gerar relatório QA após o loop
./target/release/vvtvctl qa smoke-test \
  --url "https://exemplo.com/player" \
  --mode headless
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
