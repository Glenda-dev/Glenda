use crate::error::Error;
use crate::hal;
use crate::hal::mem::PTEFLAGS_MASK;
use crate::hal::mem::Pte;
use crate::hal::mem::{PGNUM, PGSIZE, PT_LEVELS};
use crate::mem::addr::phys_to_virt;
use crate::mem::addr::virt_to_phys;
use crate::mem::pmem;
use crate::mem::{Perms, PhysAddr, VirtAddr};

#[repr(C, align(4096))]
#[derive(Debug, Clone, Copy)]
pub struct PageTable {
    pub entries: [Pte; PGNUM],
}

impl PageTable {
    pub const fn new() -> Self {
        PageTable { entries: [Pte::null(); PGNUM] }
    }

    pub fn paddr(&self) -> PhysAddr {
        virt_to_phys(VirtAddr::from(self as *const _ as usize))
    }

    pub fn from_addr(paddr: PhysAddr) -> &'static mut Self {
        let vaddr = phys_to_virt(paddr);
        unsafe { vaddr.as_mut::<PageTable>() }
    }

    /// 查找虚拟地址对应的 PTE 指针
    ///
    /// * `va`: 虚拟地址
    ///
    /// 返回：
    /// * `Some(pte)`: 找到对应的 PTE (可能是叶子节点，也可能是中间节点)
    /// * `None`: 遍历过程中断 (中间页表不存在)
    pub fn walk(&mut self, va: VirtAddr) -> Option<*mut Pte> {
        let mut table = self;

        // 遍历 3 级页表 (Level 2 -> Level 1 -> Level 0)
        // 最后一级 (Level 0) 的 PTE 将被返回
        for level in (1..PT_LEVELS).rev() {
            let idx = hal::mem::get_vpn_index(va, level).as_usize();
            let pte_val = table.entries[idx];

            if !pte_val.is_valid() {
                // 中间页表不存在，直接返回 None
                // 在微内核中，这意味着用户必须先 Map 一个 PageTable 到这个位置
                return None;
            }

            if pte_val.is_leaf() {
                // 遇到大页 (Huge Page)，直接返回该 PTE
                // 注意：调用者需要知道这是一个大页 PTE
                return Some(&mut table.entries[idx] as *mut Pte);
            }

            // 进入下一级页表
            let next_pa = pte_val.pa();
            let next_va = phys_to_virt(next_pa);
            table = unsafe { next_va.as_mut::<PageTable>() };
        }

        // 返回 Level 0 的 PTE
        Some(&mut table.entries[hal::mem::get_vpn_index(va, 0).as_usize()] as *mut Pte)
    }

    /// 映射内存区域 (机制)
    ///
    /// * `va`: 虚拟起始地址
    /// * `pa`: 物理起始地址
    /// * `size`: 映射大小 (字节)
    /// * `flags`: 权限标志
    ///
    /// 注意：此函数假设中间页表已经存在。如果不存在，会返回失败。
    /// 用户必须先调用 map_table 来建立中间层级。
    pub fn map(
        &mut self,
        va: VirtAddr,
        pa: PhysAddr,
        size: usize,
        flags: Perms,
    ) -> Result<(), Error> {
        log!("vm: PageTable::map: va={:?} pa={:?} size={:#x} flags={:?}", va, pa, size, flags);
        assert!(va.is_aligned(PGSIZE));
        assert!(pa.is_aligned(PGSIZE));

        // W^X Check: A page cannot be both Writable and Executable.
        if flags.contains(Perms::WRITE) && flags.contains(Perms::EXECUTE) {
            error!("vm: PageTable::map W^X violation: va={:?} flags={:?}", va, flags);
            return Err(Error::InvalidArgs);
        }

        let mut current_va = va;
        let mut current_pa = pa;
        let end_va = va + size;
        while current_va < end_va {
            let pte_ptr = if let Some(ptr) = self.walk(current_va) {
                ptr
            } else {
                error!("vm: PageTable::map failed: intermediate missing for va={:?}", current_va);
                return Err(Error::MappingFailed);
            };

            unsafe {
                let old_pte = *pte_ptr;
                // 如果已经存在映射，且不是更新权限，则报错 (防止覆盖)
                if old_pte.is_valid() && (old_pte.pa() != current_pa) {
                    error!(
                        "vm: PageTable::map failed: collision at va={:?} old_pa={:?} new_pa={:?}",
                        current_va,
                        old_pte.pa(),
                        current_pa
                    );
                    return Err(Error::AlreadyExists);
                }

                // 写入新的 PTE
                *pte_ptr = Pte::from(current_pa, flags);
            }

            current_va += PGSIZE;
            current_pa += PGSIZE;
        }
        Ok(())
    }

    /// 更新映射权限
    ///
    /// * `va`: 虚拟起始地址
    /// * `size`: 映射大小 (字节)
    /// * `flags`: 新的权限标志
    pub fn update(&mut self, va: VirtAddr, size: usize, flags: Perms) -> Result<(), Error> {
        log!("vm: PageTable::update: va={:?} size={} flags={:?}", va, size, flags);
        assert!(va.is_aligned(PGSIZE));

        // W^X Check: A page cannot be both Writable and Executable.
        if flags.contains(Perms::WRITE) && flags.contains(Perms::EXECUTE) {
            error!("vm: PageTable::update W^X violation: va={:?} flags={:?}", va, flags);
            return Err(Error::InvalidArgs);
        }

        let mut current_va = va;
        let end_va = va + size;
        while current_va < end_va {
            let pte_ptr = if let Some(ptr) = self.walk(current_va) {
                ptr
            } else {
                error!("vm: PageTable::update failed: mapping missing for va={:?}", current_va);
                return Err(Error::MappingFailed);
            };

            unsafe {
                let mut pte = *pte_ptr;
                if !pte.is_valid() {
                    error!("vm: PageTable::update failed: invalid pte at va={:?}", current_va);
                    return Err(Error::MappingFailed);
                }
                pte.set_flags(flags | Perms::VALID);
                *pte_ptr = pte;
            }
            current_va += PGSIZE;
        }
        Ok(())
    }

    /// 解除映射
    ///
    /// * `va`: 虚拟地址
    /// * `size`: 大小
    ///
    /// 注意：不负责释放物理内存。物理内存由 Capability 系统管理。
    pub fn unmap(&mut self, va: VirtAddr, size: usize) -> Result<(), Error> {
        log!("vm: PageTable::unmap: va={:?} size={}", va, size);
        let start_va = va.align_down(PGSIZE);
        let end_va = (va + size).align_up(PGSIZE);
        let mut current_va = start_va;

        while current_va < end_va {
            // 如果 walk 返回 None，说明中间页表都不存在，自然也不存在映射，忽略即可
            if let Some(pte_ptr) = self.walk(current_va) {
                unsafe {
                    // 无论之前是否有效，直接清零
                    *pte_ptr = Pte::null();
                }
            }
            current_va += PGSIZE;
        }
        Ok(())
    }

    /// 映射中间页表 (Map PageTable)
    ///
    /// * `va`: 目标虚拟地址范围的起始
    /// * `table_pa`: 中间页表的物理地址
    /// * `level`: 目标层级 (例如 1 代表映射一个 2 MB 范围的页目录)
    pub fn map_table(
        &mut self,
        va: VirtAddr,
        table_pa: PhysAddr,
        level: usize,
    ) -> Result<(), Error> {
        log!("vm: PageTable::map_table: va={:?} table_pa={:?} level={}", va, table_pa, level);
        if level == 0 || level >= PT_LEVELS {
            error!("vm: PageTable::map_table failed: invalid level {}", level);
            return Err(Error::InvalidArgs); // 无效层级
        }

        // 遍历到目标层级的上一级
        let mut table = self;
        for l in ((level + 1)..PT_LEVELS).rev() {
            let idx = hal::mem::get_vpn_index(va, l).as_usize();
            let pte_val = table.entries[idx];
            if !pte_val.is_valid() || pte_val.is_leaf() {
                error!(
                    "vm: PageTable::map_table failed: parent missing/huge at level {} va={:?}",
                    l, va
                );
                return Err(Error::MappingFailed); // 父级页表不存在或已被大页占用
            }
            let next_pa = pte_val.pa();
            let next_va = phys_to_virt(next_pa);
            table = unsafe { next_va.as_mut::<PageTable>() };
        }

        // 在目标层级写入 PTE，指向新的页表
        let idx = hal::mem::get_vpn_index(va, level).as_usize();
        let pte_ptr = &mut table.entries[idx];

        if pte_ptr.is_valid() {
            error!("vm: PageTable::map_table failed: slot occupied at level {} va={:?}", level, va);
            return Err(Error::AlreadyExists); // 槽位已被占用
        }
        // 注意：中间页表的 PTE 没有 R/W/X 权限，只有 V 位
        *pte_ptr = Pte::from(table_pa, Perms::VALID);

        Ok(())
    }

    /// 解锁中间页表 (Unmap PageTable)
    ///
    /// * `va`: 虚拟地址
    /// * `level`: 目标层级
    pub fn unmap_table(&mut self, va: VirtAddr, level: usize) -> Result<(), Error> {
        log!("vm: PageTable::unmap_table: va={:?} level={}", va, level);
        if level == 0 || level >= PT_LEVELS {
            error!("vm: PageTable::unmap_table failed: invalid level {}", level);
            return Err(Error::InvalidArgs);
        }

        let mut table = self;
        for l in ((level + 1)..PT_LEVELS).rev() {
            let idx = hal::mem::get_vpn_index(va, l).as_usize();
            let pte_val = table.entries[idx];
            if !pte_val.is_valid() || pte_val.is_leaf() {
                // 如果路径不存在，说明本来就是干净的，返回成功
                return Ok(());
            }
            let next_pa = pte_val.pa();
            let next_va = phys_to_virt(next_pa);
            table = unsafe { next_va.as_mut::<PageTable>() };
        }

        let idx = hal::mem::get_vpn_index(va, level).as_usize();
        let pte_ptr = &mut table.entries[idx];

        if !pte_ptr.is_valid() || pte_ptr.is_leaf() {
            // 如果目标槽位已经是空的，或者原本就是个叶子节点（Frame），返回成功
            *pte_ptr = Pte::null();
            return Ok(());
        }

        // 获取被卸载页表的虚拟地址并清零
        let pt_pa = pte_ptr.pa();
        let pt_va = phys_to_virt(pt_pa);
        unsafe {
            core::ptr::write_bytes(pt_va.as_mut_ptr::<u8>(), 0, PGSIZE);
        }

        *pte_ptr = Pte::null();

        Ok(())
    }

    /// 映射并自动分配中间页表 (辅助函数)
    ///
    /// 如果中间页表不存在，则分配新的页表页。
    /// 需要调用 pmem::alloc_pagetable_cap 来分配页表页。
    pub fn map_with_alloc(&mut self, va: VirtAddr, pa: PhysAddr, size: usize, flags: Perms) {
        log!(
            "vm: PageTable::map_with_alloc: va={:?} pa={:?} size={:#x} flags={}",
            va,
            pa,
            size,
            flags
        );
        let start = va;
        let end = va + size;

        let mut va = start;
        let mut pa = pa.align_down(PGSIZE);
        'map_loop: while va < end {
            // 手动遍历页表，如果中间层级缺失则分配
            let mut table = self as *mut PageTable;
            for level in (1..PT_LEVELS).rev() {
                let idx = hal::mem::get_vpn_index(va, level).as_usize();
                let entry = unsafe { &mut (*table).entries[idx] };

                if !entry.is_valid() {
                    let frame_pa = pmem::alloc_page().expect("Failed to alloc page for page table");

                    // 建立中间层级映射 (V=1, 无 R/W/X)
                    *entry = Pte::from(frame_pa, Perms::VALID);
                }

                if entry.is_leaf() {
                    // Check if existing huge page covers our range compatible
                    let page_size = 1usize << (12 + level * 9);
                    let huge_page_start_pa = pa.align_down(page_size);
                    let entry_pa = entry.pa();

                    if entry_pa == huge_page_start_pa {
                        // Found compatible huge page. Skip over it.
                        let next_boundary = va.align_down(page_size) + page_size;
                        // Calculate how much we can skip
                        let dist_to_boundary = next_boundary.as_usize() - va.as_usize();
                        let dist_to_end = end.as_usize() - va.as_usize();
                        let step = core::cmp::min(dist_to_boundary, dist_to_end);

                        va += step;
                        pa += step;
                        continue 'map_loop;
                    }

                    panic!(
                        "map_with_alloc: Huge page support (splitting) not implemented. Level={}, va={:?}, entry={:?}",
                        level, va, entry
                    );
                }

                // 进入下一级
                let next_pa = entry.pa();
                // 在恒等映射模式下，物理地址即为内核虚拟地址
                let next_va = phys_to_virt(next_pa);
                table = unsafe { next_va.as_mut::<PageTable>() };
            }

            // 设置最后一级 PTE
            let idx = hal::mem::get_vpn_index(va, 0).as_usize();
            unsafe {
                (*table).entries[idx] = Pte::from(pa, flags | Perms::VALID);
            }
            va += PGSIZE;
            pa += PGSIZE;
        }
    }
    pub fn debug_print(&self) {
        let pgtbl_2 = self as *const PageTable as usize;
        printk!("L2 PT @ {:#x}\n", pgtbl_2);

        for i in 0..PGNUM {
            let pte2 = self.entries[i];
            if !pte2.is_valid() {
                continue;
            }
            if pte2.is_leaf() {
                printk!("ASSERT: L2 entry is leaf (Huge Page), i={}\n", i);
                continue;
            }

            let pgtbl_1_pa = pte2.pa();
            let pgtbl_1_va = phys_to_virt(pgtbl_1_pa);
            printk!(".. L1[{}] pa={:#x}\n", i, pgtbl_1_pa.as_usize());

            let pgtbl_1 = unsafe { pgtbl_1_va.as_ref::<PageTable>() };
            for j in 0..PGNUM {
                let pte1 = pgtbl_1.entries[j];
                if !pte1.is_valid() {
                    continue;
                }
                if pte1.is_leaf() {
                    printk!("ASSERT: L1 entry is leaf (Large Page), j={}\n", j);
                    continue;
                }

                let pgtbl_0_pa = pte1.pa();
                let pgtbl_0_va = phys_to_virt(pgtbl_0_pa);
                printk!(".. .. L0[{}] pa={:#x}\n", j, pgtbl_0_pa.as_usize());

                let pgtbl_0 = unsafe { pgtbl_0_va.as_ref::<PageTable>() };
                for k in 0..PGNUM {
                    let pte0 = pgtbl_0.entries[k];
                    if !pte0.is_valid() {
                        continue;
                    }
                    if !pte0.is_leaf() {
                        printk!("ASSERT: L0 entry not leaf, k={}\n", k);
                        continue;
                    }

                    let pa = pte0.pa();
                    let va_raw = ((i << 30) | (j << 21) | (k << 12)) as usize;
                    // let va = sv39_canon(va_raw);
                    let va = va_raw; // Simplified for now
                    let flags = pte0.as_usize() & PTEFLAGS_MASK;

                    printk!(
                        ".. .. .. page {} VA={:#x} -> PA={:#x} flags={:#x}\n",
                        k,
                        va,
                        pa.as_usize(),
                        flags
                    );
                }
            }
        }
    }
}
