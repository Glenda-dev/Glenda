#!/bin/sh
# Bounded trace/oracle check for the toolchain build: run a fixed workload of
# build -> clean -> rebuild and assert the oracle of byte-identical images.
set -u

case_id="${1:-toolchain-build-trace}"
root_dir="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root_dir" || exit 2

fail() {
  echo "CHECK_FAIL $case_id: $1" >&2
  exit 1
}

kernel_elf="target/riscv64gc-unknown-none-elf/debug/kernel"
trace="${root_dir}/target/toolchain-build-trace.txt"
mkdir -p "${root_dir}/target"

# ---- workload: a deterministic sequence of build invocations ----
CARGO_NET_OFFLINE=true cargo xtask build >/dev/null 2>&1 || fail "workload: initial build failed"
h1="$(sha256sum "$kernel_elf" | cut -d' ' -f1)"
CARGO_NET_OFFLINE=true cargo clean -p kernel >/dev/null 2>&1 || fail "workload: clean failed"
CARGO_NET_OFFLINE=true cargo xtask build >/dev/null 2>&1 || fail "workload: rebuild failed"
h2="$(sha256sum "$kernel_elf" | cut -d' ' -f1)"

# ---- oracle: rebuild reproduces the identical image and the artifact exists ----
[ -s "$kernel_elf" ] || fail "oracle: kernel ELF absent after rebuild"
[ "$h1" = "$h2" ] || fail "oracle: non-deterministic image ($h1 != $h2)"

echo "trace build -> clean -> rebuild ok (h=$h1)" > "$trace"
echo "CHECK_OK $case_id"