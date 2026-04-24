use super::PhysAddr;
use crate::boot;
use crate::cap::CNODE_PAGES;
use crate::cap::{CNode, Capability, Rights};
use crate::hal::mem::PGSIZE;
use crate::mem::PageTable;
use crate::mem::UntypedRegion;
use crate::mem::addr::phys_to_virt;
use crate::platform;
// use crate::printk;
use crate::proc::TCB;
use crate::proc::asid;
use crate::sync::SpinLock;

const MAX_PMEM_REGIONS: usize = 16;

#[derive(Clone, Copy, Debug)]
struct RegionState {
    start: PhysAddr,
    current: PhysAddr,
    end: PhysAddr,
}

impl RegionState {
    const fn new() -> Self {
        Self { start: PhysAddr::null(), current: PhysAddr::null(), end: PhysAddr::null() }
    }
}

struct PmemManager {
    regions: [RegionState; MAX_PMEM_REGIONS],
    count: usize,
}

impl PmemManager {
    const fn new() -> Self {
        Self { regions: [RegionState::new(); MAX_PMEM_REGIONS], count: 0 }
    }

    fn add_region(&mut self, start: PhysAddr, end: PhysAddr) {
        if self.count >= MAX_PMEM_REGIONS {
            warn!("pmem: Warning, ignoring memory region [{}, {}) due to limit", start, end);
            return;
        }
        if end <= start {
            return;
        }

        self.regions[self.count] = RegionState { start, current: start, end };
        self.count += 1;
        log!("pmem: Added region [{}, {})", start, end);
    }

    fn init(&mut self, kernel_end: PhysAddr) {
        let mut available = 0;
        let mmap = crate::boot::get_mem_map();
        if mmap.is_empty() {
            panic!("pmem: No memory map entries found from bootloader");
        }
        let initrd = crate::boot::get_initrd();
        let (initrd_start, initrd_end) = if let Some((vaddr, size)) = initrd {
            let paddr = PhysAddr::from(vaddr.as_usize());
            (paddr, paddr + size)
        } else {
            (PhysAddr::null(), PhysAddr::null())
        };

        log!(
            "pmem: Initializing pmem regions. Kernel end at {}, found {} regions in bootloader map",
            kernel_end,
            mmap.len()
        );
        // Clear existing regions just in case
        self.count = 0;

        for entry in mmap {
            if entry.kind == platform::MemoryType::Ram {
                let r_start = entry.base;
                let r_end = r_start + entry.length;

                // Check overlap with kernel image (assumed to be from 0..kernel_end or ram_start..kernel_end)
                // A simple heuristic: if the region contains kernel_end, we start allocation after kernel_end
                // If the region is completely below kernel_end, we ignore it (it's kernel code/data)

                let effective_start = if r_start < kernel_end && r_end > kernel_end {
                    log!(
                        "pmem: Adjusting region start {} -> {} due to kernel overlap",
                        r_start,
                        kernel_end
                    );
                    kernel_end
                } else {
                    r_start
                };

                let mut current_start = effective_start;
                let mut current_end = r_end;

                if initrd_start != PhysAddr::null() {
                    // Initrd within this region
                    if initrd_start >= current_start && initrd_end <= current_end {
                        log!(
                            "pmem: Splitting region for initrd [{}, {})",
                            initrd_start,
                            initrd_end
                        );
                        // Add region before initrd
                        if initrd_start > current_start {
                            self.add_region(current_start, initrd_start);
                            available += initrd_start.as_usize() - current_start.as_usize();
                        }
                        // Start next region after initrd
                        current_start = initrd_end;
                    } else if initrd_start < current_start && initrd_end > current_start {
                        // Overlaps start
                        log!(
                            "pmem: Adjusting region start due to initrd overlap [{}, {})",
                            initrd_start,
                            initrd_end
                        );
                        current_start = initrd_end;
                    } else if initrd_start < current_end && initrd_end > current_end {
                        // Overlaps end (initrd starts inside, ends outside)
                        log!(
                            "pmem: Adjusting region end due to initrd overlap [{}, {})",
                            initrd_start,
                            initrd_end
                        );
                        current_end = initrd_start;
                    }
                }

                if current_start < current_end {
                    self.add_region(current_start, current_end);
                    available += current_end.as_usize() - current_start.as_usize();
                }
            }
        }
        log!(
            "pmem: Initialization complete with {} regions, {} MB available",
            self.count,
            available / (1024 * 1024)
        );
    }

