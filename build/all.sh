#!/usr/bin/env bash
set -euo pipefail

mode="${1:-debug}"
case "$mode" in
  debug)
    cargo build -p kernel
    ;;
  release)
    cargo build -p kernel --release
    ;;
  *) echo "Usage: $0 [debug|release]"; exit 1 ;;
esac
