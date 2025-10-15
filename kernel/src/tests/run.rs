use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, Ordering};

use super::barrier::FINAL_BARRIER;
use crate::mem::vm::{vm_switch_off, vm_switch_to_kernel, init_kernel_vm};
use crate::printk;
use crate::printk::{ANSI_GREEN, ANSI_RESET};

// 标志最终 PASS 已打印
static FINAL_DONE: AtomicBool = AtomicBool::new(false);

pub fn run_tests(hartid: usize) {
    super::spinlock::run(hartid);
    super::printk::run(hartid);
    vm_switch_off(); // 关闭 VM，确保测试在非分页环境下运行
    super::pmem::run(hartid);
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
    // 在 test() 返回前所有 hart 打印进入主循环提示，避免因为后续输出被截断而误判未进入
    printk!("Hart {} tests done, returning to main loop", hartid);
    vm_switch_to_kernel(hartid); // 恢复 VM，返回内核页表
}
