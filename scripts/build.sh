#!/usr/bin/env bash
# Build CoPaw - console frontend + Rust binary
# Run from repo root: bash scripts/build.sh
set -e

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

echo "[build] Building console frontend..."
(cd "$REPO_ROOT/console" && npm ci)
(cd "$REPO_ROOT/console" && npm run build)

echo "[build] Building Rust binary..."
cargo build --release

echo "[build] Done. Binary location: $REPO_ROOT/target/release/copaw"
