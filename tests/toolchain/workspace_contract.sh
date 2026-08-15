#!/bin/sh
# Contract toolchain check: the offline-reproducibility structure the
# toolchain-clean-rebuild binding depends on is present and self-consistent.
set -u

case_id="${1:-toolchain-workspace-contract}"
root_dir="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root_dir" || exit 2

fail() {
  echo "CHECK_FAIL $case_id: $1" >&2
  exit 1
}

# Offline mode must be declared so clean builds never touch crates.io.
grep -q 'offline\s*=\s*true' .cargo/config.toml || fail ".cargo/config.toml does not enable offline mode"

# The vendored source registry replacement declares the offline source policy.
[ -f vendor/config.toml ] || fail "vendor/config.toml missing"
grep -q 'replace-with\s*=\s*"vendored-sources"' vendor/config.toml || fail "vendor registry replacement undeclared"

# The rv64 target and kernel linker are pinned by the toolchain file.
grep -q 'riscv64gc-unknown-none-elf' rust-toolchain.toml || fail "rust-toolchain.toml does not pin the riscv64 target"
grep -q 'rust-lld' .cargo/config.toml || fail "kernel linker not declared as rust-lld"

# Workspace members must include the kernel and the xtask driver.
grep -q 'kernel' Cargo.toml || fail "workspace Cargo.toml does not list the kernel member"
grep -q 'xtask' Cargo.toml || fail "workspace Cargo.toml does not list the xtask member"

# The kernel crate declares a binary named *kernel* and a build script.
grep -q 'name\s*=\s*"kernel"' kernel/Cargo.toml || fail "kernel/Cargo.toml missing the kernel binary"
[ -f kernel/build.rs ] || fail "kernel/build.rs missing"

echo "CHECK_OK $case_id"