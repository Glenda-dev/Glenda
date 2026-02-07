use super::{CapType, Capability};
use crate::hal::mem::PGSIZE;
use crate::mem::VirtAddr;
use crate::printk;
use core::fmt::Display;
use core::sync::atomic::AtomicUsize;
use spin::Mutex;

pub const SLOT_SIZE: usize = core::mem::size_of::<Slot>();
pub const CNODE_SIZE: usize = core::mem::size_of::<CNode>();
pub const CNODE_BITS: usize = 8; // 256 slots per CNode
pub const CNODE_SLOTS: usize = 1 << CNODE_BITS;
pub const CNODE_MASK: usize = CNODE_SLOTS - 1;
pub const CNODE_PAGES: usize = (CNODE_SIZE + PGSIZE - 1) / PGSIZE; // CNode 占用的页数

static CDT_LOCK: Mutex<()> = Mutex::new(());

/// 每8位作为一层的索引号
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CapPtr(usize);

impl CapPtr {
    pub const fn null() -> Self {
        CapPtr(0)
    }
    pub const fn from(slot: usize) -> Self {
        CapPtr(slot)
    }
    pub fn next(&self) -> Self {
        CapPtr(self.0 >> CNODE_BITS)
    }
    pub const fn bits(&self) -> usize {
        self.0
    }
    pub const fn index(&self) -> usize {
        (self.0 & CNODE_MASK) as usize
    }
    pub const fn is_null(&self) -> bool {
        self.0 & CNODE_MASK == 0
    }
}

impl Display for CapPtr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#x}", self.0)
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
    pub _padding: [u8; 16], // 16 (Cap) + 32 (CDT) + 16 = 64 字节
}

/// 能力节点 (CNode)
/// 本质上是一个存储在物理页中的 Slot 数组
/// 每个 CNode 节点大小固定（通常为 1 页），包含固定数量的 Slot。
/// 查找 Capability 时，根据 CPtr 的位段逐级索引。
#[repr(C)]
pub struct CNode {
    pub slots: [Slot; CNODE_SLOTS],
    pub ref_count: AtomicUsize,
}

impl CNode {
    pub fn new() -> Self {
        Self {
            slots: [const { Slot { cap: Capability::empty(), cdt: CDTNode::new(), _padding: [0; 16] } };
                CNODE_SLOTS],
            ref_count: AtomicUsize::new(1),
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

        let index = cptr.index();
        let next_cptr = cptr.next();

        // 获取 Slot 指针
        let slot_ptr = unsafe { self.slots.as_ptr().add(index) as *mut Slot };
        let slot = unsafe { &*slot_ptr };

        if next_cptr.is_null() {
            return Some(slot_ptr);
        } else {
            if slot.cap.cap_type() == CapType::CNode {
                let next_cnode_addr = slot.cap.obj_ptr();
                if next_cnode_addr == VirtAddr::null() {
                    return None;
                }
                let next_cnode = next_cnode_addr.as_ref::<CNode>();
                // 递归调用
                return next_cnode.lookup_slot_ptr(next_cptr);
            } else {
                return None;
            }
        }
    }

    pub fn insert(&mut self, cptr: CapPtr, cap: &Capability) -> bool {
        let slot = match self.lookup_slot_ptr(cptr) {
            None => return false,
            Some(ptr) => unsafe { &mut *ptr },
        };
        if slot.cap.cap_type() != CapType::Empty {
            return false;
        }
        slot.cap = cap.clone();
        true
    }

    pub fn insert_child(&mut self, cptr: CapPtr, cap: &Capability, parent_slot: *mut Slot) -> bool {
        let _ = CDT_LOCK.lock();
        if cptr.is_null() {
            return false;
        }
        let slot = match self.lookup_slot_ptr(cptr) {
            None => return false,
            Some(ptr) => unsafe { &mut *ptr },
        };

        // 必须确保目标槽位为空，否则会破坏 CDT
        if slot.cap.cap_type() != CapType::Empty {
            return false;
        }

        slot.cap = cap.clone();

        // 2. 建立 CDT 关系
        let mut cdt = CDTNode::new();
        cdt.parent = VirtAddr::from(parent_slot as usize);

        let parent_slot_ref = unsafe { &mut *parent_slot };
        let old_first_child = parent_slot_ref.cdt.first_child;

        cdt.next_sibling = old_first_child;
        if old_first_child != VirtAddr::null() {
            let next_sib_slot = old_first_child.as_mut::<Slot>();
            next_sib_slot.cdt.prev_sibling = VirtAddr::from(slot as *mut Slot as usize);
        }
        parent_slot_ref.cdt.first_child = VirtAddr::from(slot as *mut Slot as usize);

        slot.cdt = cdt;
        true
    }

    pub fn revoke(&mut self, cptr: CapPtr) -> bool {
        let _ = CDT_LOCK.lock();
        let slot = match self.lookup_slot_ptr(cptr) {
            None => return false,
            Some(ptr) => unsafe { &mut *ptr },
        };
        revoke_recursive(slot);
        true
    }

    pub fn delete(&mut self, cptr: CapPtr) -> bool {
        let _ = CDT_LOCK.lock();
        if cptr.is_null() {
            return false;
        }
        let slot = match self.lookup_slot_ptr(cptr) {
            None => return false,
            Some(ptr) => unsafe { &mut *ptr },
        };
        delete_recursive(slot);
        true
    }

    pub fn debug_print(&self) {
        self.debug_print_recursive(0);
    }

    fn debug_print_recursive(&self, depth: usize) {
        if depth > 64 / CNODE_BITS {
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

    pub fn check_cptr(&self, cptr: CapPtr) -> bool {
        if cptr.is_null() {
            return false;
        }

        let index = cptr.index();
        let next_cptr = cptr.next();

        // 获取 Slot 指针
        let slot_ptr = unsafe { self.slots.as_ptr().add(index) as *mut Slot };
        let slot = unsafe { &*slot_ptr };

        if next_cptr.is_null() {
            return true;
        } else {
            if slot.cap.cap_type() == CapType::CNode {
                let next_cnode_addr = slot.cap.obj_ptr();
                if next_cnode_addr == VirtAddr::null() {
                    return false;
                }
                let next_cnode = next_cnode_addr.as_ref::<CNode>();
                // 递归调用
                return next_cnode.check_cptr(next_cptr);
            } else {
                return false;
            }
        }
    }
}

fn revoke_recursive(slot: &mut Slot) {
    let mut child_addr = slot.cdt.first_child;
    while child_addr != VirtAddr::null() {
        let child_slot = &mut *child_addr.as_mut::<Slot>();
        let next_sibling = child_slot.cdt.next_sibling;
        delete_recursive(child_slot);
        child_addr = next_sibling;
    }
    slot.cdt.first_child = VirtAddr::null();
}

fn delete_recursive(slot: &mut Slot) {
    // 1. 递归撤销所有子能力
    revoke_recursive(slot);

    // 2. 从 CDT 兄弟链表中移除
    let prev = slot.cdt.prev_sibling;
    let next = slot.cdt.next_sibling;
    let parent = slot.cdt.parent;

    if prev != VirtAddr::null() {
        prev.as_mut::<Slot>().cdt.next_sibling = next;
    } else if parent != VirtAddr::null() {
        parent.as_mut::<Slot>().cdt.first_child = next;
    }

    if next != VirtAddr::null() {
        next.as_mut::<Slot>().cdt.prev_sibling = prev;
    }

    // 3. 清空槽位 (触发 Capability::drop)
    slot.cap = Capability::empty();
    slot.cdt = CDTNode::new();
}
