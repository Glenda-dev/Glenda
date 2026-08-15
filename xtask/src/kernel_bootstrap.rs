//! Bootstrap kernel image source (toolchain module ownership).
//!
//! The toolchain module owns the reproducible cross-compilation scaffolding, so
//! this crate binary lives under `xtask/` rather than `kernel/src/`. Its two
//! jobs are:
//!   1. prove the offline clean rebuild reproduces an identical kernel ELF, and
//!   2. establish the QEMU virt runner contract by issuing the deterministic
//!      `GLENDA_BOOT_OK` banner exactly once on the primary serial console.
//!
//! This is a single-hart minimal stub, not the real kernel: it sets one early
//! stack, drives the ns16550 UART at the QEMU virt fixed address, prints the
//! banner, and parks. The real kernel (`kernel/src/main.rs`), its `printk`
//! layer, the device-tree UART discovery path, and the linker layout land with
//! the boot-asm milestone, which re-points `[[bin]].path` in `kernel/Cargo.toml`
//! to its own sources. Nothing here depends on boot-asm-owned files.
//!
//! The link base and early boot stack are controlled by the toolchain-owned
//! linker script ./kernel_bootstrap_linker.ld (stack `_boot_stack_top`).

#![no_std]
#![no_main]

use core::panic::PanicInfo;

/// QEMU virt primary UART (ns16550-compatible) base address, byte-addressed
/// MMIO. The first UART on the virt machine is fixed at 0x10000000.
const UART_BASE: usize = 0x1000_0000;
/// Transmit holding register offset.
const UART_THR: usize = 0;
/// Line status register offset.
const UART_LSR: usize = 5;
/// LSR bit: transmitter holding register empty.
const LSR_THRE: u8 = 1 << 5;

core::arch::global_asm!(
    r#"
    .section .text._start
    .globl _start
_start:
    la      sp, _boot_stack_top
    j       boot_body
    "#
);

/// Poll the UART transmitter until it accepts another byte.
fn uart_putc(byte: u8) {
    let lsr = (UART_BASE + UART_LSR) as *const u8;
    let thr = (UART_BASE + UART_THR) as *mut u8;
    // Busy-wait until the transmitter can accept a byte.
    while unsafe { lsr.read_volatile() } & LSR_THRE == 0 {}
    unsafe {
        thr.write_volatile(byte);
    }
}

/// Write a byte string to the primary serial console.
fn uart_puts(bytes: &[u8]) {
    for &b in bytes {
        uart_putc(b);
    }
}

/// The actual boot body, jumped to from the assembly prologue once the stack
/// pointer is live. Publishes the banner exactly once, then parks the boot
/// hart.
#[no_mangle]
pub extern "C" fn boot_body() -> ! {
    uart_puts(b"GLENDA_BOOT_OK\n");
    loop {}
}

/// Panic handler: emit a marker then park, matching the "fault rather than
/// loop silently" contract for early entry failures.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    uart_puts(b"BOOT_PANIC\n");
    loop {}
}