#![allow(dead_code)]

use core::cell::OnceCell;
use core::cmp;
use core::hint::spin_loop;
use core::ptr::{self, NonNull, addr_of_mut};

use spin::{Mutex, Once};

use crate::dtb;
use crate::printk;
use crate::printk::{ANSI_BLUE, ANSI_RED, ANSI_RESET};

pub const PGSIZE: usize = 4096;

unsafe extern "C" {
    static mut __bss_end: u8;
}

#[repr(C)]
struct FreePage {
    next: Option<NonNull<FreePage>>,
}

#[derive(Clone, Copy)]
struct RegionInner {
    head: Option<NonNull<FreePage>>,
    allocable: usize,
}

#[derive(Debug, Clone, Copy)]
struct RegionBounds {
    begin: usize,
    end: usize,
}

struct AllocRegion {
    bounds: OnceCell<RegionBounds>,
    inner: Mutex<RegionInner>,
}

unsafe impl Sync for AllocRegion {}

impl AllocRegion {
    const fn new() -> Self {
        Self {
            bounds: OnceCell::new(),
            inner: Mutex::new(RegionInner { head: None, allocable: 0 }),
        }
    }

    unsafe fn init(&self, begin: usize, end: usize) {
        let begin_aligned = align_up(begin, PGSIZE);
        let end_aligned = align_down(end, PGSIZE);

        let mut head: Option<NonNull<FreePage>> = None;
        let mut count = 0usize;
        let mut current = begin_aligned;

        while current + PGSIZE <= end_aligned {
            let page = current as *mut FreePage;
            unsafe {
                (*page).next = head;
            }
            head = NonNull::new(page);
            count += 1;
            current += PGSIZE;
        }

        self.bounds
            .set(RegionBounds { begin: begin_aligned, end: end_aligned })
            .expect("AllocRegion::init called twice");

        *self.inner.lock() = RegionInner { head, allocable: count };
    }

    fn region_info(&self) -> RegionInfo {
        let b = *self.bounds.get().expect("region not initialized");
        let allocable = self.inner.lock().allocable;
        RegionInfo { begin: b.begin, end: b.end, allocable }
    }

    fn allocate(&self) -> Option<*mut u8> {
        let head_ptr = {
            let mut inner = self.inner.lock();
            let head = inner.head?;
            let next = unsafe { (*head.as_ptr()).next };
            inner.head = next;
            inner.allocable = inner.allocable.saturating_sub(1);
            head
        };

        let p = head_ptr.as_ptr() as *mut u8;
        unsafe { ptr::write_bytes(p, 0, PGSIZE) };
        Some(p)
    }

    fn free(&self, addr: usize) {
        let b = *self.bounds.get().expect("region not initialized");
        if addr < b.begin || addr >= b.end || addr % PGSIZE != 0 {
            panic!("pmem_free: address {:#x} out of bounds [{:#x}, {:#x})", addr, b.begin, b.end);
        }

        let mut inner = self.inner.lock();
        unsafe {
            ptr::write_bytes(addr as *mut u8, 0, PGSIZE);
            let page = addr as *mut FreePage;
            (*page).next = inner.head;
            inner.head = NonNull::new(page);
        }
        inner.allocable += 1;
    }
}

static KERNEL_REGION: AllocRegion = AllocRegion::new();
static USER_REGION: AllocRegion = AllocRegion::new();
static PMEM_ONCE: Once<()> = Once::new();

#[derive(Clone, Copy, Debug)]
pub struct RegionInfo {
    pub begin: usize,
    pub end: usize,
    pub allocable: usize,
}

pub fn pmem_init() {
    PMEM_ONCE.call_once(|| {
        initialize_regions();
    });
    while PMEM_ONCE.is_completed() == false {
        spin_loop();
    }
}

pub fn pmem_alloc(for_kernel: bool) -> *mut u8 {
    match allocate_page(for_kernel) {
        Some(ptr) => ptr,
        None => {
            if for_kernel {
                panic!("pmem_alloc: kernel region exhausted");
            } else {
                panic!("pmem_alloc: user region exhausted");
            }
        }
    }
}

#[cfg(feature = "tests")]
pub fn pmem_try_alloc(for_kernel: bool) -> Option<*mut u8> {
    allocate_page(for_kernel)
}

pub fn pmem_free(addr: usize, for_kernel: bool) {
    region(for_kernel).free(addr);
}

pub fn kernel_region_info() -> RegionInfo {
    KERNEL_REGION.region_info()
}

pub fn user_region_info() -> RegionInfo {
    USER_REGION.region_info()
}

fn allocate_page(for_kernel: bool) -> Option<*mut u8> {
    region(for_kernel).allocate()
}

fn region(for_kernel: bool) -> &'static AllocRegion {
    if for_kernel { &KERNEL_REGION } else { &USER_REGION }
}

fn initialize_regions() {
    let kernel_end = align_up(addr_of_mut!(__bss_end) as usize, PGSIZE);

    let mem_range = dtb::memory_range()
        .unwrap_or_else(|| dtb::MemoryRange { start: 0x8000_0000, size: 128 * 1024 * 1024 });
    let mem_start = mem_range.start;
    let mem_end = mem_range.start + mem_range.size;

    if kernel_end >= mem_end {
        printk!(
            "{}PMEM init failed{}: kernel end {:#x} beyond memory end {:#x}",
            ANSI_RED,
            ANSI_RESET,
            kernel_end,
            mem_end
        );
        panic!("pmem_init: kernel overlaps physical memory end");
    }

    let alloc_begin = cmp::max(kernel_end, mem_start);
    let alloc_end = mem_end;
    let total_free = alloc_end.saturating_sub(alloc_begin);

    let mut kernel_split = align_up(alloc_begin + total_free / 2, PGSIZE);
    if kernel_split > alloc_end {
        kernel_split = alloc_end;
    }
    if kernel_split < alloc_begin {
        kernel_split = alloc_begin;
    }

    unsafe {
        KERNEL_REGION.init(alloc_begin, kernel_split);
        USER_REGION.init(kernel_split, alloc_end);
    }

    let k = KERNEL_REGION.region_info();
    let u = USER_REGION.region_info();

    printk!(
        "{}PMEM initialized{}: kernel [{:#x}, {:#x}) -> {} pages, user [{:#x}, {:#x}) -> {} pages",
        ANSI_BLUE,
        ANSI_RESET,
        k.begin,
        k.end,
        k.allocable,
        u.begin,
        u.end,
        u.allocable
    );
}

#[inline(always)]
const fn align_up(value: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());
    (value + align - 1) & !(align - 1)
}

#[inline(always)]
const fn align_down(value: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());
    value & !(align - 1)
}
