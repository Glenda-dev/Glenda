#![allow(dead_code)]
//! 多核测试同步工具，抽象常用的参与/启动/完成模式。
//! 设计目标：
//! - 统一减少测试中重复的原子与自旋样板代码
//! - 提供 barrier + last-run 回调 / 单次初始化 支持
//! - 零分配、No-Std、可在早期环境使用

use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// MultiCoreTestBarrier 典型用法：
/// 1. hart0 调用 `init(total_harts)` （或使用 `ensure_inited` 自动推迟初始化）
/// 2. 所有参与者调用 `wait_start()` 进入启动栅栏
/// 3. 执行测试工作
/// 4. 调用 `finish_and_last()`，最后一个返回 `true`，用于打印 PASS / 额外验证
pub struct MultiCoreTestBarrier {
    total: AtomicUsize,
    started: AtomicBool,
    arrived: AtomicUsize,
    finished: AtomicUsize,
}

impl MultiCoreTestBarrier {
    pub const fn new() -> Self {
        Self {
            total: AtomicUsize::new(0),
            started: AtomicBool::new(false),
            arrived: AtomicUsize::new(0),
            finished: AtomicUsize::new(0),
        }
    }

    /// 初始化（仅调用一次）。重复调用如果 total 不同会 panic。
    pub fn init(&self, total: usize) {
        let prev = self.total.load(Ordering::Acquire);
        if prev == 0 {
            self.total.store(total, Ordering::Release);
        } else if prev != total {
            panic!(
                "MultiCoreTestBarrier: re-init with different total (prev={}, new={})",
                prev, total
            );
        }
    }

    /// 如果尚未 init，则由调用者提供 total 并初始化。
    pub fn ensure_inited(&self, total: usize) {
        if self.total.load(Ordering::Acquire) == 0 {
            self.init(total);
        }
    }

    pub fn total(&self) -> usize {
        self.total.load(Ordering::Acquire)
    }

    /// 到达启动栅栏；最后一个到达者设置 started。
    pub fn wait_start(&self) {
        let total = self.total();
        if total == 0 {
            return;
        }
        let arrived = self.arrived.fetch_add(1, Ordering::AcqRel) + 1;
        if arrived == total {
            self.started.store(true, Ordering::Release);
        } else {
            while !self.started.load(Ordering::Acquire) {
                spin_loop();
            }
        }
    }

    /// 标记完成；返回是否为最后一个完成的参与者。
    pub fn finish_and_last(&self) -> bool {
        let total = self.total();
        if total == 0 {
            return true;
        }
        let finished = self.finished.fetch_add(1, Ordering::AcqRel) + 1;
        finished == total
    }
}

/// 单次初始化工具：首次调用执行闭包，其它调用自旋等待完成。
pub struct OnceInit {
    done: AtomicBool,
    in_progress: AtomicBool,
}

impl OnceInit {
    pub const fn new() -> Self {
        Self { done: AtomicBool::new(false), in_progress: AtomicBool::new(false) }
    }

    pub fn call_once<F: FnOnce()>(&self, init: F) {
        if self.done.load(Ordering::Acquire) {
            return;
        }
        if self
            .in_progress
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            init();
            self.done.store(true, Ordering::Release);
        } else {
            while !self.done.load(Ordering::Acquire) {
                spin_loop();
            }
        }
    }
}

/// 简单的双阶段：阶段1所有参与者先完成，阶段2最后一个执行回调。
pub struct TwoPhase {
    barrier: MultiCoreTestBarrier,
    phase1_done: AtomicBool,
}

impl TwoPhase {
    pub const fn new() -> Self {
        Self { barrier: MultiCoreTestBarrier::new(), phase1_done: AtomicBool::new(false) }
    }

    pub fn init(&self, total: usize) {
        self.barrier.init(total);
    }

    pub fn phase1(&self) {
        self.barrier.wait_start();
        // 所有线程到达后 started=true，因此这里直接标记 phase1 完成
        if self.barrier.finish_and_last() {
            self.phase1_done.store(true, Ordering::Release);
        } else {
            while !self.phase1_done.load(Ordering::Acquire) {
                spin_loop();
            }
        }
    }

    pub fn phase2_last(&self) -> bool {
        self.barrier.finish_and_last()
    }
}

pub static PRINTK_BARRIER: MultiCoreTestBarrier = MultiCoreTestBarrier::new();
/// 全部测试结束的最终 barrier，用于保证所有 hart 在进入主循环前都完成测试并统一打印。
pub static FINAL_BARRIER: MultiCoreTestBarrier = MultiCoreTestBarrier::new();
