pub mod interrupt;
pub mod plic;
pub mod timer;
pub mod trap;
pub mod vector;
pub use trap::{TrapContext, TrapFrame};

use crate::printk;

pub fn init() {
    timer::create();
    printk!("irq: Initialized global IRQs\n");
}

pub fn init_hart(hartid: usize) {
    vector::init();
    // 启用 S-mode 中断
    interrupt::enable_s();
    printk!("irq: Initialized for hart {}\n", hartid);
}
