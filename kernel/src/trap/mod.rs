pub mod context;
pub mod info;
pub mod interrupt;
mod kernel;
pub mod timer;
mod user;
pub mod vector;

pub use context::{TrapContext, TrapFrame};

use crate::cap::CapType;
use crate::ipc;
use crate::printk;
use crate::proc;
use riscv::register::scause;

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
