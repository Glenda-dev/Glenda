#!/bin/sh
# Fixed-seed bounded fuzz check for the toolchain reproducibility invariant:
# however many clean+rebuild cycles a seeded pseudo-random generator selects
# (bounded below by 2, above by 6), the rebuilt kernel image is byte-identical.
set -u

case_id="${1:-toolchain-repro-fuzz}"
seed="${2:-7}"
cases="${3:-3}"
root_dir="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root_dir" || exit 2

fail() {
  echo "CHECK_FAIL $case_id: $1" >&2
  exit 1
}

kernel_elf="target/riscv64gc-unknown-none-elf/debug/kernel"

# Final-iteration count derived from the fixed seed; bounded to [2,6].
niter=$(( (seed % 5) + 2 ))
[ "$niter" -ge 2 ] || niter=2
[ "$niter" -le 6 ] || niter=6
if [ "$cases" -lt "$niter" ]; then niter=$cases; fi

# First build establishes the reference image.
if ! CARGO_NET_OFFLINE=true cargo xtask build >/dev/null 2>&1; then
  fail "initial offline build failed"
fi
[ -f "$kernel_elf" ] || fail "kernel ELF missing"
ref="$(sha256sum "$kernel_elf" | cut -d' ' -f1)"

# Every seeded clean+rebuild round must reproduce the identical image.
i=0
while [ "$i" -lt "$niter" ]; do
  if ! CARGO_NET_OFFLINE=true cargo clean -p kernel >/dev/null 2>&1; then
    fail "cargo clean -p kernel failed (iter $i)"
  fi
  if ! CARGO_NET_OFFLINE=true cargo xtask build >/dev/null 2>&1; then
    fail "offline rebuild failed (iter $i)"
  fi
  got="$(sha256sum "$kernel_elf" | cut -d' ' -f1)"
  [ "$got" = "$ref" ] || fail "iteration $i hash mismatch ($got != $ref)"
  i=$((i + 1))
done

echo "CHECK_OK $case_id (seed=$seed, iterations=$niter)"