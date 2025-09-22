#!/usr/bin/env bash
set -euo pipefail
mode="${1:-debug}"
elf="target/riscv64gc-unknown-none-elf/${mode}/kernel"
riscv64-unknown-elf-objdump -d --source --all-headers "$elf" | less
