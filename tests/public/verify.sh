#!/bin/sh
# Deterministic public check dispatcher for the Glenda student project.
set -u

case_id="${1:-}"
if [ -z "$case_id" ]; then
  echo "usage: verify.sh <case-id>" >&2
  exit 2
fi

root_dir="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root_dir" || exit 2

fail() {
  echo "CHECK_FAIL $case_id: $1" >&2
  exit 1
}

kernel_elf="target/riscv64gc-unknown-none-elf/debug/kernel"

case "$case_id" in
  boot-banner-public)
    mkdir -p target
    log="target/serial-boot.log"
    timeout 90 cargo xtask run --mem 128M >"$log" 2>&1 || true
    [ -s "$log" ] || fail "no serial output captured"
    count="$(grep -c '^GLENDA_BOOT_OK$' "$log" || true)"
    [ -n "$count" ] || count=0
    [ "$count" -eq 1 ] || fail "expected exactly one GLENDA_BOOT_OK line, found $count"
    echo "CHECK_OK boot-banner-public"
    ;;
  boot-sbi-console-binding)
    [ -f kernel/src/console.c ] || fail "kernel/src/console.c missing"
    grep -Eq 'sbi_ecall|SBI_EXT_CONSOLE' kernel/src/console.c \
      || fail "console output is not routed through the SBI ecall interface"
    grep -Eq 'GLENDA_BOOT_OK' kernel/src/main.c kernel/src/console.c 2>/dev/null \
      || fail "boot banner literal not referenced by boot path"
    echo "CHECK_OK boot-sbi-console-binding"
    ;;
  toolchain-clean-rebuild)
    cargo xtask build >/dev/null 2>&1 || fail "initial cargo xtask build failed"
    [ -f "$kernel_elf" ] || fail "kernel ELF missing after build"
    hash1="$(sha256sum "$kernel_elf" | cut -d' ' -f1)"
    cargo clean -p kernel >/dev/null 2>&1 || fail "cargo clean failed"
    cargo xtask build >/dev/null 2>&1 || fail "rebuild after clean failed"
    hash2="$(sha256sum "$kernel_elf" | cut -d' ' -f1)"
    [ "$hash1" = "$hash2" ] || fail "clean rebuild produced a different kernel image ($hash1 != $hash2)"
    echo "CHECK_OK toolchain-clean-rebuild"
    ;;
  *)
    echo "unknown case id: $case_id" >&2
    exit 2
    ;;
esac
