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
    // 关键：在这里重新初始化内核页表，处理真正的权限控制和 HHDM
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
    // Secondary harts must also switch to the shared kernel page table
    // before they can safely run scheduled user threads.
    // This is needed when there are multiple harts
    vm::init();
    irq::init();
    INIT_CPUS_DONE.fetch_add(1, Ordering::SeqCst);
    let cpus = crate::boot::get_cpu_count();
    while INIT_CPUS_DONE.load(Ordering::SeqCst) < cpus {
        spin_loop();
    }
}
