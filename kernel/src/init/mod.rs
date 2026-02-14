mod console;
mod cpu;
mod irq;
mod platform;
mod pmem;
mod proc;
mod trap;
mod vm;

use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, Ordering};

static INIT_DONE: AtomicBool = AtomicBool::new(false);

pub fn init(is_primary: bool) {
    if is_primary {
        init_primary();
    } else {
        init_secondary();
    }
}

fn init_primary() {
    // 0. 启动协议固化
    crate::boot::init();
    // 1. 全局配置与硬件发现
    platform::init();
    cpu::init();
    console::init();
    pmem::init();
    vm::init();
    proc::init();
    // 3. 本地 CPU 初始化 (每个核心都要运行)
    trap::init();
    irq::init();
    crate::boot::bootstrap();
    // 3. 发布屏障，允许其他核心继续
    INIT_DONE.store(true, Ordering::Release);
}

fn init_secondary() {
    // 1. 等待主核心完成全局初始化
    while !INIT_DONE.load(Ordering::Acquire) {
        spin_loop();
    }
    // 2. 本地 CPU 初始化
    // 关键：必须先初始化 CPU 结构，因为后续的 vm::init() 等可能会用到 cpu::get()
    cpu::init();
    trap::init();
    irq::init();
}
