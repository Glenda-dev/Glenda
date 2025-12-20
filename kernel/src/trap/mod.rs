pub mod context;
pub mod info;
pub mod interrupt;
pub mod kernel;
pub mod syscall;
pub mod timer;
pub mod user;
pub mod vector;

pub use context::{TrapContext, TrapFrame};

use crate::printk;

pub fn init() {
    // 初始化定时器
    timer::create();
    printk!("trap: Initialized global traps\n");
}

pub fn init_hart(hartid: usize) {
    vector::init();
    // 启用 S-mode 中断
    interrupt::enable_s();
    printk!("trap: Initialized for hart {}\n", hartid);
}
