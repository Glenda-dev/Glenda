mod bootloader;
mod dtb;
mod hart;
mod irq;
mod pmem;
mod proc;
mod trap;
mod uart;
mod vm;

use crate::logo;
use crate::printk;
use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, Ordering};

static INIT_DONE: AtomicBool = AtomicBool::new(false);

pub fn init(hartid: usize, dtb: *const u8) {
    dtb::init(hartid, dtb);
    uart::init(hartid, dtb);
    pmem::init(hartid, dtb);
    bootloader::init(hartid, dtb);
    trap::init(hartid, dtb);
    irq::init(hartid, dtb);
    vm::init(hartid, dtb);
    proc::init(hartid, dtb);
    hart::init(hartid, dtb);
    if hartid == 0 {
        printk!("{}", logo::LOGO);
        if let Some(args) = crate::dtb::bootargs() {
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
