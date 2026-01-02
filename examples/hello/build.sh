#!/bin/sh
set -e

# Build the project
cargo build --target riscv64gc-unknown-none-elf

# Create build directory
mkdir -p build

# Convert ELF to binary
riscv64-unknown-elf-objcopy -O binary ../../target/riscv64gc-unknown-none-elf/debug/hello build/hello.bin
