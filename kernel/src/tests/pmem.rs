use super::barrier::MultiCoreTestBarrier;
use core::cell::UnsafeCell;
use core::cmp;
use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::dtb;
use crate::hart::MAX_HARTS;
use crate::mem::PGSIZE;
use crate::mem::addr::PhysAddr;
use crate::mem::pmem::{self, kernel_region_info, user_region_info};
use crate::printk;
use crate::printk::{ANSI_GREEN, ANSI_RESET, ANSI_YELLOW};

const MAX_TRACKED_PAGES: usize = 32; // 每个 hart 记录的最大页数

// 并发阶段 barrier + 完成计数
static CONCURRENT_BARRIER: MultiCoreTestBarrier = MultiCoreTestBarrier::new();
static HARTS_FINISHED: AtomicUsize = AtomicUsize::new(0); // 完成计数
static TEST_DONE: AtomicBool = AtomicBool::new(false); // kernel 并发阶段完成
static ALL_DONE: AtomicBool = AtomicBool::new(false); // 整个 PMEM 测试（含 user 区）完成
static ACTIVE_HARTS: AtomicUsize = AtomicUsize::new(0); // 实际参加测试的 hart 数
static INITIAL_ALLOCABLE: AtomicUsize = AtomicUsize::new(0); // 初始 allocable 记录

struct HartSlotTable {
    slots: UnsafeCell<[[usize; MAX_TRACKED_PAGES]; MAX_HARTS]>,
}
impl HartSlotTable {
    const fn new() -> Self {
        Self { slots: UnsafeCell::new([[0; MAX_TRACKED_PAGES]; MAX_HARTS]) }
    }
    #[inline]
    fn store(&self, hart: usize, idx: usize, value: PhysAddr) {
        unsafe {
            (*self.slots.get())[hart][idx] = value;
        }
    }
    #[inline]
    fn load(&self, hart: usize, idx: usize) -> usize {
        unsafe { (*self.slots.get())[hart][idx] }
    }
}
unsafe impl Sync for HartSlotTable {}

static PAGE_SLOTS: HartSlotTable = HartSlotTable::new();

pub fn run(hartid: usize) {
    printk!("{}[TEST]{} PMEM test started on hart {}", ANSI_YELLOW, ANSI_RESET, hartid);
    kernel_concurrent_alloc_test(hartid);

    if hartid == 0 {
        // 等待 kernel 并发阶段全部完成（即 TEST_DONE）
        while !TEST_DONE.load(Ordering::Acquire) {
            spin_loop();
        }
        // 再进行 user region 测试，避免与并发阶段重叠
        user_region_validation();
        printk!("{}[PASS]{} PMEM test", ANSI_GREEN, ANSI_RESET);
        ALL_DONE.store(true, Ordering::Release);
    } else {
        // 非 hart0：等待整个测试完成
        while !ALL_DONE.load(Ordering::Acquire) {
            spin_loop();
        }
    }
}

