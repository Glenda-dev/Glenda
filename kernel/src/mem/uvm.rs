use core::cmp::min;

use super::addr::{align_down, align_up};
use super::pagetable::PageTable;
use super::pmem::pmem_alloc;
use super::pte::{pte_get_flags, pte_is_leaf, pte_is_valid, pte_to_pa, PTE_A, PTE_D, PTE_R, PTE_U, PTE_W};
use super::{PGSIZE, VirtAddr, MMAP_BEGIN};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyError {
    NotMapped,
    NoPerm,
    Fault,
    TooLong,
}

#[inline(always)]
fn page_offset(addr: usize) -> usize { addr & (PGSIZE - 1) }

pub fn uvm_copyin(pt: &PageTable, dst: &mut [u8], mut src_va: VirtAddr) -> Result<(), CopyError> {
    let mut copied = 0usize;
    while copied < dst.len() {
        let va = src_va;
        let pte_ptr = match pt.lookup(align_down(va)) { Some(p) => p, None => return Err(CopyError::NotMapped) };
        let pte = unsafe { *pte_ptr };
        if !pte_is_valid(pte) || !pte_is_leaf(pte) { return Err(CopyError::NotMapped) };
        let flags = pte_get_flags(pte);
        if (flags & 0xE) == 0 { return Err(CopyError::NotMapped); }
        if (flags & PTE_U) == 0 || (flags & PTE_R) == 0 { return Err(CopyError::NoPerm); }
        let pa = pte_to_pa(pte);
        let off = page_offset(va);
        let n = min(PGSIZE - off, dst.len() - copied);
        unsafe {
            core::ptr::copy_nonoverlapping((pa + off) as *const u8, dst.as_mut_ptr().add(copied), n);
        }
        copied += n;
        src_va += n;
    }
    Ok(())
}

pub fn uvm_copyout(pt: &PageTable, mut dst_va: VirtAddr, src: &[u8]) -> Result<(), CopyError> {
    let mut copied = 0usize;
    while copied < src.len() {
        let va = dst_va;
        let pte_ptr = match pt.lookup(align_down(va)) { Some(p) => p, None => return Err(CopyError::NotMapped) };
        let pte = unsafe { *pte_ptr };
        if !pte_is_valid(pte) || !pte_is_leaf(pte) { return Err(CopyError::NotMapped) };
        let flags = pte_get_flags(pte);
        if (flags & 0xE) == 0 { return Err(CopyError::NotMapped); }
        if (flags & PTE_U) == 0 || (flags & PTE_W) == 0 { return Err(CopyError::NoPerm); }
        let pa = pte_to_pa(pte);
        let off = page_offset(va);
        let n = min(PGSIZE - off, src.len() - copied);
        unsafe {
            core::ptr::copy_nonoverlapping(src.as_ptr().add(copied), (pa + off) as *mut u8, n);
        }
        copied += n;
        dst_va += n;
    }
    Ok(())
}

pub fn uvm_copyin_str(pt: &PageTable, dst: &mut [u8], mut src_va: VirtAddr) -> Result<usize, CopyError> {
    let mut copied = 0usize;
    while copied < dst.len() {
        let page_base = align_down(src_va);
        let pte_ptr = match pt.lookup(page_base) { Some(p) => p, None => return Err(CopyError::NotMapped) };
        let pte = unsafe { *pte_ptr };
        if !pte_is_valid(pte) || !pte_is_leaf(pte) { return Err(CopyError::NotMapped) };
        let flags = pte_get_flags(pte);
        if (flags & 0xE) == 0 { return Err(CopyError::NotMapped); }
        if (flags & PTE_U) == 0 || (flags & PTE_R) == 0 { return Err(CopyError::NoPerm); }
        let pa = pte_to_pa(pte);
        let mut off = page_offset(src_va);
        while off < PGSIZE && copied < dst.len() {
            let byte = unsafe { core::ptr::read_volatile((pa + off) as *const u8) };
            dst[copied] = byte;
            copied += 1;
            off += 1;
            if byte == 0 { return Ok(copied); }
        }
        src_va = page_base + PGSIZE;
    }
    Err(CopyError::TooLong)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UvmError {
    OutOfRange,
    NoMem,
    MapFailed,
}

// 堆增长：在 (align_up(old_top), align_up(new_top)) 区间内逐页分配并映射
pub fn uvm_heap_grow(pt: &mut PageTable, old_top: VirtAddr, new_top: VirtAddr) -> Result<(), UvmError> {
    if new_top > MMAP_BEGIN { return Err(UvmError::OutOfRange); }
    let mut a = align_up(old_top);
    let last = align_up(new_top);
    while a < last {
        let pa = pmem_alloc(false) as usize;
        if pa == 0 { return Err(UvmError::NoMem); }
        if !pt.map(a, pa, PGSIZE, PTE_U | PTE_R | PTE_W | PTE_A | PTE_D) {
            return Err(UvmError::MapFailed);
        }
        a += PGSIZE;
    }
    Ok(())
}

// 堆收缩：在 [align_up(new_top), align_up(old_top)) 区间内逐页解除映射并释放
pub fn uvm_heap_ungrow(pt: &mut PageTable, old_top: VirtAddr, new_top: VirtAddr) -> Result<(), UvmError> {
    if new_top > old_top { return Ok(()); }
    let start = align_up(new_top);
    let size = align_up(old_top).saturating_sub(start);
    if size == 0 { return Ok(()); }
    if !pt.unmap(start, size, true) { return Err(UvmError::MapFailed); }
    Ok(())
}

pub fn uvm_ustack_grow(pt: &mut PageTable, _stack_pages: &mut usize, _trapframe_va: VirtAddr, fault_va: VirtAddr) -> Result<(), UvmError> {
    const STACK_BASE: usize = 0x20000;
    const STACK_SIZE: usize = 24576;
    const STACK_TOP: usize = STACK_BASE + STACK_SIZE;

    if fault_va >= STACK_BASE && fault_va < STACK_TOP {
        let needed_base = align_down(fault_va);
        let mut a = STACK_TOP;
        while a > needed_base {
            a -= PGSIZE;
            if let Some(pte_ptr) = pt.lookup(a) {
                if pte_is_valid(unsafe { *pte_ptr }) {
                    continue;
                }
            }
            let pa = pmem_alloc(false) as usize;
            if pa == 0 { return Err(UvmError::NoMem); }
            if !pt.map(a, pa, PGSIZE, PTE_U | PTE_R | PTE_W | PTE_A | PTE_D) {
                return Err(UvmError::MapFailed);
            }
        }
        return Ok(());
    }

    Err(UvmError::OutOfRange)
}
