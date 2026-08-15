//! Runnable process queue using bitmap for O(1) scheduling
//! 
//! This module provides a bitmap-based queue to quickly find runnable processes,
//! improving scheduler performance from O(n) to O(1).

use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;
use super::table::NPROC;

/// Bitmap tracking runnable processes
/// Each bit represents a process index (0-63)
/// Bit set = process is runnable, bit clear = process is not runnable
static RUNNABLE_BITMAP: AtomicU64 = AtomicU64::new(0);

/// Lock for synchronizing bitmap updates with process table
static BITMAP_LOCK: Mutex<()> = Mutex::new(());

/// Mark a process as runnable
pub fn mark_runnable(proc_idx: usize) {
    if proc_idx >= NPROC {
        return;
    }
    let bit = 1u64 << proc_idx;
    RUNNABLE_BITMAP.fetch_or(bit, Ordering::Relaxed);
}

/// Mark a process as not runnable
pub fn mark_not_runnable(proc_idx: usize) {
    if proc_idx >= NPROC {
        return;
    }
    let bit = 1u64 << proc_idx;
    RUNNABLE_BITMAP.fetch_and(!bit, Ordering::Relaxed);
}

/// Find the first runnable process index
/// Returns None if no runnable process exists
/// 
/// This uses trailing_zeros() which is typically implemented as a single CPU instruction
/// (e.g., TZCNT on x86, CLZ on ARM), making it O(1) in practice.
pub fn find_runnable() -> Option<usize> {
    let bitmap = RUNNABLE_BITMAP.load(Ordering::Acquire);
    if bitmap == 0 {
        return None;
    }
    let idx = bitmap.trailing_zeros() as usize;
    if idx < NPROC {
        Some(idx)
    } else {
        None
    }
}

/// Clear the runnable bit for a process (used when scheduling it)
pub fn clear_runnable_bit(proc_idx: usize) {
    if proc_idx >= NPROC {
        return;
    }
    let bit = 1u64 << proc_idx;
    RUNNABLE_BITMAP.fetch_and(!bit, Ordering::Relaxed);
}

/// Get a lock for synchronizing bitmap updates with process table operations
pub fn lock() -> spin::MutexGuard<'static, ()> {
    BITMAP_LOCK.lock()
}

/// Check if any process is runnable (without modifying the bitmap)
pub fn has_runnable() -> bool {
    RUNNABLE_BITMAP.load(Ordering::Acquire) != 0
}

/// Clear all runnable bits (for initialization or debugging)
pub fn clear_all() {
    RUNNABLE_BITMAP.store(0, Ordering::Release);
}

/// Find the index of a process in the process table
/// Returns None if the process is not found
pub fn find_proc_index(proc: *const super::Process) -> Option<usize> {
    use super::table::PROC_TABLE;
    let table = PROC_TABLE.lock();
    let base = table.as_ptr() as usize;
    let proc_addr = proc as usize;
    
    if proc_addr < base {
        return None;
    }
    
    let offset = proc_addr - base;
    let proc_size = core::mem::size_of::<super::Process>();
    let idx = offset / proc_size;
    
    if idx < NPROC && (base + idx * proc_size) == proc_addr {
        Some(idx)
    } else {
        None
    }
}
