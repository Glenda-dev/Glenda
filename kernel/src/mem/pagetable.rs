use super::PhysFrame;
use super::addr;
use super::addr::{align_down, align_up, vpn};
use super::pte::{self, Pte, pa_to_pte, pte_to_pa};
use super::{PGNUM, PGSIZE, PhysAddr, VA_MAX, VirtAddr};

// align 4096 to avoid SFENCE.VMA issues with unaligned root pointers
#[repr(C, align(4096))]
#[derive(Clone, Copy)]
pub struct PageTable {
    pub entries: [Pte; PGNUM],
}

impl PageTable {
    /// 创建一个新的空页表 (仅用于初始化)
    pub const fn new() -> Self {
        PageTable { entries: [0; PGNUM] }
    }

    /// 从物理帧获取页表的可变引用
    pub fn from_frame(frame: &PhysFrame) -> &'static mut Self {
        let vaddr = frame.va();
        unsafe { &mut *(vaddr as *mut PageTable) }
    }

    /// 查找虚拟地址对应的 PTE 指针
    ///
    /// * `va`: 虚拟地址
    /// * `alloc`: 必须为 false。微内核中，缺页必须由用户处理，内核不自动分配中间页表。
    ///
    /// 返回：
    /// * `Some(pte)`: 找到对应的 PTE (可能是叶子节点，也可能是中间节点)
    /// * `None`: 遍历过程中断 (中间页表不存在)
    pub fn walk(&mut self, va: VirtAddr) -> Option<*mut Pte> {
        if va >= VA_MAX {
            return None;
        }

        let mut table = self;

        // 遍历 3 级页表 (Level 2 -> Level 1 -> Level 0)
        // 最后一级 (Level 0) 的 PTE 将被返回
        for level in (1..3).rev() {
            let idx = vpn(va)[level];
            let pte_val = table.entries[idx];

            if !pte::is_valid(pte_val) {
                // 中间页表不存在，直接返回 None
                // 在微内核中，这意味着用户必须先 Map 一个 PageTable 到这个位置
                return None;
            }

            if pte::is_leaf(pte_val) {
                // 遇到大页 (Huge Page)，直接返回该 PTE
                // 注意：调用者需要知道这是一个大页 PTE
                return Some(&mut table.entries[idx] as *mut Pte);
            }

            // 进入下一级页表
            let next_pa = pte_to_pa(pte_val);
            let next_va = addr::phys_to_virt(next_pa);
            table = unsafe { &mut *(next_va as *mut PageTable) };
        }

        // 返回 Level 0 的 PTE
        Some(&mut table.entries[vpn(va)[0]] as *mut Pte)
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
    pub fn map(&mut self, va: VirtAddr, pa: PhysAddr, size: usize, flags: usize) -> Result<(), ()> {
        let start_va = align_down(va);
        let end_va = align_up(va + size);

        let mut current_va = start_va;
        let mut current_pa = align_down(pa);

        while current_va < end_va {
            let pte_ptr = self.walk(current_va).ok_or(())?;

            unsafe {
                let old_pte = *pte_ptr;
                // 如果已经存在映射，且不是更新权限，则报错 (防止覆盖)
                if pte::is_valid(old_pte) && (pte_to_pa(old_pte) != current_pa) {
                    return Err(());
                }

                // 写入新的 PTE
                *pte_ptr = pa_to_pte(current_pa, flags | pte::PTE_V);
            }

            current_va += PGSIZE;
            current_pa += PGSIZE;
        }
        Ok(())
    }

    /// 解除映射
    ///
    /// * `va`: 虚拟地址
    /// * `size`: 大小
    ///
    /// 注意：不负责释放物理内存。物理内存由 Capability 系统管理。
    pub fn unmap(&mut self, va: VirtAddr, size: usize) -> Result<(), ()> {
        let start_va = align_down(va);
        let end_va = align_up(va + size);
        let mut current_va = start_va;

        while current_va < end_va {
            // 如果 walk 返回 None，说明中间页表都不存在，自然也不存在映射，忽略即可
            if let Some(pte_ptr) = self.walk(current_va) {
                unsafe {
                    // 无论之前是否有效，直接清零
                    *pte_ptr = 0;
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
    /// * `level`: 目标层级 (例如 1 代表映射一个 2MB 范围的页目录)
    pub fn map_table(&mut self, va: VirtAddr, table_pa: PhysAddr, level: usize) -> Result<(), ()> {
        if level == 0 || level > 2 {
            return Err(()); // 无效层级
        }

        // 遍历到目标层级的上一级
        let mut table = self;
        for l in ((level + 1)..3).rev() {
            let idx = vpn(va)[l];
            let pte_val = table.entries[idx];
            if !pte::is_valid(pte_val) || pte::is_leaf(pte_val) {
                return Err(()); // 父级页表不存在或已被大页占用
            }
            let next_pa = pte_to_pa(pte_val);
            let next_va = addr::phys_to_virt(next_pa);
            table = unsafe { &mut *(next_va as *mut PageTable) };
        }

        // 在目标层级写入 PTE，指向新的页表
        let idx = vpn(va)[level];
        let pte_ptr = &mut table.entries[idx];

        if pte::is_valid(*pte_ptr) {
            return Err(()); // 槽位已被占用
        }
        // 注意：中间页表的 PTE 没有 R/W/X 权限，只有 V 位
        *pte_ptr = pa_to_pte(table_pa, pte::PTE_V);

        Ok(())
    }

    pub fn map_kernel(&mut self) {
        if let Some(kpt) = crate::mem::vm::KERNEL_PAGE_TABLE.get() {
            // 拷贝顶级页表的所有条目
            // 在恒等映射模式下，这包含了内核代码、数据以及所有物理内存的映射
            self.entries.copy_from_slice(&kpt.entries);
        }
    }
}