    fn alloc_addr(&mut self, size: usize, align: usize) -> Option<PhysAddr> {
        for i in 0..self.count {
            let region = &mut self.regions[i];
            // Ensure alignment for allocations
            let aligned_current = region.current.align_up(align);
            if aligned_current + size <= region.end {
                let paddr = aligned_current;
                region.current = aligned_current + size;

                // Zero the allocated frame
                unsafe {
                    let vaddr = phys_to_virt(paddr);
                    core::ptr::write_bytes(vaddr.as_mut_ptr::<u8>(), 0, size);
                }
                //log!("pmem: Allocated [{}, {}) in region {}", paddr, paddr + size, i);
                return Some(paddr);
            }
        }
        None
    }

    fn print_debug(&self) {
        printk!("PmemManager Status:\n");
        for i in 0..self.count {
            let r = &self.regions[i];
            printk!(
                "  Region {}: [{:#x}, {:#x}), Free: {} KB\n",
                i,
                r.current.as_usize(),
                r.end.as_usize(),
                (r.end - r.current).as_usize() / 1024
            );
        }
    }

    fn get_untyped_regions(&mut self) -> ([UntypedRegion; MAX_PMEM_REGIONS], usize) {
        let mut list =
            [UntypedRegion { start: PhysAddr::null(), pages: 0, watermark: 0 }; MAX_PMEM_REGIONS];

        let mut count = 0;

        for i in 0..self.count {
            let region = &mut self.regions[i];
            let size = region.end - region.current;
            if size.as_usize() >= PGSIZE {
                list[count] = UntypedRegion {
                    start: region.current,
                    pages: size.as_usize() / PGSIZE,
                    watermark: 0,
                };
                region.current = region.end; // Mark used
                count += 1;
            }
        }
        (list, count)
    }
}

static PMEM: SpinLock<PmemManager> = SpinLock::new(PmemManager::new());

pub fn debug_info() {
    PMEM.lock().print_debug();
}

pub fn initialize_regions() {
    let kernel_end = boot::get_kernel_address().0 + boot::get_kernel_size();
    PMEM.lock().init(kernel_end);
}

/// 分配一个物理页 Capability（flags 为 page level，0=4KiB, 1=8KiB, ...）
pub fn alloc_page_cap(level: usize) -> Option<Capability> {
    if level >= usize::BITS as usize {
        return None;
    }
    let pages = 1usize << level;
    let size = pages * PGSIZE;
    PMEM.lock()
        .alloc_addr(size, PGSIZE)
        .map(|paddr| Capability::create_page(&PhysPage { paddr, level }, Rights::ALL, false))
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
        let vaddr = phys_to_virt(paddr);
        let cnode = unsafe { vaddr.as_mut::<CNode>() };
        cnode.init();
        Capability::create_cnode(cnode, Rights::ALL)
    })
}

pub fn alloc_pagetable_cap(level: usize) -> Option<Capability> {
    PMEM.lock().alloc_addr(PGSIZE, PGSIZE).map(|paddr| {
        let pt = unsafe { phys_to_virt(paddr).as_mut::<PageTable>() };
        *pt = PageTable::new();
        Capability::create_pagetable(pt, level, Rights::ALL)
    })
}

pub fn alloc_vspace_cap() -> Option<Capability> {
    PMEM.lock().alloc_addr(PGSIZE, PGSIZE).map(|paddr| {
        let pt = unsafe { phys_to_virt(paddr).as_mut::<PageTable>() };
        *pt = PageTable::new();
        let asid = asid::alloc();
        Capability::create_vspace(pt, asid, Rights::ALL)
    })
}

pub fn alloc_tcb_cap() -> Option<Capability> {
    let align = core::mem::align_of::<TCB>();
    PMEM.lock().alloc_addr(core::mem::size_of::<TCB>(), align).map(|paddr| {
        let vaddr = phys_to_virt(paddr);
        let tcb = unsafe { vaddr.as_mut::<TCB>() };
        *tcb = TCB::new();
        Capability::create_tcb(tcb, Rights::ALL)
    })
}

pub fn alloc_page() -> Option<PhysAddr> {
    PMEM.lock().alloc_addr(PGSIZE, PGSIZE)
}

/// 获取剩余的 Untyped 内存区域
/// 这应该在 Root Task 创建完成后调用，用于将剩余内存移交给 Root Task
pub fn get_untyped() -> ([UntypedRegion; MAX_PMEM_REGIONS], usize) {
    let mut pmem = PMEM.lock();
    pmem.get_untyped_regions()
}

#[derive(Debug, Clone, Copy)]
pub struct PhysPage {
    pub paddr: PhysAddr,
    pub level: usize,
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
