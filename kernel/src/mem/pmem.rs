use super::PhysAddr;
use crate::boot::UntypedDesc;
use crate::cap::CNODE_PAGES;
use crate::cap::{CNode, Capability, Rights};
use crate::hal;
use crate::hal::mem::PGSIZE;
use crate::hal::mem::PageTable;
use crate::printk;
use crate::proc::TCB;
use crate::proc::asid;
use core::ptr::addr_of_mut;
use spin::Mutex;

/// Untyped 内存区域描述符
/// 这部分内存不被内核分配器管理，而是直接暴露给 Root Task
#[derive(Clone, Copy, Debug)]
pub struct UntypedRegion {
    pub start: PhysAddr,
    pub end: PhysAddr,
}

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
}

static PMEM: Mutex<PmemManager> = Mutex::new(PmemManager::new());

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
        .map(|paddr| Capability::create_frame(paddr, pages, Rights::ALL))
}

/// 分配一个 Untyped Capability
pub fn alloc_untyped_cap(size: usize) -> Option<Capability> {
    PMEM.lock()
        .alloc_addr(size, PGSIZE)
        .map(|paddr| Capability::create_untyped(paddr, size / PGSIZE, Rights::ALL))
}

pub fn alloc_cnode_cap(bits: u8) -> Option<Capability> {
    let size = CNODE_PAGES * PGSIZE;
    let align = PGSIZE;
    PMEM.lock().alloc_addr(size, align).map(|paddr| {
        let vaddr = hal::mem::phys_to_virt(paddr);
        let cnode = vaddr.as_mut::<CNode>();
        *cnode = CNode::new(bits);
        Capability::create_cnode(vaddr, Rights::ALL)
    })
}

pub fn alloc_pagetable_cap(level: usize) -> Option<Capability> {
    PMEM.lock().alloc_addr(PGSIZE, PGSIZE).map(|paddr| {
        let pt = hal::mem::phys_to_virt(paddr).as_mut::<PageTable>();
        *pt = PageTable::new();
        Capability::create_pagetable(paddr, level, Rights::ALL)
    })
}

pub fn alloc_vspace_cap() -> Option<Capability> {
    PMEM.lock().alloc_addr(PGSIZE, PGSIZE).map(|paddr| {
        let pt = hal::mem::phys_to_virt(paddr).as_mut::<PageTable>();
        *pt = PageTable::new();
        let asid = asid::alloc();
        Capability::create_vspace(paddr, asid, Rights::ALL)
    })
}

pub fn alloc_tcb_cap() -> Option<Capability> {
    let align = core::mem::align_of::<TCB>();
    PMEM.lock().alloc_addr(core::mem::size_of::<TCB>(), align).map(|paddr| {
        let vaddr = hal::mem::phys_to_virt(paddr);
        let tcb = vaddr.as_mut::<TCB>();
        *tcb = TCB::new();
        Capability::create_tcb(vaddr, Rights::ALL)
    })
}

pub fn alloc_page() -> Option<PhysAddr> {
    PMEM.lock().alloc_addr(PGSIZE, PGSIZE)
}

/// 获取剩余的 Untyped 内存区域
/// 这应该在 Root Task 创建完成后调用，用于将剩余内存移交给 Root Task
pub fn get_untyped() -> UntypedDesc {
    let pmem = PMEM.lock();
    UntypedDesc { paddr: pmem.current, size: (pmem.end - pmem.current).as_usize() / PGSIZE }
}
