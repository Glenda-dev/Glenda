mod clint;
mod interrupt;
mod plic;
pub mod timer;
pub mod trap;
pub mod vector;
pub use trap::{TrapContext, TrapFrame};

use crate::printk;

pub fn init() {
    plic::init();
    timer::create();
    // 使能 UART 接收中断
    drivers::uart::irq::enable();
    printk!("IRQ: Initialized global IRQs");
}

pub fn init_hart(hartid: usize) {
    plic::init_hart(hartid);
    vector::init();
    // 启用 S-mode 中断
    interrupt::enable_s();
    printk!("IRQ: Initialized for hart {}", hartid);
}
