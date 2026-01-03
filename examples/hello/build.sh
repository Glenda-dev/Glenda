#!/bin/sh
set -e

for arg in "$@"; do
  if [ "$arg" = "--release" ]; then
    MODE="release"
  fi
done

# Build the project
cargo build --target riscv64gc-unknown-none-elf $@

# Create build directory
mkdir -p build

# Copy ELF to build directory
cp ${CARGO_MANIFEST_DIR}/../target/riscv64gc-unknown-none-elf/$MODE/hello build/hello.elf
