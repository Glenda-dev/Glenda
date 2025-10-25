use super::addr::{PhysAddr, VirtAddr, align_down, align_up, vpn};
use super::pmem::{get_region, pmem_alloc, pmem_free};
use super::pte::{PTE_V, Pte, pa_to_pte, pte_is_leaf, pte_is_valid, pte_to_pa};
use super::{PGNUM, PGSIZE, VA_MAX};
use core::cell::UnsafeCell;
use spin::Mutex;

// align 4096, 防止 sfence.vma 直接 TRAP
#[repr(C, align(4096))]
#[derive(Clone, Copy)]
pub struct PageTable {
    pub entries: [Pte; PGNUM],
}

impl PageTable {
    pub const fn new() -> Self {
        PageTable { entries: [0; PGNUM] }
    }
    // walk: 只支持 4KB 页；中间层遇到 leaf(=大页) 视为错误返回 None
    pub fn walk(&mut self, va: VirtAddr, alloc: bool) -> Option<*mut Pte> {
        if va >= VA_MAX {
            return None;
        }
        let mut table: *mut PageTable = self as *mut PageTable;
        // 访问顺序：L2 -> L1，最后返回 L0 的 PTE 指针
        for level in (1..3).rev() {
            let idx = vpn(va)[level];
            let pte_ref = unsafe { &mut (*table).entries[idx] };
            if pte_is_valid(*pte_ref) {
                if pte_is_leaf(*pte_ref) {
                    // 不支持大页
                    return None;
                }
                // 进入下一层表
                table = pte_to_pa(*pte_ref) as *mut PageTable;
            } else {
                if !alloc {
                    return None;
                }
                let new_table = pmem_alloc(true) as *mut PageTable;
                if new_table.is_null() {
                    return None;
                }
                unsafe {
                    core::ptr::write_bytes(new_table as *mut u8, 0, PGSIZE);
                    *pte_ref = pa_to_pte(new_table as usize, PTE_V); // 仅 V 置位表示中间层
                }
                table = new_table;
            }
        }
        Some(unsafe { &mut (*table).entries[vpn(va)[0]] as *mut Pte })
    }

    // lookup: 只读查询 PTE；不分配、不修改
    pub fn lookup(&self, va: VirtAddr) -> Option<*mut Pte> {
        if va >= VA_MAX {
            return None;
        }
        let mut table: *const PageTable = self as *const PageTable;
        for level in (1..3).rev() {
            let idx = vpn(va)[level];
            let pte = unsafe { (*table).entries[idx] };
            if pte_is_valid(pte) {
                if pte_is_leaf(pte) {
                    return None;
                }
                table = pte_to_pa(pte) as *const PageTable;
            } else {
                return None;
            }
        }
        Some(unsafe { &((*table).entries[vpn(va)[0]]) as *const Pte as *mut Pte })
    }

    pub fn map(&mut self, va: VirtAddr, pa: PhysAddr, len: usize, flags: usize) -> bool {
        if len == 0 {
            return false;
        }
        let start = align_down(va);
        let end = align_up(va + len);
        let mut a = start;
        let mut pa_cur = align_down(pa);
        let last = end - PGSIZE;
        while a <= last {
            let pte = match self.walk(a, true) {
                Some(p) => p,
                None => return false,
            };
            let cur = unsafe { *pte };
            if pte_is_valid(cur) {
                // 已存在映射：允许对同一物理页更新权限；若物理页不同则视为冲突
                if !pte_is_leaf(cur) || pte_to_pa(cur) != pa_cur {
                    return false; // 冲突或结构错误
                }
                unsafe {
                    *pte = pa_to_pte(pa_cur, flags | PTE_V);
                }
            } else {
                unsafe {
                    *pte = pa_to_pte(pa_cur, flags | PTE_V);
                }
            }
            if a == last {
                break;
            }
            a += PGSIZE;
            pa_cur += PGSIZE;
        }
        true
    }

    pub fn unmap(&mut self, va: VirtAddr, len: usize, free: bool) -> bool {
        if len == 0 {
            return false;
        }
        let start = align_down(va);
        let end = align_up(va + len);
        let mut a = start;
        let last = end - PGSIZE;
        while a <= last {
            let pte = match self.lookup(a) {
                Some(p) => p,
                None => return false,
            };
            let old = unsafe { *pte };
            if !pte_is_valid(old) || !pte_is_leaf(old) {
                return false;
            }
            let pa = pte_to_pa(old);
            if free {
                match get_region(pa) {
                    Some(for_kernel) => pmem_free(pa, for_kernel),
                    None => panic!("vm_unmappages: PA {:#x} out of bounds", pa),
                };
            }
            unsafe { *pte = 0 }; // 清除映射
            if a == last {
                break;
            }
            a += PGSIZE;
        }
        true
    }
}

unsafe impl Sync for PageTable {}

pub struct PageTableCell {
    pub(super) cell: UnsafeCell<PageTable>,
    lock: Mutex<()>,
}
unsafe impl Sync for PageTableCell {}

impl PageTableCell {
    pub const fn new() -> Self {
        Self { cell: UnsafeCell::new(PageTable::new()), lock: Mutex::new(()) }
    }

    #[inline]
    pub fn with_mut<T>(&self, f: impl FnOnce(&mut PageTable) -> T) -> T {
        let _g = self.lock.lock();
        unsafe { f(&mut *self.cell.get()) }
    }
}
