use core::cmp::min;

use super::addr::{align_down, align_up, page_offset};
use super::pagetable::PageTable;
use super::pte::{self, PTE_A, PTE_D, PTE_R, PTE_U, PTE_W, pte_to_pa};
use super::{PGSIZE, VirtAddr};
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
