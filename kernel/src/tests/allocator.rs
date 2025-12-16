use super::barrier::MultiCoreTestBarrier;
use crate::printk;
use crate::printk::{ANSI_GREEN, ANSI_RESET, ANSI_YELLOW};
use alloc::boxed::Box;
use alloc::vec::Vec;

static ALLOC_BARRIER: MultiCoreTestBarrier = MultiCoreTestBarrier::new();

pub fn run(hartid: usize) {
    ALLOC_BARRIER.ensure_inited(crate::dtb::hart_count());
    if hartid == 0 {
        ALLOC_BARRIER.init(crate::dtb::hart_count());
        printk!("[TEST] Allocator test start ({} harts)\n", ALLOC_BARRIER.total());
    }
    ALLOC_BARRIER.wait_start();

    test_box();
    test_vec();
    if ALLOC_BARRIER.finish_and_last() {
        printk!(
            "{}[PASS]{} Allocator test ({} harts)\n",
            ANSI_GREEN,
            ANSI_RESET,
            ALLOC_BARRIER.total()
        );
    }
}

fn test_box() {
    let val = 12345;
    let b = Box::new(val);
    assert_eq!(*b, val);
    printk!("  Box test passed\n");
}

fn test_vec() {
    let mut v = Vec::new();
    // Current allocator limit is 1 page (4096 bytes) per allocation.
    // Vec growth strategy might request more than that if we push too many elements.
    // 400 * 8 bytes = 3200 bytes. Next growth might exceed 4096.
    // Let's keep it safe.
    let n = 100;
    for i in 0..n {
        v.push(i);
    }
    assert_eq!(v.len(), n);
    for i in 0..n {
        assert_eq!(v[i], i);
    }
    drop(v);
    printk!("  Vec test passed\n");
}
