#!/usr/bin/env bash
set -euo pipefail

usage() { echo "Usage: $0 [debug|release]  # default: debug"; }

mode="${1:-debug}"
case "$mode" in
  debug)   cargo build -p kernel || true ;;
  release) cargo build -p kernel --release || true ;;
  -h|--help) usage; exit 0 ;;
  *) echo "Unknown mode: $mode"; usage; exit 1 ;;
esac

profile_dir="$mode"
elf="target/riscv64gc-unknown-none-elf/${profile_dir}/kernel"

qemu-system-riscv64 -machine virt -m 128M -nographic -bios default -S -s -kernel "$elf" &
pid=$!
echo "QEMU PID=$pid"
echo "Run in another shell:"
echo "  gdb -ex 'set architecture riscv:rv64' -ex 'target remote :1234' -ex 'symbol-file ${elf}'"
wait $pid
