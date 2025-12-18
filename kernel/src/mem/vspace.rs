use super::PhysAddr;
use super::PhysFrame;
use crate::mem::PageTable;
use riscv::register::satp;

// TODO: Add RAII
#[repr(C)]
pub struct VSpace {
    pub asid: usize,                // 地址空间标识符 (ASID)
    pub root_pt: Option<PhysFrame>, // RAII frame
}

impl VSpace {
    pub const fn new() -> Self {
        Self { asid: 0, root_pt: None }
    }
    pub fn init(&mut self) {
        let frame = PhysFrame::alloc(true).expect("Failed to allocate root page table frame");
        self.root_pt = Some(frame);
    }
    pub fn pa(&self) -> PhysAddr {
        self.root_pt.as_ref().expect("VSpace::pa: root_pt is None").addr()
    }
    pub fn satp(&self) -> usize {
        // 根页表物理页号
        let ppn = (self.pa() >> 12) & ((1usize << (usize::BITS as usize - 12)) - 1);
        // Compose SATP value for Sv39: MODE in bits [63:60], ASID=pid, PPN in [43:0]
        ((satp::Mode::Sv39 as usize) << 60) | (self.asid << 44) | ppn
    }
    pub fn get_pt_mut(&self) -> *mut PageTable {
        let pa = self.pa();
        pa as *mut PageTable
    }
    pub fn get_pt(&self) -> *const PageTable {
        let pa = self.pa();
        pa as *const PageTable
    }
    pub fn free(&mut self) {
        let page_table = unsafe { &mut *(self.pa() as *mut PageTable) };

        // Destory page table first (frees TrapFrame and children)
        page_table.destroy();
    }
}
