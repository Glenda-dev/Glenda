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
    sed -i 's/\r$//' "$log" 2>/dev/null || true
    [ -s "$log" ] || fail "no serial output captured"
    count="$(grep -c '^GLENDA_BOOT_OK$' "$log" || true)"
    [ -n "$count" ] || count=0
    [ "$count" -eq 1 ] || fail "expected exactly one GLENDA_BOOT_OK line, found $count"
    echo "CHECK_OK boot-banner-public"
    ;;
  boot-console-binding)
    [ -f kernel/src/printk.rs ] || fail "kernel/src/printk.rs missing"
    grep -Eq 'driver_uart|uart::' kernel/src/printk.rs kernel/src/main.rs 2>/dev/null \
      || fail "printk is not routed through the uart driver"
    grep -REq 'GLENDA_BOOT_OK' kernel/src/main.rs kernel/src/logo.rs 2>/dev/null \
      || fail "boot banner literal not referenced by boot path"
    echo "CHECK_OK boot-console-binding"
    ;;
  uart-output-public)
    mkdir -p target
    log="target/serial-uart.log"
    timeout 20 cargo xtask run --mem 128M >"$log" 2>&1 || true
    sed -i 's/\r$//' "$log" 2>/dev/null || true
    [ -s "$log" ] || fail "no serial output captured"
    grep -q 'UART in use: base=0x' "$log" || fail "uart discovery line missing from console"
    grep -q 'microkernel booting' "$log" || fail "banner bytes never reached the uart console"
    echo "CHECK_OK uart-output-public"
    ;;
  dtb-memory-discovery-public)
    mkdir -p target
    log="target/serial-dtb.log"
    timeout 20 cargo xtask run --mem 128M >"$log" 2>&1 || true
    sed -i 's/\r$//' "$log" 2>/dev/null || true
    [ -s "$log" ] || fail "no serial output captured"
    grep -q 'Device tree blob at 0x' "$log" || fail "dtb discovery banner missing"
    grep -Eq '^[0-9]+ harts detected' "$log" || fail "hart count discovery missing"
    echo "CHECK_OK dtb-memory-discovery-public"
    ;;
  mem-sv39-layout-binding)
    [ -f kernel/src/mem/pte.rs ] || fail "kernel/src/mem/pte.rs missing"
    grep -Eq 'PTE_V' kernel/src/mem/pte.rs || fail "Sv39 PTE flag constants missing"
    grep -Eq 'SATP_SV39|8 << 60' kernel/src/mem/vm.rs || fail "Sv39 satp mode constant missing"
    [ -f kernel/src/init/pmem.rs ] || fail "kernel/src/init/pmem.rs missing"
    echo "CHECK_OK mem-sv39-layout-binding"
    ;;
  boot-sbi-services-binding)
    grep -Eq 'sbi_set_timer' kernel/src/sbi.rs || fail "sbi_set_timer service missing"
    grep -Eq 'sbi::set_timer|crate::sbi::set_timer' kernel/src/trap/timer.rs \
      || fail "timer programming does not route through sbi_set_timer"
    echo "CHECK_OK boot-sbi-services-binding"
    ;;
  proc-lifecycle-public)
    mkdir -p target
    log="target/serial-proc.log"
    timeout 45 cargo xtask run --mem 128M >"$log" 2>&1 || true
    sed -i 's/\r$//' "$log" 2>/dev/null || true
    [ -s "$log" ] || fail "no serial output captured"
    grep -q 'Creating init process from payload' "$log" || fail "init process was not created from the embedded payload"
    grep -q 'Starting scheduler on hart 0' "$log" || fail "scheduler never started on the boot hart"
    grep -q '\[PASS\] Memory fork test done' "$log" || fail "fork/wait/exit lifecycle evidence missing"
    echo "CHECK_OK proc-lifecycle-public"
    ;;
  syscall-dispatch-public)
    mkdir -p target
    log="target/serial-syscall.log"
    timeout 45 cargo xtask run --mem 128M >"$log" 2>&1 || true
    sed -i 's/\r$//' "$log" 2>/dev/null || true
    [ -s "$log" ] || fail "no serial output captured"
    grep -q '\[PASS\] brk test passed' "$log" || fail "brk syscall evidence missing"
    grep -q '\[PASS\] mmap/munmap tests done' "$log" || fail "mmap/munmap syscall dispatch evidence missing"
    echo "CHECK_OK syscall-dispatch-public"
    ;;
  fs-mount-public)
    mkdir -p target
    cargo xtask mkfs >/dev/null 2>&1 || fail "cargo xtask mkfs failed"
    [ -f disk.img ] || fail "disk.img missing after mkfs"
    magic="$(od -A n -t x1 -N 4 disk.img | tr -d ' \n')"
    [ "$magic" = "40302010" ] || fail "disk.img superblock magic mismatch ($magic)"
    log="target/serial-fs.log"
    timeout 40 cargo xtask run --mem 128M >"$log" 2>&1 || true
    sed -i 's/\r$//' "$log" 2>/dev/null || true
    [ -s "$log" ] || fail "no serial output captured"
    grep -q 'FS: Superblock read:' "$log" || fail "fs superblock mount evidence missing"
    grep -q 'FS: All self-tests passed!' "$log" || fail "fs self-test evidence missing"
    echo "CHECK_OK fs-mount-public"
    ;;
  virtio-negotiated-queue-public)
    mkdir -p target
    log="target/serial-virtio.log"
    timeout 40 cargo xtask run --mem 128M >"$log" 2>&1 || true
    sed -i 's/\r$//' "$log" 2>/dev/null || true
    [ -s "$log" ] || fail "no serial output captured"
    grep -q 'VirtIO: Disk initialized' "$log" || fail "virtio-blk device never reached initialized state"
    echo "CHECK_OK virtio-negotiated-queue-public"
    ;;
  userland-exec-public)
    mkdir -p target
    log="target/serial-userland.log"
    timeout 45 cargo xtask run --mem 128M >"$log" 2>&1 || true
    sed -i 's/\r$//' "$log" 2>/dev/null || true
    [ -s "$log" ] || fail "no serial output captured"
    grep -q 'Creating init process from payload' "$log" || fail "init process was not created from the embedded payload"
    grep -q 'Starting scheduler on hart 0' "$log" || fail "scheduler never started on the boot hart"
    grep -q 'LAB-9 tests completed' "$log" || fail "userland LAB-9 self-test completion evidence missing"
    echo "CHECK_OK userland-exec-public"
    ;;
  userland-evidence-public)
    cargo xtask build >/dev/null 2>&1 || fail "cargo xtask build failed"
    [ -s target/service/hello/hello.elf ] || fail "hello.elf missing or empty"
    [ -s target/service/hello/hello.bin ] || fail "hello.bin missing or empty"
    [ -f target/proc_payload.rs ] || fail "generated target/proc_payload.rs missing"
    echo "CHECK_OK userland-evidence-public"
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
