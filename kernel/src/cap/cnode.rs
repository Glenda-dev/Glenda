use super::{CapType, Capability};
use crate::mem::VirtAddr;
use crate::printk;
use core::sync::atomic::AtomicUsize;

pub const SLOT_SIZE: usize = core::mem::size_of::<Slot>();
pub const CNODE_SIZE: usize = core::mem::size_of::<CNode>();
pub const CNODE_BITS: u8 = 8; // 256 slots per CNode
pub const CNODE_SLOTS: usize = 1 << CNODE_BITS;
pub const CNODE_PAGES: usize = 4;
pub const ROOT_BITS: u8 = 64;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CapPtr(usize);

impl CapPtr {
    pub const fn from(slot: usize) -> Self {
        CapPtr(slot)
    }
    pub fn next(&self) -> Self {
        CapPtr(self.0 + 1)
    }
    pub fn prev(&self) -> Self {
        CapPtr(self.0 - 1)
    }
    pub const fn bits(&self) -> usize {
        self.0
    }
    pub const fn is_null(&self) -> bool {
        self.0 == 0
    }
}

/// CDT (Capability Derivation Tree) 节点
/// 用于追踪能力的派生关系，实现 Revoke
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CDTNode {
    pub parent: VirtAddr,
    pub first_child: VirtAddr,
    pub next_sibling: VirtAddr,
    pub prev_sibling: VirtAddr,
}

