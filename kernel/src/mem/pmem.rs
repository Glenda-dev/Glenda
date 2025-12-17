#![allow(dead_code)]

use core::cell::OnceCell;
use core::ptr::{self, NonNull, addr_of_mut};
use core::sync::atomic::{AtomicU8, Ordering};

use spin::Mutex;

use super::addr::{align_down, align_up};
use super::{KERN_PAGES, PGSIZE, PhysAddr};
use crate::dtb;
use crate::printk;

const PHY_MEM_START: usize = 0x8000_0000;
const TOTAL_PAGES: usize = 128 * 1024 * 1024 / PGSIZE; // 32768
static PAGE_REF: [AtomicU8; TOTAL_PAGES] = [const { AtomicU8::new(0) }; TOTAL_PAGES];

fn pa_to_index(pa: usize) -> usize {
    if pa < PHY_MEM_START {
        panic!("pa_to_index: pa {:#x} too low", pa);
    }
    let offset = pa - PHY_MEM_START;
    let idx = offset / PGSIZE;
    if idx >= TOTAL_PAGES {
        panic!("pa_to_index: pa {:#x} too high", pa);
    }
    idx
}

unsafe extern "C" {
    static mut __bss_end: u8;
    static mut __alloc_start: u8;
}

pub fn initialize_regions(hartid: usize) {
    let kernel_end = align_up(addr_of_mut!(__bss_end) as PhysAddr);

    let mem_range = dtb::memory_range()
        .unwrap_or_else(|| dtb::MemoryRange { start: 0x8000_0000, size: 128 * 1024 * 1024 });
    let mem_end = mem_range.start + mem_range.size;

    if kernel_end >= mem_end {
        panic!("pmem_init: kernel end {:#x} beyond memory end {:#x}", kernel_end, mem_end);
    }

    let alloc_begin = addr_of_mut!(__alloc_start) as PhysAddr;
    assert!(alloc_begin >= align_up(addr_of_mut!(__bss_end) as PhysAddr));
    assert_eq!(alloc_begin & (PGSIZE - 1), 0, "__alloc_start must be 4K-aligned");

    let alloc_end = mem_end;
    let total_free = alloc_end.saturating_sub(alloc_begin);
    printk!(
        "PMEM: physical memory [{:#x}, {:#x}) -> {} MiB, free [{:#x}, {:#x}) -> {} MiB\n",
        mem_range.start,
        mem_end,
        mem_range.size / (1024 * 1024),
        alloc_begin,
        alloc_end,
        total_free / (1024 * 1024)
    );

    let mut kernel_split = align_up(alloc_begin + KERN_PAGES * PGSIZE);
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

    let k = KERNEL_REGION.info();
    let u = USER_REGION.info();
    assert_eq!(k.begin & (PGSIZE - 1), 0);
    assert_eq!(k.end & (PGSIZE - 1), 0);
    assert_eq!(u.begin & (PGSIZE - 1), 0);
    assert_eq!(u.end & (PGSIZE - 1), 0);

    printk!(
        "PMEM: Initialized kernel [{:#x}, {:#x}) -> {} pages, user [{:#x}, {:#x}) -> {} pages on hart {}\n",
        k.begin,
        k.end,
        k.allocable,
        u.begin,
        u.end,
        u.allocable,
        hartid
    );
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
    begin: PhysAddr,
    end: PhysAddr,
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

    fn contains(&self, addr: PhysAddr) -> bool {
        if let Some(b) = self.bounds.get() { addr >= b.begin && addr < b.end } else { false }
    }

    unsafe fn init(&self, begin: PhysAddr, end: PhysAddr) {
        let begin_aligned = align_up(begin);
        let end_aligned = align_down(end);

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

    fn info(&self) -> RegionInfo {
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
        let idx = pa_to_index(p as usize);
        if PAGE_REF[idx].fetch_add(1, Ordering::SeqCst) != 0 {
            panic!("pmem_alloc: ref count corrupted (expected 0) at {:#x}", p as usize);
        }
        unsafe { ptr::write_bytes(p, 0, PGSIZE) };
        Some(p)
    }

    fn free(&self, addr: PhysAddr) {
        let b = *self.bounds.get().expect("region not initialized");
        if addr < b.begin || addr >= b.end || addr % PGSIZE != 0 {
            panic!("pmem_free: address {:#x} out of bounds [{:#x}, {:#x}]", addr, b.begin, b.end);
        }

        let idx = pa_to_index(addr);
        let old = PAGE_REF[idx].fetch_sub(1, Ordering::SeqCst);
        if old == 0 {
            panic!("pmem_free: double free detected at {:#x}", addr);
        } else if old > 1 {
            // Ref count was > 1, so it is still used (COW or shared). Don't free yet.
            return;
        }

        // old == 1, so now it is 0. Proceed to free.
        let mut inner = self.inner.lock();
        unsafe {
            let page = addr as *mut FreePage;
            (*page).next = inner.head;
            inner.head = NonNull::new(page);
        }
        inner.allocable += 1;
    }
}
static KERNEL_REGION: AllocRegion = AllocRegion::new();
static USER_REGION: AllocRegion = AllocRegion::new();

#[derive(Clone, Copy, Debug)]
pub struct RegionInfo {
    pub begin: PhysAddr,
    pub end: PhysAddr,
    pub allocable: usize,
}

pub fn alloc(for_kernel: bool) -> *mut u8 {
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

pub fn free(addr: PhysAddr, _for_kernel: bool) {
    if KERNEL_REGION.contains(addr) {
        KERNEL_REGION.free(addr);
    } else if USER_REGION.contains(addr) {
        USER_REGION.free(addr);
    } else {
        panic!("pmem_free: address {:#x} out of all regions", addr);
    }
}

pub fn kernel_region_info() -> RegionInfo {
    KERNEL_REGION.info()
}

pub fn user_region_info() -> RegionInfo {
    USER_REGION.info()
}

fn allocate_page(for_kernel: bool) -> Option<*mut u8> {
    region(for_kernel).allocate()
}

fn region(for_kernel: bool) -> &'static AllocRegion {
    if for_kernel { &KERNEL_REGION } else { &USER_REGION }
}

#[inline]
pub fn kernel_pool_range() -> (PhysAddr, PhysAddr) {
    let info = KERNEL_REGION.info();
    (info.begin, info.end)
}

pub fn get_region(pa: PhysAddr) -> Option<bool> {
    let kern_region = kernel_region_info();
    let user_region = user_region_info();
    if pa >= kern_region.begin && pa < kern_region.end {
        return Some(true);
    } else if pa >= user_region.begin && pa < user_region.end {
        return Some(false);
    } else {
        return None;
    }
}
