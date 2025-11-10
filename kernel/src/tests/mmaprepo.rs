use super::barrier::MultiCoreTestBarrier;
use crate::dtb;
use crate::mem::mmap::{self, MmapRegion, N_MMAP};
use crate::printk;
use core::cell::UnsafeCell;
use core::hint::spin_loop;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static START_BARRIER: MultiCoreTestBarrier = MultiCoreTestBarrier::new();
static ALLOC_BARRIER: MultiCoreTestBarrier = MultiCoreTestBarrier::new();
static FREE_BARRIER: MultiCoreTestBarrier = MultiCoreTestBarrier::new();
static ACTIVE: AtomicUsize = AtomicUsize::new(0);
static ALL_DONE: AtomicBool = AtomicBool::new(false);

struct SharedList {
    slots: UnsafeCell<[*mut MmapRegion; N_MMAP]>,
}
unsafe impl Sync for SharedList {}

impl SharedList {
    const fn new() -> Self {
        Self { slots: UnsafeCell::new([core::ptr::null_mut(); N_MMAP]) }
    }
    #[inline]
    fn store(&self, i: usize, p: *mut MmapRegion) {
        unsafe {
            (*self.slots.get())[i] = p;
        }
    }
    #[inline]
    fn load(&self, i: usize) -> *mut MmapRegion {
        unsafe { (*self.slots.get())[i] }
    }
}

static MMAP_LIST: SharedList = SharedList::new();

pub fn run(hartid: usize) {
    // Decide active participants (up to 2 harts)
    if hartid == 0 {
        let active = core::cmp::min(2, dtb::hart_count());
        ACTIVE.store(active, Ordering::Release);
        START_BARRIER.init(active);
        ALLOC_BARRIER.init(active);
        FREE_BARRIER.init(active);
    } else {
        while ACTIVE.load(Ordering::Acquire) == 0 {
            spin_loop();
        }
    }
    let active = ACTIVE.load(Ordering::Acquire);
    if active == 0 {
        return;
    }

    if hartid == 0 {
        printk!("[TEST] mmap warehouse begin");
        mmap::init();
        mmap::print_nodelist();
    }

    if hartid >= active {
        // idle harts
        while !ALL_DONE.load(Ordering::Acquire) {
            spin_loop();
        }
        return;
    }

    START_BARRIER.wait_start();

    // Allocation phase
    if hartid == 0 {
        for i in 0..(N_MMAP / 2) {
            let p = mmap::region_alloc();
            MMAP_LIST.store(i, p);
        }
    } else if hartid == 1 {
        for i in (N_MMAP / 2)..N_MMAP {
            let p = mmap::region_alloc();
            MMAP_LIST.store(i, p);
        }
    }

    ALLOC_BARRIER.wait_start();

    // Free phase
    if hartid == 0 {
        for i in (0..(N_MMAP / 2)).rev() {
            let p = MMAP_LIST.load(i);
            mmap::region_free(p);
        }
    } else if hartid == 1 {
        for i in ((N_MMAP / 2)..N_MMAP).rev() {
            let p = MMAP_LIST.load(i);
            mmap::region_free(p);
        }
    }

    FREE_BARRIER.wait_start();

    if hartid == 0 {
        // Show final free-list state
        mmap::print_nodelist();
        printk!("[PASS] mmap warehouse");
        ALL_DONE.store(true, Ordering::Release);
    }
}
