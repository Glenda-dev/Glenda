use super::PhysAddr;
use crate::cap::CNODE_PAGES;
use crate::cap::{CNode, Capability, Rights};
use crate::hal;
use crate::hal::mem::PGSIZE;
use crate::mem::PageTable;
use crate::mem::UntypedRegion;
use crate::printk;
use crate::proc::TCB;
use crate::proc::asid;
use core::ptr::addr_of_mut;
use spin::Mutex;

struct PmemManager {
    start: PhysAddr,
    current: PhysAddr,
    end: PhysAddr,
}

impl PmemManager {
    const fn new() -> Self {
        Self { start: PhysAddr::from(0), current: PhysAddr::from(0), end: PhysAddr::from(0) }
    }

    fn init(&mut self, start: PhysAddr, end: PhysAddr) {
        self.start = start;
        self.current = start;
        self.end = end;
    }

    fn alloc_addr(&mut self, size: usize, align: usize) -> Option<PhysAddr> {
        //printk!("pmem: Allocating {} bytes with alignment {}\n", size, align);
        if self.current.as_usize() == 0 {
            return None;
        }
        // Ensure alignment for allocations
        let aligned_current = self.current.align_up(align);
        if aligned_current + size <= self.end {
            let paddr = aligned_current;
            self.current = aligned_current + size;

            // Zero the allocated frame to prevent information leakage
            unsafe {
                core::ptr::write_bytes(paddr.as_mut_ptr::<u8>(), 0, size);
            }
            Some(paddr)
        } else {
            None
        }
    }

    fn print_debug(&self) {
        printk!("PmemManager Status:\n");
        printk!("  Start: {:#x}\n", self.start.as_usize());
        printk!("  Current: {:#x}\n", self.current.as_usize());
        printk!("  End: {:#x}\n", self.end.as_usize());
        printk!("  Free: {} KB\n", (self.end - self.current).as_usize() / 1024);
    }
}

static PMEM: Mutex<PmemManager> = Mutex::new(PmemManager::new());

pub fn debug_info() {
    PMEM.lock().print_debug();
}

pub fn initialize_regions(_hartid: usize) {
    let mem_range = hal::platform::memory_range().expect("Memory range not found in DTB");
    let mem_start = mem_range.start;
    let mem_end = mem_range.start + mem_range.size;

    unsafe extern "C" {
        static mut __alloc_start: u8;
    }
    let alloc_start = PhysAddr::from(addr_of_mut!(__alloc_start) as usize);
    let alloc_start = alloc_start.align_up(PGSIZE);

    printk!("pmem: Physical Memory: [{:#x}, {:#x})\n", mem_start.as_usize(), mem_end.as_usize());
    printk!("pmem: Allocator Start: {:#x}\n", alloc_start.as_usize());

    PMEM.lock().init(alloc_start, mem_end);
}

/// 分配一个物理页 Capability
pub fn alloc_frame_cap(pages: usize) -> Option<Capability> {
    PMEM.lock()
        .alloc_addr(pages * PGSIZE, PGSIZE)
        .map(|paddr| Capability::create_frame(&PhysFrame { paddr, pages }, Rights::ALL))
}

/// 分配一个 Untyped Capability
pub fn alloc_untyped_cap(size: usize) -> Option<Capability> {
    PMEM.lock().alloc_addr(size, PGSIZE).map(|paddr| {
        Capability::create_untyped(
            &UntypedRegion { start: paddr, pages: size / PGSIZE, watermark: 0 },
            Rights::ALL,
        )
    })
}

pub fn alloc_cnode_cap() -> Option<Capability> {
    let size = CNODE_PAGES * PGSIZE;
    let align = PGSIZE;
    PMEM.lock().alloc_addr(size, align).map(|paddr| {
        let vaddr = hal::mem::phys_to_virt(paddr);
        let cnode = vaddr.as_mut::<CNode>();
        *cnode = CNode::new();
        Capability::create_cnode(cnode, Rights::ALL)
    })
}

pub fn alloc_pagetable_cap(level: usize) -> Option<Capability> {
    PMEM.lock().alloc_addr(PGSIZE, PGSIZE).map(|paddr| {
        let pt = hal::mem::phys_to_virt(paddr).as_mut::<PageTable>();
        *pt = PageTable::new();
        Capability::create_pagetable(pt, level, Rights::ALL)
    })
}

pub fn alloc_vspace_cap() -> Option<Capability> {
    PMEM.lock().alloc_addr(PGSIZE, PGSIZE).map(|paddr| {
        let pt = hal::mem::phys_to_virt(paddr).as_mut::<PageTable>();
        *pt = PageTable::new();
        let asid = asid::alloc();
        Capability::create_vspace(pt, asid, Rights::ALL)
    })
}

pub fn alloc_tcb_cap() -> Option<Capability> {
    let align = core::mem::align_of::<TCB>();
    PMEM.lock().alloc_addr(core::mem::size_of::<TCB>(), align).map(|paddr| {
        let vaddr = hal::mem::phys_to_virt(paddr);
        let tcb = vaddr.as_mut::<TCB>();
        *tcb = TCB::new();
        Capability::create_tcb(tcb, Rights::ALL)
    })
}

pub fn alloc_page() -> Option<PhysAddr> {
    PMEM.lock().alloc_addr(PGSIZE, PGSIZE)
}

/// 获取剩余的 Untyped 内存区域
/// 这应该在 Root Task 创建完成后调用，用于将剩余内存移交给 Root Task
pub fn get_untyped() -> UntypedRegion {
    let pmem = PMEM.lock();
    UntypedRegion {
        start: pmem.current,
        pages: (pmem.end - pmem.current).as_usize() / PGSIZE,
        watermark: 0,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PhysFrame {
    pub paddr: PhysAddr,
    pub pages: usize,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MemoryRange {
    pub start: PhysAddr,
    pub size: usize,
}

impl MemoryRange {
    pub fn from(start: PhysAddr, size: usize) -> MemoryRange {
        MemoryRange { start, size }
    }
    pub fn end(&self) -> PhysAddr {
        self.start + self.size
    }
    pub fn empty() -> Self {
        Self { start: PhysAddr::null(), size: 0 }
    }
}
