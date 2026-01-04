use super::{CapType, Capability};
use crate::mem::PhysAddr;
use core::sync::atomic::AtomicUsize;

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
        Self {
            parent: PhysAddr::null(),
            first_child: PhysAddr::null(),
            next_sibling: PhysAddr::null(),
            prev_sibling: PhysAddr::null(),
        }
    }
}

/// CNode 中的一个槽位
#[repr(C)]
pub struct Slot {
    pub cap: Capability,
    pub cdt: CDTNode,
}

pub const CNODE_BITS: usize = 12; // 4096 slots per CNode
pub const CNODE_SLOTS: usize = 1 << CNODE_BITS;

/// 能力节点 (CNode)
/// 本质上是一个存储在物理页中的 Slot 数组
pub struct CNode {
    paddr: PhysAddr,
}

impl CNode {
    pub fn new(paddr: PhysAddr) -> Self {
        // 初始化 Header
        let header_ptr = paddr.as_mut::<CNodeHeader>();
        unsafe {
            (*header_ptr).ref_count = AtomicUsize::new(1);
            // 初始化所有 Slot 为 Empty
            let slots_ptr =
                (paddr.as_mut_ptr::<u8>()).add(core::mem::size_of::<CNodeHeader>()) as *mut Slot;
            for i in 0..(1 << 12) {
                core::ptr::write(
                    slots_ptr.add(i),
                    Slot { cap: Capability::empty(), cdt: CDTNode::new() },
                );
            }
        }
        Self { paddr }
    }

    pub fn from_addr(paddr: PhysAddr) -> Self {
        Self { paddr }
    }

    pub fn size(&self) -> usize {
        1 << CNODE_BITS
    }

    fn get_header(&self) -> *mut CNodeHeader {
        self.paddr.as_mut::<CNodeHeader>()
    }

    fn get_slots_ptr(&self) -> *mut Slot {
        // Slots 紧跟在 Header 之后
        unsafe {
            (self.paddr.as_mut_ptr::<u8>()).add(core::mem::size_of::<CNodeHeader>()) as *mut Slot
        }
    }

    pub fn get_slot_addr(&self, slot: usize) -> PhysAddr {
        if slot >= self.size() {
            return PhysAddr::null();
        }
        unsafe { PhysAddr::from(self.get_slots_ptr().add(slot) as usize) }
    }

    pub fn lookup_cap(&self, slot: usize) -> Option<Capability> {
        if slot >= self.size() {
            return None;
        }
        let ptr = self.get_slots_ptr();
        let cap = unsafe { (*ptr.add(slot)).cap.clone() };
        if let CapType::Empty = cap.object { None } else { Some(cap) }
    }

    pub fn insert(&mut self, slot: usize, cap: &Capability) -> bool {
        if slot >= self.size() {
            return false;
        }
        let ptr = self.get_slots_ptr();
        unsafe {
            // 注意：这里会触发旧 Cap 的 Drop
            (*ptr.add(slot)).cap = cap.clone();
        }
        true
    }

    /// 插入能力并建立 CDT 关系
    pub fn insert_child(&mut self, slot: usize, cap: &Capability, parent_addr: PhysAddr) -> bool {
        if slot >= self.size() {
            return false;
        }
        let slot_ptr = unsafe { self.get_slots_ptr().add(slot) };
        let slot_addr = PhysAddr::from(slot_ptr as usize);

        unsafe {
            // 1. 插入能力
            (*slot_ptr).cap = cap.clone();
            // 2. 建立 CDT 关系
            let mut cdt = CDTNode::new();
            cdt.parent = parent_addr;

            if parent_addr != PhysAddr::null() {
                let parent_slot = &mut *(parent_addr.as_mut::<Slot>());
                let old_first_child = parent_slot.cdt.first_child;

                cdt.next_sibling = old_first_child;
                if old_first_child != PhysAddr::null() {
                    let next_sib_slot = &mut *(old_first_child.as_mut::<Slot>());
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

    pub fn revoke(&mut self, slot: usize) {
        if slot >= self.size() {
            return;
        }
        let slot_addr = self.get_slot_addr(slot);
        revoke_recursive(slot_addr);
    }

    pub fn delete(&mut self, slot: usize) {
        if slot >= self.size() {
            return;
        }
        let slot_addr = self.get_slot_addr(slot);
        delete_recursive(slot_addr);
    }
}

fn revoke_recursive(slot_addr: PhysAddr) {
    let slot = slot_addr.as_mut::<Slot>();
    let mut child_addr = slot.cdt.first_child;
    while child_addr != PhysAddr::null() {
        let next_sibling = (*(child_addr.as_mut::<Slot>())).cdt.next_sibling;
        delete_recursive(child_addr);
        child_addr = next_sibling;
    }
    slot.cdt.first_child = PhysAddr::null();
}

fn delete_recursive(slot_addr: PhysAddr) {
    // 1. 递归撤销所有子能力
    revoke_recursive(slot_addr);

    // 2. 从 CDT 兄弟链表中移除
    let slot = slot_addr.as_mut::<Slot>();
    let prev = slot.cdt.prev_sibling;
    let next = slot.cdt.next_sibling;
    let parent = slot.cdt.parent;

    if prev != PhysAddr::null() {
        (*(prev.as_mut::<Slot>())).cdt.next_sibling = next;
    } else if parent != PhysAddr::null() {
        (*(parent.as_mut::<Slot>())).cdt.first_child = next;
    }

    if next != PhysAddr::null() {
        (*(next.as_mut::<Slot>())).cdt.prev_sibling = prev;
    }

    // 3. 清空槽位 (触发 Capability::drop)
    slot.cap = Capability::empty();
    slot.cdt = CDTNode::new();
}
