use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, Ordering};

use super::barrier::FINAL_BARRIER;
use crate::mem::vm::{init_kernel_vm, vm_switch_off, vm_switch_to_kernel};
use crate::printk;
use crate::printk::{ANSI_GREEN, ANSI_RESET};

// 标志最终 PASS 已打印
static FINAL_DONE: AtomicBool = AtomicBool::new(false);

pub fn run_tests(hartid: usize) {
    vm_switch_off(hartid); // 关闭 VM，确保测试在非分页环境下运行
    super::spinlock::run(hartid);
    super::printk::run(hartid);
    super::pmem::run(hartid);
    init_kernel_vm(hartid);
    vm_switch_to_kernel(hartid);
    super::vm::run(hartid);
    // 最终同步：所有测试结束后再统一进入 main loop
    // 初始化（任意先到可执行）；如果已经 init 则忽略
    FINAL_BARRIER.ensure_inited(crate::dtb::hart_count());
    FINAL_BARRIER.wait_start();
    let last = FINAL_BARRIER.finish_and_last();
    if last {
        printk!(
            "{}All tests completed across {} harts{}",
            ANSI_GREEN,
            FINAL_BARRIER.total(),
            ANSI_RESET
        );
        FINAL_DONE.store(true, Ordering::Release);
    } else {
        while !FINAL_DONE.load(Ordering::Acquire) {
            spin_loop();
        }
    }
    vm_switch_to_kernel(hartid); // 恢复 VM，返回内核页表
}
