/*
 The main file of Glenda
 */

#![no_std]
#![no_main]

mod uart;

use core::panic::PanicInfo;
use riscv::asm::wfi;
use crate::uart::{ANSI_BLUE, ANSI_RED, ANSI_RESET};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}PANIC{}: {}", ANSI_RED, ANSI_RESET, info);
    loop { wfi(); }
}

/*
  为了便捷，M-mode 固件与 M->S 的降权交给 OpenSBI，程序只负责 S-mode 下的内核
  (虽然大概率以后要从头写出来 M-mode 到 S-mode 的切换)

  寄存器约定[1]:
    - $a0 存放当前核的 hartid
    - $a1 存放设备树指针

  [1]: https://www.kernel.org/doc/Documentation/riscv/boot.rst
 */
#[unsafe(no_mangle)]
pub extern "C" fn glenda_main(hartid: usize, _dtb: *const u8) -> ! {
    println!("{}Glenda from Outer Space (hart={}){}", ANSI_BLUE, hartid, ANSI_RESET);
    println!("println tests starts here:");
    println!("  int zero = {}", 0i32);
    println!("  int pos = {}", 42i32);
    println!("  int neg = {}", -42i32);
    println!("  int i32_min = {}", core::i32::MIN);
    println!("  int i32_max = {}", core::i32::MAX);
    println!("  u64 hex (no prefix) = {:x}", 0xdead_beefu64);
    println!("  u64 hex (with 0x) = 0x{:x}", 0xdead_beefu64);
    println!("  u64 dec = {}", 12345678901234u64);
    let p1: *const u8 = 0x0 as *const u8;
    let p2: *const u8 = 0x1234 as *const u8;
    println!("  ptr null = {:p}", p1);
    println!("  ptr some = {:p}", p2);
    println!("  char = {}", 'A');
    println!("  string = {}", "Hello from Rust!");
    println!("  string (empty) = {}", "");
    println!("  mix: hart={} ptr={:p} hex=0x{:X} msg={}", hartid, p2, 0xCAFEBABEu64, "ok");
    println!("  u64 zero = {}", 0u64);
    println!("  u64 max = {}", core::u64::MAX);
    loop { wfi(); }
}