fn kernel_concurrent_alloc_test(hartid: usize) {
    // 仅 hart0 读取一次初始信息并确定参与者数量
    if hartid == 0 {
        let info = kernel_region_info();
        let mut active = cmp::min(dtb::hart_count(), MAX_HARTS);
        if info.allocable == 0 {
            active = 0;
        }
        ACTIVE_HARTS.store(active, Ordering::Release);
        INITIAL_ALLOCABLE.store(info.allocable, Ordering::Release);
    } else {
        // 等待 ACTIVE_HARTS 设置
        while ACTIVE_HARTS.load(Ordering::Acquire) == 0
            && INITIAL_ALLOCABLE.load(Ordering::Acquire) == 0
        {
            // 若 allocable=0，hart0 会将 active=0，并把 INITIAL_ALLOCABLE=0；保持自旋到 hart0 完成 PASS
            if TEST_DONE.load(Ordering::Acquire) {
                return;
            }
            spin_loop();
        }
    }

    let active = ACTIVE_HARTS.load(Ordering::Acquire);
    if active == 0 {
        if hartid == 0 {
            printk!("pmem::kernel_concurrent: kernel region empty (allocable=0)");
            // 立即标记完成
            TEST_DONE.store(true, Ordering::Release);
        }
        return;
    }
    if hartid >= active {
        printk!("pmem::kernel_concurrent: hart {} idle ({} active)", hartid, active);
        // 等待测试整体完成
        while !TEST_DONE.load(Ordering::Acquire) {
            spin_loop();
        }
        return;
    }

    // 初始化并进入启动栅栏
    if CONCURRENT_BARRIER.total() == 0 {
        CONCURRENT_BARRIER.init(active);
    }
    CONCURRENT_BARRIER.wait_start();

    // 分配页数：至少 1，平均分配，限制 MAX_TRACKED_PAGES
    let total_pages = INITIAL_ALLOCABLE.load(Ordering::Acquire);
    let base = if active > 0 { total_pages / active } else { 0 };
    let pages_per_hart = cmp::max(1, cmp::min(MAX_TRACKED_PAGES, base.max(1)));

    for slot in 0..pages_per_hart {
        let page = pmem::alloc(true) as PhysAddr;
        unsafe {
            core::ptr::write_bytes(page as *mut u8, hartid as u8 + 1, PGSIZE);
        }
        PAGE_SLOTS.store(hartid, slot, page);
    }

    // 立即释放（无需中间栅栏——页面相互独立）
    for slot in 0..pages_per_hart {
        let addr = PAGE_SLOTS.load(hartid, slot);
        pmem::free(addr, true);
        PAGE_SLOTS.store(hartid, slot, 0);
    }

    let finished = HARTS_FINISHED.fetch_add(1, Ordering::SeqCst) + 1;
    if finished == active {
        // 最终校验
        let final_info = kernel_region_info();
        let expected = INITIAL_ALLOCABLE.load(Ordering::Acquire);
        assert_eq!(
            final_info.allocable, expected,
            "pmem::kernel_concurrent: final allocable {} expected {}",
            final_info.allocable, expected
        );
        printk!("pmem::kernel_concurrent: {} pages restored", expected);
        TEST_DONE.store(true, Ordering::SeqCst); // 仅表示 kernel 并发部分结束
    } else {
        while !TEST_DONE.load(Ordering::Acquire) {
            spin_loop();
        }
    }
}

fn user_region_validation() {
    const TEST_CNT: usize = 10;
    let mut pages = [0usize; TEST_CNT];

    let before = user_region_info();
    let allocable_before = before.allocable;
    let pages_to_use = cmp::min(TEST_CNT, allocable_before);

    for idx in 0..pages_to_use {
        let page = pmem::alloc(false) as PhysAddr;
        pages[idx] = page;
        unsafe {
            core::ptr::write_bytes(page as *mut u8, 0xAA, PGSIZE);
        }
    }

    let during = user_region_info();
    let expected_after_alloc = allocable_before.saturating_sub(pages_to_use);
    assert_eq!(
        during.allocable, expected_after_alloc,
        "pmem::user_region: allocable after alloc {} expected {}",
        during.allocable, expected_after_alloc
    );

    for idx in 0..pages_to_use {
        pmem::free(pages[idx], false);
    }

    let after = user_region_info();
    assert_eq!(
        after.allocable, allocable_before,
        "pmem::user_region: allocable after free {} expected {}",
        after.allocable, allocable_before
    );

    let mut zero_verified = true;
    for idx in 0..pages_to_use {
        let page = pmem::alloc(false) as PhysAddr;
        pages[idx] = page;
        zero_verified &= is_zeroed(page);
    }

    for idx in 0..pages_to_use {
        pmem::free(pages[idx], false);
    }
    assert_eq!(
        user_region_info().allocable,
        allocable_before,
        "pmem::user_region: allocable after zero-check free {} expected {}",
        user_region_info().allocable,
        allocable_before
    );

    assert!(zero_verified, "pmem::user_region: zero check failed");

    assert!(exhaust_user_region(), "pmem::user_region: exhaustion test failed");
    printk!("pmem::user_region: allocation/free/zero validated");
}

fn is_zeroed(page: usize) -> bool {
    let word_sz = core::mem::size_of::<usize>();
    let words = PGSIZE / word_sz;
    let mut i = 0;
    while i < words {
        let v = unsafe { core::ptr::read((page as *const usize).add(i)) };
        if v != 0 {
            return false;
        }
        i += 1;
    }
    true
}

fn exhaust_user_region() -> bool {
    let mut head: usize = 0;
    let mut count = 0usize;

    loop {
        match pmem::try_alloc(false) {
            Some(page) => {
                unsafe {
                    core::ptr::write(page as *mut usize, head);
                }
                head = page as usize;
                count += 1;
            }
            None => break,
        }
    }

    let mut node = head;
    while node != 0 {
        let next = unsafe { core::ptr::read(node as *const usize) };
        pmem::free(node, false);
        node = next;
    }

    count > 0
}
