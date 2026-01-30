#![no_std]
#![no_main]
#![allow(dead_code)]

mod boot;
mod cap;
mod cpu;
mod hal;
mod init;
mod ipc;
mod irq;
mod logo;
mod mem;
mod printk;
mod proc;
mod trap;

use core::panic::PanicInfo;
use printk::{ANSI_BLUE, ANSI_RED, ANSI_RESET};

/*
 为了便捷，M-mode 固件与 M->S 的降权交给 OpenSBI，程序只负责 S-mode 下的内核
 (虽然大概率以后要从头写出来 M-mode 到 S-mode 的切换)

 寄存器约定[1]:
   - $a0 存放当前核的 hartid
   - $a1 存放设备树指针

 [1]: https://www.kernel.org/doc/Documentation/riscv/boot.rst

*/
#[unsafe(no_mangle)]
pub extern "C" fn glenda_main() -> ! {
    let (cpuid, info) = hal::boot::detect();
    init::init(cpuid, info);
    printk!("{}CPU {} entering scheduler{}\n", ANSI_BLUE, cpuid, ANSI_RESET);
    proc::scheduler::scheduler();
}

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    printk_unsynced!("{}PANIC{}: {}\n", ANSI_RED, ANSI_RESET, info);
    hal::runtime::backtrace();
    loop {
        hal::irq::wfi();
    }
}
