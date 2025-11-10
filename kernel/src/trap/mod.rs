mod clint;
mod context;
pub mod handler;
mod plic;
pub mod timer;
pub use context::{TrapContext, TrapFrame};
pub use handler::vector;
pub mod interrupt;

use crate::dtb;
use crate::printk;
use riscv::register::{sie, sscratch, sstatus};

pub fn inittraps() {
    plic::init();
    timer::create();
    // 使能 UART 接收中断
    driver_uart::irq::enable();
    printk!("TRAP: Initialized Global trap");
}

pub fn inittraps_hart(hartid: usize) {
    plic::init_hart(hartid);
    handler::vector::set();
    // 启用 S-mode 中断
    interrupt::enable_s();
    printk!("TRAP: Initialized for hart {}", hartid);
}
