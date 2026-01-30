mod console;
mod hart;
mod irq;
mod platform;
mod pmem;
mod proc;
mod trap;
mod vm;

use crate::hal;
use crate::hal::platform::PlatformInfo;
use crate::logo;
use crate::printk;
use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, Ordering};

static INIT_DONE: AtomicBool = AtomicBool::new(false);

pub fn init(cpuid: usize, info: PlatformInfo) {
    platform::init(cpuid, info);
    console::init(cpuid, info);
    pmem::init(cpuid, info);
    vm::init(cpuid, info);
    trap::init(cpuid, info);
    irq::init(cpuid, info);
    proc::init(cpuid, info);
    hart::init(cpuid, info);
    init_guard(cpuid);
}

fn init_guard(cpuid: usize) {
    if cpuid == 0 {
        printk!("{}", logo::LOGO);
        if let Some(args) = hal::platform::bootargs() {
            printk!("bootargs: {}\n", args);
        }
        // 标记初始化完成，允许其他核心进入调度器
        INIT_DONE.store(true, Ordering::Release);
    } else {
        while !INIT_DONE.load(Ordering::Acquire) {
            spin_loop();
        }
    }
}
