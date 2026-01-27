use crate::mem::{PPN, PhysAddr, VPN, VirtAddr};
use core::fmt::Debug;
use core::ops::{Add, BitAnd, BitAndAssign, BitOr, BitOrAssign, Sub};

/// 页大小
pub const PGSIZE: usize = 0;
/// 最大虚拟地址
pub const VA_MAX: usize = 0;
/// HHDM: 物理内存线性映射的虚拟基地址 (例如 0xFFFF_8000_0000_0000)
pub const PHYS_MAP_BASE: usize = 0;
/// 内核代码段加载的虚拟基地址 (例如 0xFFFF_FFFF_8020_0000)
pub const KERNEL_BASE: usize = 0;
/// 页表层级数
pub const PT_LEVELS: usize = 0;
/// 每级页表的页表项数量
pub const PGNUM: usize = 0;
/// 页表项标志位掩码
pub const PTEFLAGS_MASK: usize = 0;

/// 页表项类型
#[derive(Clone, Copy, Debug)]
pub struct Pte;
/// 页表项标志
#[derive(Clone, Copy)]
pub struct PteFlags;
/// 页表结构体
pub struct PageTable;

/// 刷新 TLB
///
/// `vaddr`: 可选的虚拟地址，如果为 None 则刷新整个 TLB
fn flush_tlb(vaddr: Option<VirtAddr>) {
    unimplemented!()
}

/// 激活页表
///
/// 将根页表物理地址写入控制寄存器 (如 satp, ttbr0, pgd)
unsafe fn activate_pagetable(root_paddr: PhysAddr) {
    unimplemented!()
}

/// 取消激活页表
///
/// 将控制寄存器 (如 satp, ttbr0, pgd) 置零
unsafe fn deactivate_pagetable() {
    unimplemented!()
}

/// 页表权限
pub mod perms {
    pub const VALID: usize = 0;
    pub const READ: usize = 0;
    pub const WRITE: usize = 0;
    pub const EXECUTE: usize = 0;
    pub const USER: usize = 0;
    pub const GLOBAL: usize = 0;
    pub const ACCESSED: usize = 0;
    pub const DIRTY: usize = 0;
}

impl Pte {
    pub const fn null() -> Self {
        unimplemented!()
    }
    pub const fn from(pa: PhysAddr, flags: PteFlags) -> Self {
        unimplemented!()
    }
    pub const fn as_usize(&self) -> usize {
        unimplemented!()
    }
    pub const fn get_ppn(&self) -> PPN {
        unimplemented!()
    }
    pub const fn set_ppn(&mut self, ppn: PPN) {
        unimplemented!()
    }
    pub const fn get_flags(&self) -> PteFlags {
        unimplemented!()
    }
    pub const fn set_flags(&mut self, flags: PteFlags) {
        unimplemented!()
    }
    pub const fn is_valid(&self) -> bool {
        unimplemented!()
    }
    pub const fn is_leaf(&self) -> bool {
        unimplemented!()
    }
    pub const fn is_table(&self) -> bool {
        unimplemented!()
    }
    pub const fn pa(&self) -> PhysAddr {
        unimplemented!()
    }
}

impl PteFlags {
    pub const fn null() -> Self {
        unimplemented!()
    }
    pub const fn from(value: usize) -> Self {
        unimplemented!()
    }
    pub const fn as_usize(&self) -> usize {
        unimplemented!()
    }
}

impl BitOr<usize> for PteFlags {
    type Output = PteFlags;
    fn bitor(self, flags: usize) -> PteFlags {
        unimplemented!()
    }
}
impl BitOrAssign<usize> for PteFlags {
    fn bitor_assign(&mut self, rhs: usize) {
        unimplemented!()
    }
}

impl BitAnd<usize> for PteFlags {
    type Output = PteFlags;
    fn bitand(self, flags: usize) -> PteFlags {
        unimplemented!()
    }
}
impl BitAndAssign<usize> for PteFlags {
    fn bitand_assign(&mut self, rhs: usize) {
        unimplemented!()
    }
}

impl Add for PteFlags {
    type Output = PteFlags;
    fn add(self, rhs: PteFlags) -> PteFlags {
        unimplemented!()
    }
}

impl Sub for PteFlags {
    type Output = PteFlags;
    fn sub(self, rhs: PteFlags) -> PteFlags {
        unimplemented!()
    }
}

impl Debug for PteFlags {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        unimplemented!()
    }
}
