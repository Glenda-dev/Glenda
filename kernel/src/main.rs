#![no_std]
#![no_main]

mod lock;
mod logo;
mod printk;
mod tests;

use core::panic::PanicInfo;
use logo::LOGO;
use printk::{ANSI_BLUE, ANSI_RED, ANSI_RESET};
use riscv::asm::wfi;

/*
 为了便捷，M-mode 固件与 M->S 的降权交给 OpenSBI，程序只负责 S-mode 下的内核
 (虽然大概率以后要从头写出来 M-mode 到 S-mode 的切换)

 寄存器约定[1]:
   - $a0 存放当前核的 hartid
   - $a1 存放设备树指针

 [1]: https://www.kernel.org/doc/Documentation/riscv/boot.rst

*/
#[unsafe(no_mangle)]
pub extern "C" fn glenda_main(hartid: usize, dtb: *const u8) -> ! {
    tests::init_harts(hartid, dtb);
    if hartid == 0 {
        printk!("{}", LOGO);
        printk!("{}Glenda microkernel booting{}", ANSI_BLUE, ANSI_RESET);
        printk!("Device tree blob at {:p}", dtb);
    }

    #[cfg(feature = "tests")]
    {
        tests::run_spinlock_tests(hartid, dtb);
    }

    loop {
        wfi();
    }
}

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    printk!("{}PANIC{}: {}", ANSI_RED, ANSI_RESET, info);
    loop {
        wfi();
    }
}
