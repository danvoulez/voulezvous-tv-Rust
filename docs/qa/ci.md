# Local CI & Coverage Guide

Pipeline local para garantir os critérios do Epic L2.

## Requisitos

- Rust stable ≥1.74
- `cargo-tarpaulin` instalado (`cargo install cargo-tarpaulin`)
- `sqlite3`, `ffmpeg` e dependências presentes (ver `docs/deployment.md`)

## Workflow GitHub Actions

Arquivo: `.github/workflows/ci.yml`

Etapas:

1. `cargo fmt -- --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --workspace`
4. `cargo tarpaulin --workspace --out Xml`

## Execução Local

```bash
./scripts/system/local_ci.sh     # wrapper opcional
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo tarpaulin --workspace --out Html
```

> O `local_ci.sh` garante que os relatórios sejam armazenados em
> `target/ci-artifacts/`.

## Metas

- Cobertura mínima: 70% (ver saída do tarpaulin).
- Zero warnings no `clippy`.
- Formatação `rustfmt` consistente.

## Referências

- `VVTV INDUSTRIAL DOSSIER.md`, Bloco V (linhas 1500–1650)
- `.github/workflows/ci.yml`
