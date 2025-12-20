use super::{CapType, Capability};
use crate::mem::{PGSIZE, PhysAddr, PhysFrame};
use core::sync::atomic::{AtomicUsize, Ordering};

/// CNode 在物理内存中的布局头
#[repr(C)]
pub struct CNodeHeader {
    pub ref_count: AtomicUsize,
}

/// CDT (Capability Derivation Tree) 节点
/// 用于追踪能力的派生关系，实现 Revoke
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CDTNode {
    pub parent: PhysAddr,
    pub first_child: PhysAddr,
    pub next_sibling: PhysAddr,
    pub prev_sibling: PhysAddr,
}

impl CDTNode {
    pub const fn new() -> Self {
        Self { parent: 0, first_child: 0, next_sibling: 0, prev_sibling: 0 }
    }
}

/// CNode 中的一个槽位
#[repr(C)]
pub struct Slot {
    pub cap: Capability,
    pub cdt: CDTNode,
}

/// 能力节点 (CNode)
/// 本质上是一个存储在物理页中的 Slot 数组
pub struct CNode {
    paddr: PhysAddr,
    bits: u8,
}

impl CNode {
    pub fn new(paddr: PhysAddr, bits: u8) -> Self {
        // 初始化 Header
        let header_ptr = paddr as *mut CNodeHeader;
        unsafe {
            (*header_ptr).ref_count = AtomicUsize::new(1);
            // 初始化所有 Slot 为 Empty
            let slots_ptr =
                (paddr as *mut u8).add(core::mem::size_of::<CNodeHeader>()) as *mut Slot;
            for i in 0..(1 << bits) {
                core::ptr::write(
                    slots_ptr.add(i),
                    Slot { cap: Capability::empty(), cdt: CDTNode::new() },
                );
            }
        }
        Self { paddr, bits }
    }

    pub fn from_frame(frame: &PhysFrame, bits: u8) -> Self {
        Self { paddr: frame.addr(), bits }
    }

    pub fn from_addr(paddr: PhysAddr, bits: u8) -> Self {
        Self { paddr, bits }
    }

    pub fn size(&self) -> usize {
        1 << self.bits
    }

    fn get_header(&self) -> *mut CNodeHeader {
        self.paddr as *mut CNodeHeader
    }

    fn get_slots_ptr(&self) -> *mut Slot {
        // Slots 紧跟在 Header 之后
        unsafe { (self.paddr as *mut u8).add(core::mem::size_of::<CNodeHeader>()) as *mut Slot }
    }

    pub fn get_slot_addr(&self, slot: usize) -> PhysAddr {
        if slot >= self.size() {
            return 0;
        }
        unsafe { self.get_slots_ptr().add(slot) as PhysAddr }
    }

    pub fn lookup_cap(&self, slot: usize) -> Option<Capability> {
        if slot >= self.size() {
            return None;
        }
        let ptr = self.get_slots_ptr();
        let cap = unsafe { (*ptr.add(slot)).cap.clone() };
        if let CapType::Empty = cap.object { None } else { Some(cap) }
    }

    pub fn insert(&mut self, slot: usize, cap: Capability) -> bool {
        if slot >= self.size() {
            return false;
        }
        let ptr = self.get_slots_ptr();
        unsafe {
            // 注意：这里会触发旧 Cap 的 Drop
            (*ptr.add(slot)).cap = cap;
        }
        true
    }

    /// 插入能力并建立 CDT 关系
    pub fn insert_child(&mut self, slot: usize, cap: Capability, parent_addr: PhysAddr) -> bool {
        if slot >= self.size() {
            return false;
        }
        let slot_ptr = unsafe { self.get_slots_ptr().add(slot) };
        let slot_addr = slot_ptr as PhysAddr;

        unsafe {
            // 1. 插入能力
            (*slot_ptr).cap = cap;

            // 2. 建立 CDT 关系
            let mut cdt = CDTNode::new();
            cdt.parent = parent_addr;

            if parent_addr != 0 {
                let parent_slot = &mut *(parent_addr as *mut Slot);
                let old_first_child = parent_slot.cdt.first_child;

                cdt.next_sibling = old_first_child;
                if old_first_child != 0 {
                    let next_sib_slot = &mut *(old_first_child as *mut Slot);
                    next_sib_slot.cdt.prev_sibling = slot_addr;
                }
                parent_slot.cdt.first_child = slot_addr;
            }
            (*slot_ptr).cdt = cdt;
        }
        true
    }

    pub fn remove(&mut self, slot: usize) -> Option<Capability> {
        if slot >= self.size() {
            return None;
        }
        let ptr = self.get_slots_ptr();
        unsafe {
            let slot_ref = &mut *ptr.add(slot);
            let cap = core::ptr::read(&slot_ref.cap);
            core::ptr::write(&mut slot_ref.cap, Capability::empty());

            // 注意：remove 不会自动处理 CDT 关系，通常用于 Move
            // 如果是彻底删除，应该使用 delete_recursive
            if let CapType::Empty = cap.object { None } else { Some(cap) }
        }
    }
}
