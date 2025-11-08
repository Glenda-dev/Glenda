use core::cmp::min;

use super::addr::{align_down, align_up};
use super::mmap::{MmapRegion, mmap_region_alloc, mmap_region_free};
use super::pagetable::PageTable;
use super::pmem::pmem_alloc;
use super::pmem::{get_region, pmem_free};
use super::pte::{
    PTE_A, PTE_D, PTE_R, PTE_U, PTE_W, PTE_X, pte_get_flags, pte_is_leaf, pte_is_table,
    pte_is_valid, pte_to_pa,
};
use super::{MMAP_BEGIN, PGSIZE, PhysAddr, VirtAddr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyError {
    NotMapped,
    NoPerm,
    Fault,
    TooLong,
}

#[inline(always)]
fn page_offset(addr: usize) -> usize {
    addr & (PGSIZE - 1)
}

pub fn uvm_copyin(pt: &PageTable, dst: &mut [u8], mut src_va: VirtAddr) -> Result<(), CopyError> {
    let mut copied = 0usize;
    while copied < dst.len() {
        let va = src_va;
        let pte_ptr = match pt.lookup(align_down(va)) {
            Some(p) => p,
            None => return Err(CopyError::NotMapped),
        };
        let pte = unsafe { *pte_ptr };
        if !pte_is_valid(pte) || !pte_is_leaf(pte) {
            return Err(CopyError::NotMapped);
        };
        let flags = pte_get_flags(pte);
        if (flags & 0xE) == 0 {
            return Err(CopyError::NotMapped);
        }
        if (flags & PTE_U) == 0 || (flags & PTE_R) == 0 {
            return Err(CopyError::NoPerm);
        }
        let pa = pte_to_pa(pte);
        let off = page_offset(va);
        let n = min(PGSIZE - off, dst.len() - copied);
        unsafe {
            core::ptr::copy_nonoverlapping(
                (pa + off) as *const u8,
                dst.as_mut_ptr().add(copied),
                n,
            );
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
        let pte_ptr = match pt.lookup(align_down(va)) {
            Some(p) => p,
            None => return Err(CopyError::NotMapped),
        };
        let pte = unsafe { *pte_ptr };
        if !pte_is_valid(pte) || !pte_is_leaf(pte) {
            return Err(CopyError::NotMapped);
        };
        let flags = pte_get_flags(pte);
        if (flags & 0xE) == 0 {
            return Err(CopyError::NotMapped);
        }
        if (flags & PTE_U) == 0 || (flags & PTE_W) == 0 {
            return Err(CopyError::NoPerm);
        }
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

pub fn uvm_copyin_str(
    pt: &PageTable,
    dst: &mut [u8],
    mut src_va: VirtAddr,
) -> Result<usize, CopyError> {
    let mut copied = 0usize;
    while copied < dst.len() {
        let page_base = align_down(src_va);
        let pte_ptr = match pt.lookup(page_base) {
            Some(p) => p,
            None => return Err(CopyError::NotMapped),
        };
        let pte = unsafe { *pte_ptr };
        if !pte_is_valid(pte) || !pte_is_leaf(pte) {
            return Err(CopyError::NotMapped);
        };
        let flags = pte_get_flags(pte);
        if (flags & 0xE) == 0 {
            return Err(CopyError::NotMapped);
        }
        if (flags & PTE_U) == 0 || (flags & PTE_R) == 0 {
            return Err(CopyError::NoPerm);
        }
        let pa = pte_to_pa(pte);
        let mut off = page_offset(src_va);
        while off < PGSIZE && copied < dst.len() {
            let byte = unsafe { core::ptr::read_volatile((pa + off) as *const u8) };
            dst[copied] = byte;
            copied += 1;
            off += 1;
            if byte == 0 {
                return Ok(copied);
            }
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
pub fn uvm_heap_grow(
    pt: &mut PageTable,
    old_top: VirtAddr,
    new_top: VirtAddr,
) -> Result<(), UvmError> {
    if new_top > MMAP_BEGIN {
        return Err(UvmError::OutOfRange);
    }
    let mut a = align_up(old_top);
    let last = align_up(new_top);
    while a < last {
        let pa = pmem_alloc(false) as usize;
        if pa == 0 {
            return Err(UvmError::NoMem);
        }
        if !pt.map(a, pa, PGSIZE, PTE_U | PTE_R | PTE_W | PTE_A | PTE_D) {
            return Err(UvmError::MapFailed);
        }
        a += PGSIZE;
    }
    Ok(())
}

// 堆收缩：在 [align_up(new_top), align_up(old_top)) 区间内逐页解除映射并释放
pub fn uvm_heap_ungrow(
    pt: &mut PageTable,
    old_top: VirtAddr,
    new_top: VirtAddr,
) -> Result<(), UvmError> {
    if new_top > old_top {
        return Ok(());
    }
    let start = align_up(new_top);
    let size = align_up(old_top).saturating_sub(start);
    if size == 0 {
        return Ok(());
    }
    if !pt.unmap(start, size, true) {
        return Err(UvmError::MapFailed);
    }
    Ok(())
}

pub fn uvm_ustack_grow(
    pt: &mut PageTable,
    _stack_pages: &mut usize,
    _trapframe_va: VirtAddr,
    fault_va: VirtAddr,
) -> Result<(), UvmError> {
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
            if pa == 0 {
                return Err(UvmError::NoMem);
            }
            if !pt.map(a, pa, PGSIZE, PTE_U | PTE_R | PTE_W | PTE_A | PTE_D) {
                return Err(UvmError::MapFailed);
            }
        }
        return Ok(());
    }

    Err(UvmError::OutOfRange)
}

#[inline(always)]
fn pages_of_len(len: usize) -> usize {
    (align_up(len) / PGSIZE).max(0)
}

fn map_pages(pt: &mut PageTable, va: VirtAddr, npages: usize) -> Result<(), UvmError> {
    let mut a = align_down(va);
    let last = a + npages * PGSIZE;
    while a < last {
        let pa = pmem_alloc(false) as usize;
        if pa == 0 {
            return Err(UvmError::NoMem);
        }
        if !pt.map(a, pa, PGSIZE, PTE_U | PTE_R | PTE_W | PTE_A | PTE_D) {
            return Err(UvmError::MapFailed);
        }
        a += PGSIZE;
    }
    Ok(())
}

pub fn uvm_mmap(
    pt: &mut PageTable,
    head: &mut *mut MmapRegion,
    mut begin: VirtAddr,
    len: usize,
    _flags: usize,
    mmap_begin: usize,
    mmap_end: usize,
) -> Result<VirtAddr, UvmError> {
    if len == 0 {
        return Err(UvmError::OutOfRange);
    }
    let npages = align_up(len) / PGSIZE;
    if npages == 0 {
        return Err(UvmError::OutOfRange);
    }

    unsafe {
        if begin == 0 {
            let mut cur = *head;
            let mut cursor = mmap_begin;

            while !cur.is_null() {
                let cur_begin = (*cur).begin;
                if cur_begin >= cursor {
                    let gap = cur_begin.saturating_sub(cursor);
                    if gap >= npages * PGSIZE {
                        begin = cursor;
                        break;
                    }
                    cursor = (*cur).begin + (*cur).npages as usize * PGSIZE;
                }
                cur = (*cur).next;
            }
            if begin == 0 {
                if mmap_end.saturating_sub(cursor) >= npages * PGSIZE {
                    begin = cursor;
                } else {
                    return Err(UvmError::OutOfRange);
                }
            }
        }
        // Sanity
        if begin < mmap_begin || begin + npages * PGSIZE > mmap_end || begin & (PGSIZE - 1) != 0 {
            return Err(UvmError::OutOfRange);
        }
        let end = begin + npages * PGSIZE;

        let mut prev: *mut MmapRegion = core::ptr::null_mut();
        let mut cur = *head;
        while !cur.is_null() && (*cur).begin < begin {
            prev = cur;
            cur = (*cur).next;
        }

        if !prev.is_null() {
            let prev_end = (*prev).begin + (*prev).npages as usize * PGSIZE;
            if begin < prev_end {
                return Err(UvmError::OutOfRange);
            }
        }
        if !cur.is_null() {
            if end > (*cur).begin {
                return Err(UvmError::OutOfRange);
            }
        }

        let mut merged_begin = begin;
        let mut merged_end = end;
        let mut use_prev = false;

        if !prev.is_null() {
            let prev_end = (*prev).begin + (*prev).npages as usize * PGSIZE;
            if prev_end == begin {
                merged_begin = (*prev).begin;
                use_prev = true;
            }
        }
        let mut consume_next = false;
        if !cur.is_null() {
            if end == (*cur).begin {
                merged_end = (*cur).begin + (*cur).npages as usize * PGSIZE;
                consume_next = true;
            }
        }

        if use_prev {
            // Map [prev_end, end)
            let prev_end = (*prev).begin + (*prev).npages as usize * PGSIZE;
            if end > prev_end {
                map_pages(pt, prev_end, (end - prev_end) / PGSIZE)?;
            }
            (*prev).npages = ((merged_end - merged_begin) / PGSIZE) as u32;
            if consume_next {
                let next = cur;
                let next_end = (*next).begin + (*next).npages as usize * PGSIZE;
                (*prev).npages = ((next_end - (*prev).begin) / PGSIZE) as u32;
                (*prev).next = (*next).next;
                mmap_region_free(next);
            }
            return Ok((*prev).begin);
        }

        if consume_next {
            // Map [begin, (*cur).begin)
            map_pages(pt, begin, (end - begin) / PGSIZE)?;
            let next = cur;
            (*next).begin = merged_begin;
            (*next).npages = ((merged_end - merged_begin) / PGSIZE) as u32;
            if prev.is_null() {
                *head = next;
            } else {
                (*prev).next = next;
            }
            return Ok((*next).begin);
        }

        map_pages(pt, begin, npages)?;
        let node = mmap_region_alloc();
        if node.is_null() {
            return Err(UvmError::NoMem);
        }
        (*node).begin = begin;
        (*node).npages = npages as u32;
        (*node).next = cur;
        if prev.is_null() {
            *head = node;
        } else {
            (*prev).next = node;
        }
        Ok(begin)
    }
}

pub fn uvm_munmap(
    pt: &mut PageTable,
    head: &mut *mut MmapRegion,
    begin: VirtAddr,
    len: usize,
) -> Result<(), UvmError> {
    if len == 0 {
        return Ok(());
    }
    let start = align_down(begin);
    let end = align_up(begin + len);
    if end <= start {
        return Ok(());
    }

    unsafe {
        let mut prev: *mut MmapRegion = core::ptr::null_mut();
        let mut cur = *head;
        while !cur.is_null() && (*cur).begin + (*cur).npages as usize * PGSIZE <= start {
            prev = cur;
            cur = (*cur).next;
        }
        if cur.is_null() {
            return Ok(());
        }

        let mut unmap_start = start;
        while !cur.is_null() && unmap_start < end {
            let cur_begin = (*cur).begin;
            let cur_end = cur_begin + (*cur).npages as usize * PGSIZE;
            if end <= cur_begin {
                break;
            }

            let s = core::cmp::max(unmap_start, cur_begin);
            let e = core::cmp::min(end, cur_end);
            if e > s {
                // Unmap [s, e)
                let sz = e - s;
                if !pt.unmap(s, sz, true) {
                    return Err(UvmError::MapFailed);
                }

                if s == cur_begin && e == cur_end {
                    // remove whole node
                    let next = (*cur).next;
                    if prev.is_null() {
                        *head = next;
                    } else {
                        (*prev).next = next;
                    }
                    mmap_region_free(cur);
                    cur = next;
                } else if s == cur_begin {
                    // trim front
                    (*cur).begin = e;
                    (*cur).npages = ((cur_end - e) / PGSIZE) as u32;
                    prev = cur;
                    cur = (*cur).next;
                } else if e == cur_end {
                    // trim back
                    (*cur).npages = ((s - cur_begin) / PGSIZE) as u32;
                    prev = cur;
                    cur = (*cur).next;
                } else {
                    // left [cur_begin, s), right [e, cur_end)
                    let right = mmap_region_alloc();
                    if right.is_null() {
                        return Err(UvmError::NoMem);
                    }
                    (*right).begin = e;
                    (*right).npages = ((cur_end - e) / PGSIZE) as u32;
                    (*right).next = (*cur).next;
                    (*cur).npages = ((s - cur_begin) / PGSIZE) as u32;
                    (*cur).next = right;
                    prev = cur;
                    cur = (*right).next;
                }
            } else {
                prev = cur;
                cur = (*cur).next;
            }
            unmap_start = s + (e - s);
        }
    }
    Ok(())
}

// Destroy a 3-level Sv39 page table rooted at `root_pa`.
pub fn uvm_destroy_pgtbl(root_pa: PhysAddr) {
    fn destroy_level(table_pa: usize) {
        let table = table_pa as *mut PageTable;
        for i in 0..super::PGNUM {
            let pte = unsafe { (*table).entries[i] };
            if !pte_is_valid(pte) {
                continue;
            }
            if pte_is_leaf(pte) {
                let pa = pte_to_pa(pte);
                if let Some(for_kernel) = get_region(pa) {
                    pmem_free(pa, for_kernel);
                } else {
                    // Mapping to non-pool PA (e.g., trampoline); leave it.
                }
                unsafe {
                    (*table).entries[i] = 0;
                }
            } else if pte_is_table(pte) {
                let child_pa = pte_to_pa(pte);
                destroy_level(child_pa);
                // free the child page table page (kernel pool)
                pmem_free(child_pa, true);
                unsafe {
                    (*table).entries[i] = 0;
                }
            }
        }
    }

    destroy_level(root_pa);
    // finally free root table page
    pmem_free(root_pa, true);
}

/// Deep-copy a Sv39 page table. Returns new root page table PA.
/// - For user pages: allocate new user page and copy data.
/// - For trapframe-like pages: allocate new kernel page and copy data.
/// - For trampoline-like pages: reuse the same PA, do not copy.
pub fn uvm_copy_pgtbl(src_root_pa: PhysAddr) -> Result<PhysAddr, UvmError> {
    // allocate destination root
    let dst_root = pmem_alloc(true) as usize;
    if dst_root == 0 {
        return Err(UvmError::NoMem);
    }
    unsafe {
        core::ptr::write_bytes(dst_root as *mut u8, 0, PGSIZE);
    }
    let dst_pt = unsafe { &mut *(dst_root as *mut PageTable) };

    unsafe {
        let l2 = src_root_pa as *const PageTable;
        for i in 0..super::PGNUM {
            let pte2 = (*l2).entries[i];
            if !pte_is_valid(pte2) {
                continue;
            }
            if !pte_is_table(pte2) {
                continue;
            }
            let l1_pa = pte_to_pa(pte2);
            let l1 = l1_pa as *const PageTable;
            for j in 0..super::PGNUM {
                let pte1 = (*l1).entries[j];
                if !pte_is_valid(pte1) {
                    continue;
                }
                if !pte_is_table(pte1) {
                    continue;
                }
                let l0_pa = pte_to_pa(pte1);
                let l0 = l0_pa as *const PageTable;
                for k in 0..super::PGNUM {
                    let pte0 = (*l0).entries[k];
                    if !pte_is_valid(pte0) {
                        continue;
                    }
                    if !pte_is_leaf(pte0) {
                        continue;
                    }
                    let pa = pte_to_pa(pte0);
                    let flags = pte_get_flags(pte0);
                    // compute canonical VA from indices
                    let va_raw = ((i << 30) | (j << 21) | (k << 12)) as usize;
                    let sign = (va_raw >> 38) & 1;
                    let va = if sign == 1 {
                        va_raw | (!0usize << 39)
                    } else {
                        va_raw & ((1usize << 39) - 1)
                    };

                    if (flags & PTE_U) != 0 {
                        // user page
                        let new_pa = pmem_alloc(false) as usize;
                        if new_pa == 0 {
                            return Err(UvmError::NoMem);
                        }
                        core::ptr::copy_nonoverlapping(pa as *const u8, new_pa as *mut u8, PGSIZE);
                        if !dst_pt.map(va, new_pa, PGSIZE, flags) {
                            return Err(UvmError::MapFailed);
                        }
                    } else if (flags & PTE_X) != 0 {
                        // trampoline-like
                        if !dst_pt.map(va, pa, PGSIZE, flags) {
                            return Err(UvmError::MapFailed);
                        }
                    } else {
                        // trapframe-like
                        let new_pa = pmem_alloc(true) as usize;
                        if new_pa == 0 {
                            return Err(UvmError::NoMem);
                        }
                        core::ptr::copy_nonoverlapping(pa as *const u8, new_pa as *mut u8, PGSIZE);
                        if !dst_pt.map(va, new_pa, PGSIZE, flags) {
                            return Err(UvmError::MapFailed);
                        }
                    }
                }
            }
        }
    }

    Ok(dst_root)
}
