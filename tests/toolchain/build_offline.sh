#!/bin/sh
# Public toolchain check: a real cargo xtask build must succeed in offline mode
# and publish the runnable kernel ELF at the documented artifact path.
set -u

case_id="${1:-toolchain-offline-build-public}"
root_dir="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root_dir" || exit 2

fail() {
  echo "CHECK_FAIL $case_id: $1" >&2
  exit 1
}

kernel_elf="target/riscv64gc-unknown-none-elf/debug/kernel"

# Force offline resolution exactly as the runner does (env PATH provided by VOS).
if ! CARGO_NET_OFFLINE=true cargo xtask build >/dev/null 2>&1; then
  fail "cargo xtask build failed in offline mode"
fi

[ -f "$kernel_elf" ] || fail "kernel ELF missing at $kernel_elf"
[ -s "$kernel_elf" ] || fail "kernel ELF empty"

echo "CHECK_OK $case_id"