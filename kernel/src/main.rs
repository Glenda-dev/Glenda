#![no_std]
#![no_main]
#![allow(dead_code)]

extern crate alloc;

mod cap;
mod dtb;
mod hart;
mod init;
mod ipc;
mod irq;
mod logo;
mod mem;
mod printk;
mod proc;
mod sbi;
mod syscall;
mod trap;

use core::panic::PanicInfo;
use init::init;
use mem::alloc::Allocator;
use printk::{ANSI_BLUE, ANSI_RED, ANSI_RESET};
use riscv::asm::wfi;

#[global_allocator]
static GLOBAL_ALLOCATOR: Allocator = Allocator::new();

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
    unimplemented!();
    if hartid == 0 {
        printk!("{}", logo::LOGO);
        printk!("Starting scheduler on hart 0...\n");
        proc::scheduler::scheduler();
    }

    printk!("{}Hart {} entering main loop{}\n", ANSI_BLUE, hartid, ANSI_RESET);
    loop {
        wfi();
    }
}

#[inline(always)]
fn fp() -> usize {
    let ptr: usize;
    unsafe {
        core::arch::asm!("mv {}, s0", out(reg) ptr);
    }
    ptr
}

fn backtrace() {
    printk!("\n--- GLENDA BACKTRACE START ---\n");
    let mut current_fp = fp();
    let mut depth = 0;
    while current_fp != 0 && depth < 20 {
        // 0(fp) -> saved fp
        // 8(fp) -> saved ra
        unsafe {
            let ra_ptr = (current_fp as *const usize).sub(1);
            let prev_fp_ptr = (current_fp as *const usize).sub(2);

            // TODO: embed more info
            if ra_ptr as usize >= 0x80000000 && prev_fp_ptr as usize >= 0x80000000 {
                let ra = *ra_ptr;
                let prev_fp = *prev_fp_ptr;
                printk!("{:>2}: fp={:#x} ra={:#x}\n", depth, current_fp, ra);
                current_fp = prev_fp;
            } else {
                printk!("Invalid fp/ra ptr at {:#x}\n", current_fp);
                break;
            }
        }
        depth += 1;
    }
    printk!("--- GLENDA BACKTRACE END ---\n");
}

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    printk!("{}PANIC{}: {}", ANSI_RED, ANSI_RESET, info);
    backtrace();
    loop {
        wfi();
    }
}

#[macro_export]
macro_rules! unimplemented {
    () => {
        panic!("{}UNIMPLEMENTED{} at {}:{}", ANSI_RED, ANSI_RESET, core::file!(), core::line!());
    };
    ($msg:expr) => {
        panic!(
            "{}UNIMPLEMENTED{} at {}:{}: {}",
            ANSI_RED,
            ANSI_RESET,
            core::file!(),
            core::line!(),
            $msg
        );
    };
}
