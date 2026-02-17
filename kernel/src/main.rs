#![no_std]
#![no_main]
#![allow(dead_code)]

#[macro_use]
mod printk;
mod boot;
mod cap;
mod cpu;
#[cfg(feature = "gdb")]
mod debug;
mod drivers;
mod error;
mod hal;
mod init;
mod ipc;
mod irq;
mod logo;
mod mem;
mod platform;
mod proc;
#[cfg(feature = "shell")]
mod shell;
mod sync;
mod trap;
mod version;

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
pub fn glenda_main(is_primary: bool) -> ! {
    init::init(is_primary);

    let cpuid = hal::cpu::cpu_id();
    printk!("{}CPU {} entering scheduler{}\n", ANSI_BLUE, cpuid, ANSI_RESET);
    if is_primary {
        print_banner();
        let bootargs = crate::boot::get_cmdline().unwrap_or("");
        printk!("bootargs: {}\n", bootargs);
        if !bootargs.contains("-v") {
            printk::set_verbose(true);
        }
        if bootargs.contains("-s") {
            run_shell();
        } else {
            proc::roottask::spawn_first();
        }
    }
    proc::scheduler::scheduler();
}

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    printk_unsynced!("{}PANIC{}(cpu {}): {}\n", ANSI_RED, ANSI_RESET, hal::cpu::cpu_id(), info);
    hal::runtime::backtrace();
    unsafe {
        hal::mem::deactivate_vspace();
    }
    run_shell();
    hal::platform::shutdown()
}

fn run_shell() {
    #[cfg(feature = "shell")]
    shell::run();
}

fn print_banner() {
    printk!("{}", printk::ANSI_CLEAR);
    printk!("{}", logo::LOGO);
    version::print();
}

#[unsafe(no_mangle)]
pub extern "C" fn glenda_boot() -> ! {
    crate::glenda_main(true);
}

#[unsafe(no_mangle)]
pub extern "C" fn glenda_secondary(hartid: usize) -> ! {
    hal::cpu::set_cpuid(hartid);
    crate::glenda_main(false);
}
