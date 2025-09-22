#![no_std]
#![no_main]

use core::panic::PanicInfo;
use riscv::asm::wfi;
use driver_uart::{println, ANSI_BLUE, ANSI_RED, ANSI_RESET};

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
#[no_mangle]
pub extern "C" fn glenda_main(hartid: usize, _dtb: *const u8) -> ! {
    println!("{}Glenda microkernel booting (hart={}){}",
             ANSI_BLUE, hartid, ANSI_RESET);
    println!("println self-test:");
    println!("  int = {}", -42i32);
    println!("  hex = 0x{:x}", 0xDEAD_BEEFu64);

    loop { wfi(); }
}
