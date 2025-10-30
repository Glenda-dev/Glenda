mod clint;
mod context;
mod handler;
mod plic;
pub mod timer;
pub use context::{TrapContext, TrapFrame};
pub use handler::vector;

use crate::dtb;
use crate::printk;
use riscv::register::{sie, sscratch, sstatus};

pub fn inittraps() {
    plic::init();
    timer::create();
    // 使能 UART 接收中断
    {
        let cfg = dtb::uart_config().unwrap_or(driver_uart::DEFAULT_QEMU_VIRT);
        let base = cfg.base();
        let lsr_off = cfg.lsr_offset();
        let stride = if lsr_off >= 5 { lsr_off / 5 } else { 1 };
        let ier = (base + stride * 1) as *mut u8;
        unsafe { core::ptr::write_volatile(ier, 0x01) };
    }
    printk!("TRAP: Initialized Global trap");
}

pub fn inittraps_hart(hartid: usize) {
    plic::init_hart(hartid);
    handler::vector::set();
    // 启用 S-mode 中断
    unsafe {
        sscratch::write(hartid);
        sstatus::set_sie();
        sie::set_sext();
        sie::set_ssoft();
        sie::set_stimer();
        crate::trap::timer::start(hartid);
    }
    printk!("TRAP: Initialized for hart {}", hartid);
}
