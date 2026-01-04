use super::{PGSIZE, PhysAddr};
use crate::cap::CNODE_BITS;
use crate::cap::cnode::CNodeHeader;
use crate::cap::{CNode, Capability, Slot, rights};
use crate::dtb;
use crate::mem::PageTable;
use crate::printk;
use crate::proc::TCB;
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
    let mem_range = dtb::memory_range().expect("Memory range not found in DTB");
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
pub fn alloc_frame_cap(count: usize) -> Option<Capability> {
    PMEM.lock()
        .alloc_addr(PGSIZE * count, PGSIZE)
        .map(|paddr| Capability::create_frame(paddr, count, rights::ALL))
}

/// 分配一个 Untyped Capability
pub fn alloc_untyped_cap(size: usize) -> Option<Capability> {
    PMEM.lock()
        .alloc_addr(size, PGSIZE)
        .map(|paddr| Capability::create_untyped(paddr, size / PGSIZE, rights::ALL))
}

pub fn alloc_cnode_cap() -> Option<Capability> {
    let size =
        (1 << CNODE_BITS) * core::mem::size_of::<Slot>() + core::mem::size_of::<CNodeHeader>();
    let align = core::mem::align_of::<Slot>();
    PMEM.lock().alloc_addr(size, align).map(|paddr| {
        CNode::new(paddr);
        Capability::create_cnode(paddr, rights::ALL)
    })
}

pub fn alloc_pagetable_cap(level: usize) -> Option<Capability> {
    PMEM.lock().alloc_addr(PGSIZE, PGSIZE).map(|paddr| {
        let pt = paddr.to_va().as_mut::<PageTable>();
        *pt = PageTable::new();
        Capability::create_pagetable(paddr, level, rights::ALL)
    })
}

pub fn alloc_tcb_cap() -> Option<Capability> {
    let align = core::mem::align_of::<TCB>();
    PMEM.lock().alloc_addr(core::mem::size_of::<TCB>(), align).map(|paddr| {
        let tcb = paddr.to_va().as_mut::<TCB>();
        *tcb = TCB::new();
        Capability::create_thread(paddr.to_va(), rights::ALL)
    })
}

/// 获取剩余的 Untyped 内存区域
/// 这应该在 Root Task 创建完成后调用，用于将剩余内存移交给 Root Task
pub fn get_untyped() -> UntypedRegion {
    let pmem = PMEM.lock();
    UntypedRegion { start: pmem.current, end: pmem.end }
}

/// 获取保留的未分配内存区域
pub fn get_preserved_untyped() -> UntypedRegion {
    let pmem = PMEM.lock();
    UntypedRegion { start: PhysAddr::null(), end: pmem.start - 1 }
}
