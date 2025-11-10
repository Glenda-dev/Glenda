use core::mem::offset_of;
use core::ptr::null_mut;

use spin::{Mutex, Once};

use crate::printk;

// Number of mmap region nodes in the global warehouse
pub const N_MMAP: usize = 256;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MmapRegion {
    pub begin: usize,          // start VA (page-aligned)
    pub npages: u32,           // number of pages
    pub next: *mut MmapRegion, // next region in per-process list
}

impl MmapRegion {
    const fn zero() -> Self {
        Self { begin: 0, npages: 0, next: null_mut() }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MmapRegionNode {
    mmap: MmapRegion,
    next: usize, // pointer as usize for Send
}

impl MmapRegionNode {
    const fn new() -> Self {
        Self { mmap: MmapRegion::zero(), next: 0 }
    }
}

#[derive(Clone, Copy)]
struct MmapWarehouse {
    head: MmapRegionNode, // sentinel, not allocatable
    node_list: [MmapRegionNode; N_MMAP],
}

impl MmapWarehouse {
    const fn new() -> Self {
        Self { head: MmapRegionNode::new(), node_list: [MmapRegionNode::new(); N_MMAP] }
    }
}

unsafe impl Send for MmapWarehouse {}

// Global warehouse, protected by mutex
static INIT_ONCE: Once = Once::new();
static WAREHOUSE: Mutex<MmapWarehouse> = Mutex::new(MmapWarehouse::new());

fn node_index(ptr: *mut MmapRegionNode, warehouse: &MmapWarehouse) -> Option<usize> {
    let base = warehouse.node_list.as_ptr() as usize;
    let end = base + core::mem::size_of::<MmapRegionNode>() * N_MMAP;
    let p = ptr as usize;
    if p < base || p >= end {
        return None;
    }
    let idx = (p - base) / core::mem::size_of::<MmapRegionNode>();
    Some(idx)
}

pub fn init() {
    INIT_ONCE.call_once(|| {
        let mut warehouse = WAREHOUSE.lock();
        let base = warehouse.node_list.as_mut_ptr();
        for i in 0..N_MMAP {
            let node_i = unsafe { &mut *base.add(i) };
            node_i.mmap = MmapRegion::zero();
            node_i.next = if i + 1 < N_MMAP { unsafe { base.add(i + 1) as usize } } else { 0 };
        }
        warehouse.head.next = base as usize;
        printk!("MMAP: initialized warehouse (nodes = {})", N_MMAP);
    });
}

// Allocate a node from the warehouse and return a pointer to its embedded MmapRegion.
pub fn region_alloc() -> *mut MmapRegion {
    init();
    let mut warehouse = WAREHOUSE.lock();
    unsafe {
        let head_next = warehouse.head.next as *mut MmapRegionNode;
        if head_next.is_null() {
            printk!("MMAP: region_alloc failed - out of nodes!");
            return null_mut();
        }
        let first = head_next;
        warehouse.head.next = (*first).next;
        (*first).next = 0;
        (*first).mmap = MmapRegion::zero();
        if let Some(idx) = node_index(first, &*warehouse) {
            printk!("MMAP: alloc node index = {}", idx);
        }
        &mut (*first).mmap as *mut MmapRegion
    }
}

// Return a node back to the warehouse. The input must be a pointer previously
// returned by region_alloc() or derived from NODE_LIST.
pub fn region_free(region: *mut MmapRegion) {
    if region.is_null() {
        return;
    }
    let mut warehouse = WAREHOUSE.lock();
    unsafe {
        // Recover node pointer from the embedded field offset
        let node_ptr = (region as usize - offset_of!(MmapRegionNode, mmap)) as *mut MmapRegionNode;
        (*node_ptr).mmap = MmapRegion::zero();
        if let Some(idx) = node_index(node_ptr, &*warehouse) {
            printk!("MMAP: free node index = {}", idx);
        }
        (*node_ptr).next = warehouse.head.next;
        warehouse.head.next = node_ptr as usize;
    }
}

// Debug helper: dump current free-list order by node index
#[cfg(debug_assertions)]
pub fn print_nodelist() {
    use crate::printk;
    init();
    let warehouse = WAREHOUSE.lock();
    unsafe {
        printk!("MMAP: free-list indices:");
        let mut p = warehouse.head.next as *mut MmapRegionNode;
        while !p.is_null() {
            if let Some(idx) = node_index(p, &*warehouse) {
                printk!("  {}", idx);
            } else {
                printk!("  ?");
            }
            p = (*p).next as *mut MmapRegionNode;
        }
    }
}

// Debug: show a per-process mmap list (allocated regions)
#[cfg(debug_assertions)]
pub fn print_mmaplist(mut head: *mut MmapRegion) {
    use crate::printk;
    unsafe {
        printk!("MMAP: mmap list ->");
        let mut first = true;
        while !head.is_null() {
            if !first {
                printk!("  |");
            } else {
                first = false;
            }
            printk!("  [begin=0x{:x}, pages={}]", (*head).begin, (*head).npages);
            head = (*head).next;
        }
        if first {
            printk!("  <empty>");
        }
    }
}
