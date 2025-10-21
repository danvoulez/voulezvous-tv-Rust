#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ARTIFACT_DIR="$ROOT_DIR/target/ci-artifacts"
mkdir -p "$ARTIFACT_DIR"

run() {
  echo "[CI] $*"
  "$@"
}

run cargo fmt
run cargo clippy --all-targets -- -D warnings
run cargo test --workspace --all-features
run cargo tarpaulin --workspace --out Html --output-dir "$ARTIFACT_DIR"

echo "Relatórios disponíveis em $ARTIFACT_DIR"
