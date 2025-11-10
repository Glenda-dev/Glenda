use super::addr::{align_down, align_up, vpn};
use super::pmem::{self, get_region};
use super::pte::{self, PTE_U, PTE_V, PTE_X, Pte, pa_to_pte, pte_to_pa};
use super::uvm::UvmError;
use super::{PGNUM, PGSIZE, PhysAddr, VA_MAX, VirtAddr};
use core::cell::UnsafeCell;
use core::ptr;
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
            if pte::is_valid(*pte_ref) {
                if pte::is_leaf(*pte_ref) {
                    // 不支持大页
                    return None;
                }
                // 进入下一层表
                table = pte_to_pa(*pte_ref) as *mut PageTable;
            } else {
                if !alloc {
                    return None;
                }
                let new_table = pmem::alloc(true) as *mut PageTable;
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
            if pte::is_valid(pte) {
                if pte::is_leaf(pte) {
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
            if pte::is_valid(cur) {
                // 已存在映射：允许对同一物理页更新权限；若物理页不同则视为冲突
                if !pte::is_leaf(cur) || pte_to_pa(cur) != pa_cur {
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
            if !pte::is_valid(old) || !pte::is_leaf(old) {
                return false;
            }
            let pa = pte_to_pa(old);
            if free {
                match get_region(pa) {
                    Some(for_kernel) => pmem::free(pa, for_kernel),
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
    #[cfg(debug_assertions)]
    pub fn print(&self) {
        use crate::printk;
        #[inline(always)]
        fn pa_in_any_region(pa: usize) -> bool {
            let k = super::pmem::kernel_region_info();
            let u = super::pmem::user_region_info();
            (pa >= k.begin && pa < k.end) || (pa >= u.begin && pa < u.end)
        }

        #[inline(always)]
        fn sv39_canon(va: usize) -> usize {
            // sign-extend bit 38
            let sign = (va >> 38) & 1;
            if sign == 1 { va | (!0usize << 39) } else { va & ((1usize << 39) - 1) }
        }

        let pgtbl_2 = self as *const PageTable as usize;
        printk!("L2 PT @ 0x{:x}", pgtbl_2);

        for i in 0..PGNUM {
            let pte2 = unsafe { (*(pgtbl_2 as *const PageTable)).entries[i] };
            if !pte::is_valid(pte2) {
                continue;
            }
            if !pte::is_table(pte2) {
                printk!("ASSERT: L2 entry is not table, i={}", i);
                return;
            }

            let pgtbl_1_pa = pte_to_pa(pte2);
            if (pgtbl_1_pa & (PGSIZE - 1)) != 0 {
                printk!("ASSERT: L1 pa not page-aligned: 0x{:x}", pgtbl_1_pa);
                return;
            }
            if !pa_in_any_region(pgtbl_1_pa) {
                printk!("ASSERT: L1 pa out of region: 0x{:x}", pgtbl_1_pa);
                return;
            }

            printk!(".. L1[{}] pa=0x{:x}", i, pgtbl_1_pa);

            let pgtbl_1 = pgtbl_1_pa as *const PageTable;
            for j in 0..PGNUM {
                let pte1 = unsafe { (*pgtbl_1).entries[j] };
                if !pte::is_valid(pte1) {
                    continue;
                }
                if !pte::is_table(pte1) {
                    printk!("ASSERT: L1 entry is not table, j={}", j);
                    return;
                }

                let pgtbl_0_pa = pte_to_pa(pte1);
                if (pgtbl_0_pa & (PGSIZE - 1)) != 0 {
                    printk!("ASSERT: L0 pa not page-aligned: 0x{:x}", pgtbl_0_pa);
                    return;
                }
                if !pa_in_any_region(pgtbl_0_pa) {
                    printk!("ASSERT: L0 pa out of region: 0x{:x}", pgtbl_0_pa);
                    return;
                }

                printk!(".. .. L0[{}] pa=0x{:x}", j, pgtbl_0_pa);

                let pgtbl_0 = pgtbl_0_pa as *const PageTable;
                for k in 0..PGNUM {
                    let pte0 = unsafe { (*pgtbl_0).entries[k] };
                    if !pte::is_valid(pte0) {
                        continue;
                    }
                    if !pte::is_leaf(pte0) {
                        printk!("ASSERT: L0 entry not leaf, k={}", k);
                        return;
                    }

                    let pa = pte_to_pa(pte0);
                    let va_raw = ((i << 30) | (j << 21) | (k << 12)) as usize;
                    let va = sv39_canon(va_raw);
                    let flags = pte::get_flags(pte0);

                    printk!(
                        ".. .. .. page {} VA=0x{:x} -> PA=0x{:x} flags=0x{:x}",
                        k,
                        va,
                        pa,
                        flags
                    );
                }
            }
        }
    }
    // Destroy a 3-level Sv39 page table rooted at `root_pa`.
    pub fn destroy(self) {
        fn destroy_level(table_pa: usize) {
            let table = table_pa as *mut PageTable;
            for i in 0..super::PGNUM {
                let pte = unsafe { (*table).entries[i] };
                if !pte::is_valid(pte) {
                    continue;
                }
                if pte::is_leaf(pte) {
                    let pa = pte_to_pa(pte);
                    if let Some(for_kernel) = pmem::get_region(pa) {
                        pmem::free(pa, for_kernel);
                    } else {
                        // Mapping to non-pool PA (e.g., trampoline); leave it.
                    }
                    unsafe {
                        (*table).entries[i] = 0;
                    }
                } else if pte::is_table(pte) {
                    let child_pa = pte_to_pa(pte);
                    destroy_level(child_pa);
                    // free the child page table page (kernel pool)
                    pmem::free(child_pa, true);
                    unsafe {
                        (*table).entries[i] = 0;
                    }
                }
            }
        }
        let root_pa = &self as *const PageTable as usize;
        destroy_level(root_pa);
        // finally free root table page
        pmem::free(root_pa, true);
    }

    /// Deep-copy a Sv39 page table. Returns new root page table PA.
    /// - For user pages: allocate new user page and copy data.
    /// - For trapframe-like pages: allocate new kernel page and copy data.
    /// - For trampoline-like pages: reuse the same PA, do not copy.
    pub fn copy(self) -> Result<PhysAddr, UvmError> {
        let src_root_pa = &self as *const PageTable as usize;
        // allocate destination root
        let dst_root = pmem::alloc(true) as usize;
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
                if !pte::is_valid(pte2) {
                    continue;
                }
                if !pte::is_table(pte2) {
                    continue;
                }
                let l1_pa = pte_to_pa(pte2);
                let l1 = l1_pa as *const PageTable;
                for j in 0..super::PGNUM {
                    let pte1 = (*l1).entries[j];
                    if !pte::is_valid(pte1) {
                        continue;
                    }
                    if !pte::is_table(pte1) {
                        continue;
                    }
                    let l0_pa = pte_to_pa(pte1);
                    let l0 = l0_pa as *const PageTable;
                    for k in 0..super::PGNUM {
                        let pte0 = (*l0).entries[k];
                        if !pte::is_valid(pte0) {
                            continue;
                        }
                        if !pte::is_leaf(pte0) {
                            continue;
                        }
                        let pa = pte_to_pa(pte0);
                        let flags = pte::get_flags(pte0);
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
                            let new_pa = pmem::alloc(false) as usize;
                            if new_pa == 0 {
                                return Err(UvmError::NoMem);
                            }
                            ptr::copy_nonoverlapping(pa as *const u8, new_pa as *mut u8, PGSIZE);
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
                            let new_pa = pmem::alloc(true) as usize;
                            if new_pa == 0 {
                                return Err(UvmError::NoMem);
                            }
                            ptr::copy_nonoverlapping(pa as *const u8, new_pa as *mut u8, PGSIZE);
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
}