impl CDTNode {
    pub const fn new() -> Self {
        Self {
            parent: VirtAddr::null(),
            first_child: VirtAddr::null(),
            next_sibling: VirtAddr::null(),
            prev_sibling: VirtAddr::null(),
        }
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
///
/// 实现了稀疏树（Radix Tree）结构的 CSpace。
/// 每个 CNode 节点大小固定（通常为 1 页），包含固定数量的 Slot。
/// 查找 Capability 时，根据 CPtr 的位段逐级索引。
#[repr(C)]
pub struct CNode {
    pub ref_count: AtomicUsize,
    pub guard: usize,
    pub guard_size: u8,
    pub bits: u8,
    pub _padding: [u8; 46],
    pub slots: [Slot; CNODE_SLOTS],
}

impl CNode {
    pub const fn new(bits: u8) -> Self {
        Self {
            ref_count: AtomicUsize::new(1),
            guard: 0,
            guard_size: 0,
            _padding: [0; 46],
            slots: [const { Slot { cap: Capability::empty(), cdt: CDTNode::new() } }; CNODE_SLOTS],
            bits: bits,
        }
    }

    /// 查找 Capability
    pub fn lookup(&self, cptr: CapPtr) -> Option<Capability> {
        let slot_ptr = self.lookup_slot_ptr(cptr)?;
        unsafe {
            let cap = (*slot_ptr).cap.clone();
            if cap.cap_type() == CapType::Empty { None } else { Some(cap) }
        }
    }

    /// 内部查找逻辑：返回找到的 Slot 指针
    pub fn lookup_slot_ptr(&self, cptr: CapPtr) -> Option<*mut Slot> {
        if cptr.is_null() {
            return None;
        }
        let bits = self.bits;

        // 1. Guard 检查
        if self.guard_size > 0 {
            if bits < self.guard_size {
                return None;
            }
            let guard_mask =
                if self.guard_size == 64 { !0 } else { (1usize << self.guard_size) - 1 };
            // 根据当前节点的 bits 定位 guard 在 cptr 中的位置
            let cptr_guard = (cptr.0 >> (bits - self.guard_size)) & guard_mask;
            if cptr_guard != self.guard {
                return None;
            }
        }

        let rem_bits = bits - self.guard_size;

        // 2. 本级索引
        if rem_bits == 0 {
            return None;
        }

        let radix_bits = CNODE_BITS;
        let index = if rem_bits > radix_bits {
            (cptr.bits() >> (rem_bits - radix_bits)) & ((1 << radix_bits) - 1)
        } else {
            cptr.bits() & ((1 << rem_bits) - 1)
        };

        if index >= CNODE_SLOTS {
            return None;
        }

        // 获取 Slot 指针
        let slot_ptr = unsafe { self.slots.as_ptr().add(index) as *mut Slot };
        let slot = unsafe { &*slot_ptr };

        // 3. 递归查下一层
        if rem_bits > radix_bits {
            if slot.cap.cap_type() == CapType::CNode {
                let next_cnode_addr = slot.cap.obj_ptr();
                if next_cnode_addr == VirtAddr::null() {
                    return None;
                }
                let next_cnode = next_cnode_addr.as_ref::<CNode>();
                // 递归调用
                return next_cnode.lookup_slot_ptr(cptr);
            } else {
                return Some(slot_ptr);
            }
        } else {
            return Some(slot_ptr);
        }
    }

    pub fn insert(&mut self, slot: usize, cap: &Capability) -> bool {
        if slot >= CNODE_SLOTS {
            return false;
        }
        self.slots[slot].cap = cap.clone();
        true
    }

    pub fn insert_child(&mut self, slot: usize, cap: &Capability, parent_addr: VirtAddr) -> bool {
        if slot >= CNODE_SLOTS {
            return false;
        }

        let slot_ref = &mut self.slots[slot];
        let slot_ptr = slot_ref as *mut Slot;
        let slot_addr = VirtAddr::from(slot_ptr as usize);

        // 1. 插入能力
        slot_ref.cap = cap.clone();

        // 2. 建立 CDT 关系
        let mut cdt = CDTNode::new();
        cdt.parent = parent_addr;
        if parent_addr != VirtAddr::null() {
            let parent_slot = parent_addr.as_mut::<Slot>();
            let old_first_child = parent_slot.cdt.first_child;

            cdt.next_sibling = old_first_child;
            if old_first_child != VirtAddr::null() {
                let next_sib_slot = old_first_child.as_mut::<Slot>();
                next_sib_slot.cdt.prev_sibling = slot_addr;
            }
            parent_slot.cdt.first_child = slot_addr;
        }
        slot_ref.cdt = cdt;
        true
    }

    pub fn remove(&mut self, slot: usize) -> Option<Capability> {
        if slot >= CNODE_SLOTS {
            return None;
        }
        let cap = self.slots[slot].cap.clone();
        self.slots[slot].cap = Capability::empty();
        if cap.cap_type() == CapType::Empty { None } else { Some(cap) }
    }

    pub fn revoke(&mut self, slot: usize) {
        if slot >= CNODE_SLOTS {
            return;
        }
        let slot_addr = self.get_slot_addr(slot);
        revoke_recursive(slot_addr);
    }

    pub fn delete(&mut self, slot: usize) {
        if slot >= CNODE_SLOTS {
            return;
        }
        let slot_addr = self.get_slot_addr(slot);
        delete_recursive(slot_addr);
    }

    pub fn debug_print(&self) {
        self.debug_print_recursive(0);
    }

    fn debug_print_recursive(&self, depth: usize) {
        if depth > 8 {
            panic!("CNode debug_print_recursive: too deep recursion");
        }

        let start = { if depth == 0 { 2 } else { 1 } };

        for i in start..CNODE_SLOTS {
            let slot = &self.slots[i];
            if slot.cap.cap_type() == CapType::Empty {
                continue;
            }
            for _ in 0..depth {
                printk!("  ");
            }
            printk!("[0x{:02x}] {}\n", i, slot.cap);

            if slot.cap.cap_type() == CapType::CNode {
                let child_ptr = slot.cap.obj_ptr();
                if child_ptr != VirtAddr::null() {
                    child_ptr.as_ref::<CNode>().debug_print_recursive(depth + 1);
                }
            }
        }
    }

    pub fn get_slot_addr(&self, index: usize) -> VirtAddr {
        VirtAddr::from(&self.slots[index] as *const Slot as usize)
    }
}

fn revoke_recursive(slot_addr: VirtAddr) {
    let slot = slot_addr.as_mut::<Slot>();
    let mut child_addr = slot.cdt.first_child;
    while child_addr != VirtAddr::null() {
        let next_sibling = (*(child_addr.as_mut::<Slot>())).cdt.next_sibling;
        delete_recursive(child_addr);
        child_addr = next_sibling;
    }
    slot.cdt.first_child = VirtAddr::null();
}

fn delete_recursive(slot_addr: VirtAddr) {
    // 1. 递归撤销所有子能力
    revoke_recursive(slot_addr);

    // 2. 从 CDT 兄弟链表中移除
    let slot = slot_addr.as_mut::<Slot>();
    let prev = slot.cdt.prev_sibling;
    let next = slot.cdt.next_sibling;
    let parent = slot.cdt.parent;

    if prev != VirtAddr::null() {
        (*(prev.as_mut::<Slot>())).cdt.next_sibling = next;
    } else if parent != VirtAddr::null() {
        (*(parent.as_mut::<Slot>())).cdt.first_child = next;
    }

    if next != VirtAddr::null() {
        (*(next.as_mut::<Slot>())).cdt.prev_sibling = prev;
    }

    // 3. 清空槽位 (触发 Capability::drop)
    slot.cap = Capability::empty();
    slot.cdt = CDTNode::new();
}
