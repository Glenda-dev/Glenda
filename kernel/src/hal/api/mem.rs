use crate::mem::{PPN, PageTable, Perms, PhysAddr, VPN, VirtAddr};

/// 页大小
pub const PGSIZE: usize = 4096;
/// 最大虚拟地址
pub const VA_MAX: usize = 0xFFFFFFFFFFFFFFFF;
/// 页表层级数
pub const PT_LEVELS: usize = 1;
/// 每级页表的页表项数量
pub const PGNUM: usize = 1;
/// 地址空间标识符的最大值
pub const MAX_ASID: usize = 1 << 16;
/// 地址空间标识符掩码
pub const ASID_MASK: usize = MAX_ASID - 1;
/// 内核栈大小
pub const KSTACK_PAGES: usize = 1;
/// 用户地址
pub const USER_VA: usize = 0x400000;
/// 页表项类型
#[derive(Clone, Copy, Debug)]
pub struct Pte;

/// 刷新 TLB
///
/// `vaddr`: 可选的虚拟地址，如果为 None 则刷新整个 TLB
pub fn flush_tlb(vaddr: Option<VirtAddr>) {
    unimplemented!()
}

/// 激活页表
///
/// 将根页表物理地址写入控制寄存器 (如 satp, ttbr0, pgd)
pub unsafe fn activate_vspace(register: usize) {
    unimplemented!()
}

/// 取消激活页表
///
/// 将控制寄存器 (如 satp, ttbr0, pgd) 置零
pub unsafe fn deactivate_vspace() {
    unimplemented!()
}

/// 获取 MMU 寄存器值
///
/// `addr`: 根页表物理地址
/// `asid`: 地址空间标识符
pub fn get_mmu_register(addr: PhysAddr, asid: usize) -> usize {
    unimplemented!()
}

/// 获取页表项
///
/// `va`: 虚拟地址
/// `level`: 页表层级 (0 为最低层)
pub fn get_vpn_index(va: VirtAddr, level: usize) -> VPN {
    unimplemented!()
}

/// 设置内核页表
pub fn kpt_setup(kpt: &mut PageTable) {
    unimplemented!()
}

/// 设置页表
pub fn pt_setup(pt: &mut PageTable) -> Result<(), ()> {
    unimplemented!()
}

impl Pte {
    pub const fn null() -> Self {
        unimplemented!()
    }
    pub const fn from(pa: PhysAddr, flags: Perms) -> Self {
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
    pub const fn get_flags(&self) -> Perms {
        unimplemented!()
    }
    pub const fn set_flags(&mut self, flags: Perms) {
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

/// 物理地址与虚拟地址转换
pub const fn phys_to_virt(pa: PhysAddr) -> VirtAddr {
    unimplemented!()
}

/// 虚拟地址与物理地址转换
pub const fn virt_to_phys(va: VirtAddr) -> PhysAddr {
    unimplemented!()
}
