#![no_std]
#![no_main]

mod drivers;
mod dtb;
mod hart;
mod init;
mod irq;
mod logo;
mod mem;
mod printk;
mod proc;
mod sbi;
mod syscall;

#[cfg(feature = "tests")]
mod tests;

use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};
use init::init;
use logo::LOGO;
use printk::{ANSI_BLUE, ANSI_RED, ANSI_RESET};
use riscv::asm::wfi;

include!("../../target/proc_payload.rs");

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
    init(hartid, dtb);
    #[cfg(feature = "tests")]
    {
        tests::test(hartid);
    }

    if hartid == 0 {
        if HAS_PROC_PAYLOAD && !PROC_PAYLOAD.is_empty() {
            printk!("Creating init process from payload...\n");
            proc::process::create(PROC_PAYLOAD);
        } else {
            printk!("Creating init process from fallback...\n");
            // wfi()
            proc::process::create(&[0x6f, 0x00, 0x00, 0x00]);
        }
        printk!("Starting scheduler on hart 0...\n");
        proc::scheduler::scheduler();
    }

    printk!("{}Hart {} entering main loop{}\n", ANSI_BLUE, hartid, ANSI_RESET);
    loop {
        wfi();
    }
}

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    printk!("{}PANIC{}: {}\n", ANSI_RED, ANSI_RESET, info);
    loop {
        wfi();
    }
}
