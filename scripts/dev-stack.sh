#!/usr/bin/env bash
# Dev stack helper: build, optionally run host+hub on loopback, smoke-test.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

cmd="${1:-help}"

build() {
  cargo build --workspace --release
}

smoke() {
  echo "stack smoke: build + unit/integration tests (no long-running daemons)"
  cargo test --workspace
}

case "$cmd" in
  build) build ;;
  smoke) smoke ;;
  help|*)
    echo "usage: $0 {build|smoke|help}"
    exit 0
    ;;
esac
