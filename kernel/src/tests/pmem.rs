use core::cell::UnsafeCell;
use core::cmp;
use core::hint::spin_loop;
use core::sync::atomic::{
    AtomicBool, AtomicUsize,
    Ordering::{AcqRel, Acquire, Release},
};

use crate::dtb;
use crate::init;
use crate::mem::PGSIZE;
use crate::mem::addr::{PhysAddr, VirtAddr};
use crate::mem::pmem::{
    kernel_region_info, pmem_alloc, pmem_free, pmem_try_alloc, user_region_info,
};
use crate::printk;
use crate::printk::{ANSI_GREEN, ANSI_RED, ANSI_RESET, ANSI_YELLOW};

const MAX_HARTS: usize = 8;
const MAX_TRACKED_PAGES: usize = 32;

static START_SYNC: AtomicBool = AtomicBool::new(false);
static HARTS_READY: AtomicUsize = AtomicUsize::new(0);
static HARTS_DONE_ALLOC: AtomicUsize = AtomicUsize::new(0);
static HARTS_DONE_FREE: AtomicUsize = AtomicUsize::new(0);
static TOTAL_PAGES: AtomicUsize = AtomicUsize::new(0);
static ACTIVE_PARTICIPANTS: AtomicUsize = AtomicUsize::new(0);
static KERNEL_INFO_READY: AtomicBool = AtomicBool::new(false);

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
        user_region_validation();
    }
    printk!("{}[PASS]{} PMEM test", ANSI_GREEN, ANSI_RESET);
}

fn kernel_concurrent_alloc_test(hartid: usize) {
    let potential = cmp::min(dtb::hart_count(), MAX_HARTS);
    if potential == 0 {
        return;
    }

    if hartid == 0 {
        let info = kernel_region_info();
        TOTAL_PAGES.store(info.allocable, Release);
        let active = if info.allocable == 0 { 0 } else { cmp::min(potential, info.allocable) };
        ACTIVE_PARTICIPANTS.store(active, Release);
        KERNEL_INFO_READY.store(true, Release);
    } else {
        while !KERNEL_INFO_READY.load(Acquire) {
            spin_loop();
        }
    }

    let active = ACTIVE_PARTICIPANTS.load(Acquire);
    if active == 0 {
        if hartid == 0 {
            printk!(
                "{}[WARN]{} pmem_kernel_concurrent: kernel region empty",
                ANSI_YELLOW,
                ANSI_RESET
            );
        }
        return;
    }

    if hartid >= active {
        printk!("pmem_kernel_concurrent: hart {} idle ({} active)", hartid, active);
        return;
    }

    let total_pages = TOTAL_PAGES.load(Acquire);
    let pages_per_hart = cmp::max(1, cmp::min(MAX_TRACKED_PAGES, total_pages / active));

    let ready = HARTS_READY.fetch_add(1, AcqRel) + 1;
    if ready == active {
        START_SYNC.store(true, Release);
    } else {
        while !START_SYNC.load(Acquire) {
            spin_loop();
        }
    }

    for slot in 0..pages_per_hart {
        let page = pmem_alloc(true) as PhysAddr;
        unsafe {
            core::ptr::write_bytes(page as *mut u8, hartid as u8 + 1, PGSIZE);
        }
        PAGE_SLOTS.store(hartid, slot, page);
    }

    HARTS_DONE_ALLOC.fetch_add(1, AcqRel);
    while HARTS_DONE_ALLOC.load(Acquire) < active {
        spin_loop();
    }

    for slot in 0..pages_per_hart {
        let addr = PAGE_SLOTS.load(hartid, slot);
        pmem_free(addr, true);
        PAGE_SLOTS.store(hartid, slot, 0);
    }

    HARTS_DONE_FREE.fetch_add(1, AcqRel);

    if hartid == 0 {
        while HARTS_DONE_FREE.load(Acquire) < active {
            spin_loop();
        }
        let final_info = kernel_region_info();
        let expected = TOTAL_PAGES.load(Acquire);
        assert_eq!(
            final_info.allocable, expected,
            "pmem_kernel_concurrent: final allocable {} expected {}",
            final_info.allocable, expected
        );
        printk!("pmem_kernel_concurrent: {} pages restored", expected);
    }
}

fn user_region_validation() {
    const TEST_CNT: usize = 10;
    let mut pages = [0usize; TEST_CNT];

    let before = user_region_info();
    let allocable_before = before.allocable;
    let pages_to_use = cmp::min(TEST_CNT, allocable_before);

    for idx in 0..pages_to_use {
        let page = pmem_alloc(false) as PhysAddr;
        pages[idx] = page;
        unsafe {
            core::ptr::write_bytes(page as *mut u8, 0xAA, PGSIZE);
        }
    }

    let during = user_region_info();
    let expected_after_alloc = allocable_before.saturating_sub(pages_to_use);
    assert_eq!(
        during.allocable, expected_after_alloc,
        "pmem_user_region: allocable after alloc {} expected {}",
        during.allocable, expected_after_alloc
    );

    for idx in 0..pages_to_use {
        pmem_free(pages[idx], false);
    }

    let after = user_region_info();
    assert_eq!(
        after.allocable, allocable_before,
        "pmem_user_region: allocable after free {} expected {}",
        after.allocable, allocable_before
    );

    let mut zero_verified = true;
    for idx in 0..pages_to_use {
        let page = pmem_alloc(false) as PhysAddr;
        pages[idx] = page;
        zero_verified &= is_zeroed(page);
    }

    for idx in 0..pages_to_use {
        pmem_free(pages[idx], false);
    }
    assert_eq!(
        user_region_info().allocable,
        allocable_before,
        "pmem_user_region: allocable after zero-check free {} expected {}",
        user_region_info().allocable,
        allocable_before
    );

    assert!(zero_verified, "pmem_user_region: zero check failed");

    assert!(exhaust_user_region(), "pmem_user_region: exhaustion test failed");
    printk!("pmem_user_region: allocation/free/zero validated");
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
        match pmem_try_alloc(false) {
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
        pmem_free(node, false);
        node = next;
    }

    count > 0
}
