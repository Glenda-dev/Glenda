use crate::cap::CapType;
use crate::cap::Capability;
use crate::mem::{PGSIZE, PhysAddr};
use riscv::asm::sfence_vma_all;
use riscv::register::satp;
use spin::Mutex;

/// ASID 管理器 (单例)
static ASID_MANAGER: Mutex<AsidManager> = Mutex::new(AsidManager::new());

struct AsidManager {
    /// 当前生成的 ASID (0..MAX_ASID)
    current_asid: u16,
    /// 全局代际计数器
    generation: u64,
}

impl AsidManager {
    const MAX_ASID: u16 = 0xFFFF; // RISC-V SV39/48 通常支持 16位 ASID

    const fn new() -> Self {
        Self {
            current_asid: 0,
            generation: 1, // 从 1 开始，0 表示未初始化
        }
    }

    /// 分配一个新的 ASID
    /// 如果这一代用完了，会触发 flush 并进入下一代
    fn alloc(&mut self) -> (u16, u64) {
        if self.current_asid < Self::MAX_ASID {
            self.current_asid += 1;
            (self.current_asid, self.generation)
        } else {
            // ASID 耗尽，进入下一代
            self.generation += 1;
            self.current_asid = 1;

            // 关键：刷新所有 TLB，因为我们即将复用 ASID 1
            // 在 RISC-V 中，这会使所有旧的 ASID 条目失效
            sfence_vma_all();

            (self.current_asid, self.generation)
        }
    }
}

/// 虚拟地址空间
/// 在微内核中，它主要代表根页表 + ASID
#[derive(Debug)]
pub struct VSpace {
    /// 根页表的物理地址 (用于写入 satp.ppn)
    root_paddr: PhysAddr,

    /// 缓存的 ASID
    asid: u16,
    /// 该 ASID 所属的代际
    asid_generation: u64,
}

impl VSpace {
    /// 创建一个新的 VSpace (通常由 Retype 调用)
    pub fn new(root_paddr: PhysAddr) -> Self {
        Self {
            root_paddr,
            asid: 0,
            asid_generation: 0, // 0 表示无效/未分配
        }
    }

    pub const fn empty() -> Self {
        Self { root_paddr: PhysAddr::null(), asid: 0, asid_generation: 0 }
    }

    pub fn get_satp(&self) -> usize {
        // Mode: Sv39 (8)
        // PPN: paddr >> 12
        let mode = 8usize << 60;
        let ppn = self.root_paddr.as_usize() >> 12;
        let asid = self.asid as usize;
        mode | (asid << 44) | ppn
    }

    /// 激活此地址空间 (上下文切换时调用)
    /// 返回需要写入 satp 的值 (包含 ASID)
    pub fn activate(&mut self) {
        // 检查 ASID 是否有效且属于当前代际
        let mut manager = ASID_MANAGER.lock();
        if self.asid == 0 || self.asid_generation != manager.generation {
            let (asid, generation) = manager.alloc();
            self.asid = asid;
            self.asid_generation = generation;
        }
        drop(manager);

        assert!(self.root_paddr.is_aligned(PGSIZE), "Root page table address must be page-aligned");
        unsafe {
            satp::set(satp::Mode::Sv39, self.asid as usize, self.root_paddr.to_ppn().as_usize());
        }
    }

    /// 获取根页表物理地址
    pub fn root_paddr(&self) -> PhysAddr {
        self.root_paddr
    }

    pub fn asid(&self) -> u16 {
        self.asid
    }

    pub fn configure(&mut self, cap: &Capability) {
        match cap.object {
            CapType::PageTable { paddr, level, .. } => {
                assert!(level == 2, "Root page table must be level 2");
                assert!(paddr.is_aligned(PGSIZE), "Page table address must be page-aligned");
                self.root_paddr = paddr;
            }
            _ => return,
        }
    }
}
