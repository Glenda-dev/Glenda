use core::mem::offset_of;
use core::ptr::null_mut;

use spin::{Mutex, Once};

use crate::printk::{uart_hex, uart_puts};

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
    next: *mut MmapRegionNode,
}

impl MmapRegionNode {
    const fn new() -> Self {
        Self { mmap: MmapRegion::zero(), next: core::ptr::null_mut() }
    }
}

// Global node warehouse (singly-linked free list), protected by a lock.
static INIT_ONCE: Once = Once::new();
static LIST_LOCK: Mutex<()> = Mutex::new(());
static mut LIST_HEAD: MmapRegionNode = MmapRegionNode::new(); // sentinel, not allocatable
static mut NODE_LIST: [MmapRegionNode; N_MMAP] = [MmapRegionNode::new(); N_MMAP];

fn node_index(ptr: *mut MmapRegionNode) -> Option<usize> {
    let base = core::ptr::addr_of_mut!(NODE_LIST) as usize;
    let end = base + core::mem::size_of::<MmapRegionNode>() * N_MMAP;
    let p = ptr as usize;
    if p < base || p >= end {
        return None;
    }
    let idx = (p - base) / core::mem::size_of::<MmapRegionNode>();
    Some(idx)
}

pub fn init() {
    INIT_ONCE.call_once(|| unsafe {
        // Build free list: LIST_HEAD.next -> NODE_LIST[0] -> ... -> NODE_LIST[N-1] -> null
        let base = core::ptr::addr_of_mut!(NODE_LIST) as *mut MmapRegionNode;
        for i in 0..N_MMAP {
            let node_i = base.add(i);
            (*node_i).mmap = MmapRegion::zero();
            (*node_i).next = if i + 1 < N_MMAP { base.add(i + 1) } else { null_mut() };
        }
        core::ptr::addr_of_mut!(LIST_HEAD).as_mut().unwrap().next = base;
        uart_puts("MMAP: warehouse initialized (nodes=");
        uart_hex(N_MMAP);
        uart_puts(")\n");
    });
}

// Allocate a node from the warehouse and return a pointer to its embedded MmapRegion.
pub fn region_alloc() -> *mut MmapRegion {
    init();
    let _g = LIST_LOCK.lock();
    unsafe {
        let head = core::ptr::addr_of_mut!(LIST_HEAD) as *mut MmapRegionNode;
        let first = (*head).next;
        if first.is_null() {
            uart_puts("MMAP: alloc failed (empty)\n");
            return null_mut();
        }
        (*head).next = (*first).next;
        (*first).next = null_mut();
        (*first).mmap = MmapRegion::zero();
        if let Some(idx) = node_index(first) {
            uart_puts("MMAP: alloc node index = ");
            uart_hex(idx);
            uart_puts("\n");
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
    let _g = LIST_LOCK.lock();
    unsafe {
        // Recover node pointer from the embedded field offset
        let node_ptr = (region as usize - offset_of!(MmapRegionNode, mmap)) as *mut MmapRegionNode;
        (*node_ptr).mmap = MmapRegion::zero();
        if let Some(idx) = node_index(node_ptr) {
            uart_puts("MMAP: free node index = ");
            uart_hex(idx);
            uart_puts("\n");
        }
        let head = core::ptr::addr_of_mut!(LIST_HEAD) as *mut MmapRegionNode;
        (*node_ptr).next = (*head).next;
        (*head).next = node_ptr;
    }
}

// Debug helper: dump current free-list order by node index
#[cfg(debug_assertions)]
pub fn print_nodelist() {
    init();
    let _g = LIST_LOCK.lock();
    unsafe {
        uart_puts("MMAP: free-list indices: ");
        let mut p = LIST_HEAD.next;
        let mut first = true;
        while !p.is_null() {
            if !first {
                uart_puts(", ");
            } else {
                first = false;
            }
            if let Some(idx) = node_index(p) {
                uart_hex(idx);
            } else {
                uart_puts("?");
            }
            p = (*p).next;
        }
        uart_puts("\n");
    }
}

// Debug: show a per-process mmap list (allocated regions)
#[cfg(debug_assertions)]
pub fn print_mmaplist(mut head: *mut MmapRegion) {
    unsafe {
        uart_puts("MMAP: mmap list -> ");
        let mut first = true;
        while !head.is_null() {
            if !first {
                uart_puts(" | ");
            } else {
                first = false;
            }
            uart_puts("[begin=");
            uart_hex((*head).begin);
            uart_puts(", pages=");
            uart_hex((*head).npages as usize);
            uart_puts("]");
            head = (*head).next;
        }
        if first {
            uart_puts("<empty>");
        }
        uart_puts("\n");
    }
}
