mod console;
mod cpu;
mod irq;
mod platform;
mod pmem;
mod proc;
mod trap;
mod vm;

use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static INIT_DONE: AtomicBool = AtomicBool::new(false);
static INIT_CPUS_DONE: AtomicUsize = AtomicUsize::new(0);

pub fn init(is_primary: bool) {
    if is_primary {
        init_primary();
    } else {
        init_secondary();
    }
}

fn init_primary() {
    // 0. 启动协议固化
    unsafe { crate::boot::init() };
    // 1. 全局配置与硬件发现
    cpu::init();
    trap::init();
    pmem::init();
    platform::init();
    console::init();
    vm::init();
    proc::init();
    irq::init();
    // 3. 本地 CPU 初始化 (每个核心都要运行)
    crate::boot::bootstrap();
    // 3. 发布屏障，允许其他核心继续
    INIT_DONE.store(true, Ordering::Release);
    INIT_CPUS_DONE.fetch_add(1, Ordering::SeqCst);
    let cpus = crate::boot::get_cpu_count();
    while INIT_CPUS_DONE.load(Ordering::SeqCst) < cpus {
        spin_loop();
    }
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
    INIT_CPUS_DONE.fetch_add(1, Ordering::SeqCst);
    let cpus = crate::boot::get_cpu_count();
    while INIT_CPUS_DONE.load(Ordering::SeqCst) < cpus {
        spin_loop();
    }
}
