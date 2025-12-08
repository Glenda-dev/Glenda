use core::cmp::min;

use super::addr::{align_down, align_up, page_offset};
use super::mmap::{self, MmapRegion};
use super::pagetable::PageTable;
use super::pmem;
use super::pte::{self, PTE_A, PTE_D, PTE_R, PTE_U, PTE_W, pte_to_pa};
use super::{MMAP_BEGIN, PGSIZE, VirtAddr};
use core::cmp;
use core::ptr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyError {
    NotMapped,
    NoPerm,
    Fault,
    TooLong,
}

pub fn copyin(pt: &PageTable, dst: &mut [u8], mut src_va: VirtAddr) -> Result<(), CopyError> {
    let mut copied = 0usize;
    while copied < dst.len() {
        let va = src_va;
        let pte_ptr = match pt.lookup(align_down(va)) {
            Some(p) => p,
            None => return Err(CopyError::NotMapped),
        };
        let pte = unsafe { *pte_ptr };
        if !pte::is_valid(pte) || !pte::is_leaf(pte) {
            return Err(CopyError::NotMapped);
        };
        let flags = pte::get_flags(pte);
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
            ptr::copy_nonoverlapping((pa + off) as *const u8, dst.as_mut_ptr().add(copied), n);
        }
        copied += n;
        src_va += n;
    }
    Ok(())
}

pub fn copyout(pt: &PageTable, mut dst_va: VirtAddr, src: &[u8]) -> Result<(), CopyError> {
    let mut copied = 0usize;
    while copied < src.len() {
        let va = dst_va;
        let pte_ptr = match pt.lookup(align_down(va)) {
            Some(p) => p,
            None => return Err(CopyError::NotMapped),
        };
        let pte = unsafe { *pte_ptr };
        if !pte::is_valid(pte) || !pte::is_leaf(pte) {
            return Err(CopyError::NotMapped);
        };
        let flags = pte::get_flags(pte);
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

pub fn copyin_str(
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
        if !pte::is_valid(pte) || !pte::is_leaf(pte) {
            return Err(CopyError::NotMapped);
        };
        let flags = pte::get_flags(pte);
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
pub fn heap_grow(pt: &mut PageTable, old_top: VirtAddr, new_top: VirtAddr) -> Result<(), UvmError> {
    if new_top > MMAP_BEGIN {
        return Err(UvmError::OutOfRange);
    }
    let mut a = align_up(old_top);
    let last = align_up(new_top);
    while a < last {
        let pa = pmem::alloc(false) as usize;
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
pub fn heap_ungrow(
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

pub fn ustack_grow(
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
                if pte::is_valid(unsafe { *pte_ptr }) {
                    continue;
                }
            }
            let pa = pmem::alloc(false) as usize;
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
        let pa = pmem::alloc(false) as usize;
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

pub fn mmap(
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

        let mut prev: *mut MmapRegion = ptr::null_mut();
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
                mmap::region_free(next);
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
        let node = mmap::region_alloc();
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

pub fn munmap(
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
        let mut prev: *mut MmapRegion = ptr::null_mut();
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

            let s = cmp::max(unmap_start, cur_begin);
            let e = cmp::min(end, cur_end);
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
                    mmap::region_free(cur);
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
                    let right = mmap::region_alloc();
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
