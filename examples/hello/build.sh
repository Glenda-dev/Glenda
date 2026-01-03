#!/bin/sh
set -e

MODE="debug"
CARGO_FLAGS=""

for arg in "$@"; do
  if [ "$arg" = "--release" ]; then
    MODE="release"
    CARGO_FLAGS="--release"
  fi
done

# Build the project
cargo build --target riscv64gc-unknown-none-elf $CARGO_FLAGS

# Create build directory
mkdir -p build

# Copy ELF to build directory
cp ${CARGO_MANIFEST_DIR}/../target/riscv64gc-unknown-none-elf/$MODE/hello build/hello.elf
