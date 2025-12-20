use super::{KERN_PAGES, PGSIZE, PhysAddr};
use crate::dtb;
use crate::printk;
use core::ptr::{self, NonNull, addr_of_mut};
use spin::Mutex;
use spin::Once;

const PHY_MEM_START: usize = 0x8000_0000;
const TOTAL_PAGES: usize = 128 * 1024 * 1024 / PGSIZE; // 32768

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

/// 仅用于内核启动阶段的简单分配器
struct BootAllocRegion {
    bounds: Once<RegionBounds>,
    inner: Mutex<RegionInner>,
}

unsafe impl Sync for BootAllocRegion {}

impl BootAllocRegion {
    const fn new() -> Self {
        Self { bounds: Once::new(), inner: Mutex::new(RegionInner { head: None, allocable: 0 }) }
    }

    fn contains(&self, addr: PhysAddr) -> bool {
        if let Some(b) = self.bounds.get() { addr >= b.begin && addr < b.end } else { false }
    }

    unsafe fn init(&self, begin: PhysAddr, end: PhysAddr) {
        // ... 初始化链表逻辑保持不变 ...
        let begin_aligned = begin.align_up(PGSIZE);
        let end_aligned = end.align_down(PGSIZE);

        let mut head: Option<NonNull<FreePage>> = None;
        let mut count = 0usize;
        let mut current = begin_aligned;

        while current + PGSIZE <= end_aligned {
            let page = current.as_mut::<FreePage>();
            (*page).next = head;
            head = NonNull::new(page);
            count += 1;
            current += PGSIZE;
        }

        self.bounds.call_once(|| RegionBounds { begin, end });
        *self.inner.lock() = RegionInner { head, allocable: count };
    }

    /// 仅限内核启动时调用
    fn allocate(&self) -> Option<*mut u8> {
        let mut inner = self.inner.lock();
        let head = inner.head?;
        let next = unsafe { (*head.as_ptr()).next };
        inner.head = next;
        inner.allocable = inner.allocable.saturating_sub(1);

        let p = head.as_ptr() as *mut u8;
        // 必须清零，防止信息泄漏
        unsafe { ptr::write_bytes(p, 0, PGSIZE) };
        Some(p)
    }

    fn info(&self) -> RegionInner {
        *self.inner.lock()
    }

    fn free(&self, pa: PhysAddr) -> Result<(), ()> {
        if !self.contains(pa) {
            return Err(());
        }

        let mut inner = self.inner.lock();
        let page = pa.as_mut::<FreePage>();
        (*page).next = inner.head;
        inner.head = NonNull::new(page);
        inner.allocable += 1;
        Ok(())
    }
}

/// Untyped 内存区域描述符
/// 这部分内存不被内核分配器管理，而是直接暴露给 Root Task
#[derive(Clone, Copy, Debug)]
pub struct UntypedRegion {
    pub start: PhysAddr,
    pub end: PhysAddr,
}

static KERNEL_REGION: BootAllocRegion = BootAllocRegion::new();
static USER_REGION: Once<UntypedRegion> = Once::new();

pub fn initialize_regions(hartid: usize) {
    let mem_range = dtb::memory_range().expect("Memory range not found in DTB");
    let mem_end = PhysAddr::from(mem_range.start + mem_range.size);
    let alloc_begin = PhysAddr::from(addr_of_mut!(__alloc_start) as usize);
    printk!("pmem: Physical Memory Range: [{:#x}, {:#x})\n", mem_range.start, mem_end.as_usize());

    // 划分内核保留区和用户 Untyped 区
    let mut kernel_split = alloc_begin + KERN_PAGES * PGSIZE;
    kernel_split = kernel_split.align_up(PGSIZE);
    if kernel_split > mem_end {
        kernel_split = mem_end;
    }

    unsafe {
        // 1. 初始化内核分配器 (仅管理 KERNEL_REGION)
        KERNEL_REGION.init(alloc_begin, kernel_split);

        // 2. 记录用户 Untyped 区域 (不初始化链表，保持原样)
        USER_REGION.call_once(|| UntypedRegion { start: kernel_split, end: mem_end });
    }
    printk!(
        "pmem: Initialized on hart {}\n\
        pmem: Kernel: [{:#x}, {:#x}) (allocable pages: {})\n\
        pmem: Untyped: [{:#x}, {:#x}) (size: {} MiB)\n",
        hartid,
        alloc_begin.as_usize(),
        kernel_split.as_usize(),
        KERNEL_REGION.info().allocable,
        kernel_split.as_usize(),
        mem_end.as_usize(),
        (mem_end - kernel_split).as_usize() / (1024 * 1024)
    );
}

/// 分配一个物理页，仅供 PhysFrame 使用
pub(super) fn alloc_frame() -> Option<PhysAddr> {
    KERNEL_REGION.allocate().map(|p| PhysAddr::from(p as usize))
}

/// 释放一个物理页，仅供 PhysFrame 使用
pub(super) fn free_frame(pa: PhysAddr) {
    KERNEL_REGION.free(pa).expect("Free Failed: Address not in kernel region");
}

pub fn get_untyped() -> UntypedRegion {
    // TODO: 目前只有一个大的连续区域，未来可能有多个碎片
    USER_REGION.get().cloned().expect("Untyped region not initialized")
}
