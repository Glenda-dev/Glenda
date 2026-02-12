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

pub fn init(is_primary: bool) {
    if is_primary {
        init_primary();
    } else {
        init_secondary();
    }
}

fn init_primary() {
    // 1. 全局子系统初始化 (仅运行一次)
    platform::init();
    console::init();
    pmem::init();
    vm::init();
    proc::init();

    // 2. 本地 CPU 初始化 (每个核心都要运行)
    trap::init();
    irq::init();
    cpu::init();
    hal::platform::bootstrap_cpus();

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
    // 注意：vm::init() 内部会调用 switch_to_kernel() 加载页表
    vm::init();
    trap::init();
    irq::init();
}
