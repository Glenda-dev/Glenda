use crate::printk;
use riscv::register::sstatus;

mod clint;
mod context;
mod handler;
mod plic;
pub mod timer;
mod vector;

pub fn inittraps() {
    plic::init();
    timer::create();
    printk!("TRAP: Initialized Global trap");
}

pub fn inittraps_hart(hartid: usize) {
    plic::init_hart(hartid);
    vector::set_vector();
    // 启用 S-mode 中断
    unsafe {
        sstatus::set_sie();
    }
    printk!("TRAP: Initialized for hart {}", hartid);
}
