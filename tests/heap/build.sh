#!/bin/sh
set -e

MODE="debug"

# Build the project
cargo build --target riscv64gc-unknown-none-elf $@

# Create build directory
mkdir -p build

# Copy ELF to build directory
cp ${CARGO_MANIFEST_DIR}/../target/riscv64gc-unknown-none-elf/$MODE/heap_test build/heap.elf
