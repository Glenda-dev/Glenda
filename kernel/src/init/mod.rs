mod console;
mod cpu;
mod irq;
mod platform;
mod pmem;
mod proc;
mod trap;
mod vm;

use crate::hal;
use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, Ordering};

static INIT_DONE: AtomicBool = AtomicBool::new(false);

pub fn init() {
    platform::init();
    console::init();
    pmem::init();
    vm::init();
    trap::init();
    irq::init();
    proc::init();
    cpu::init();
    init_guard();
}

fn init_guard() {
    let cpuid = hal::cpu::cpu_id();
    if cpuid == 0 {
        // 标记初始化完成，允许其他核心进入调度器
        INIT_DONE.store(true, Ordering::Release);
    } else {
        while !INIT_DONE.load(Ordering::Acquire) {
            spin_loop();
        }
    }
}
