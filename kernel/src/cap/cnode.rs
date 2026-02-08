use super::{CapType, Capability};
use crate::hal::mem::PGSIZE;
use crate::mem::VirtAddr;
use crate::printk;
use crate::sync::{SpinLock, SpinLockGuard};
use core::fmt::Display;
use core::sync::atomic::AtomicUsize;

pub const SLOT_SIZE: usize = core::mem::size_of::<Slot>();
pub const CNODE_SIZE: usize = core::mem::size_of::<CNode>();
pub const CNODE_BITS: usize = 8; // 256 slots per CNode
pub const CNODE_SLOTS: usize = 1 << CNODE_BITS;
pub const CNODE_MASK: usize = CNODE_SLOTS - 1;
pub const CNODE_PAGES: usize = (CNODE_SIZE + PGSIZE - 1) / PGSIZE; // CNode 占用的页数

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
    pub cnode_lock: usize, // Pointer to CNode::lock
    pub _padding: [u8; 8], // 16 (Cap) + 32 (CDT) + 8 + 8 = 64 字节
}

impl Slot {
    /// Lock the CNode containing this slot.
    /// Unsafe because it dereferences a raw pointer stored in the slot.
    /// Returns a guard that is completely detached from the Slot's lifetime
    /// (by extending lifetime to 'static due to CNode immobility).
    pub unsafe fn lock_cnode<'a>(&self) -> SpinLockGuard<'a, ()> {
        let lock_ptr = self.cnode_lock as *const SpinLock<()>;
        let lock_ref = unsafe { &*lock_ptr };
        lock_ref.lock()
    }
}

/// CNode 元数据，重用 Slot 0 的空间存储
#[repr(C)]
struct CNodeMetadata {
    ref_count: AtomicUsize,
    lock: SpinLock<()>,
}

/// 能力节点 (CNode)
/// 本质上是一个存储在物理页中的 Slot 数组
/// 每个 CNode 节点大小固定（通常为 1 页），包含固定数量的 Slot。
/// 查找 Capability 时，根据 CPtr 的位段逐级索引。
#[repr(C)]
pub struct CNode {
    pub slots: [Slot; CNODE_SLOTS],
}

impl CNode {
    fn metadata(&self) -> &CNodeMetadata {
        unsafe { &*(self.slots.as_ptr() as *const CNodeMetadata) }
    }

    /// 创建一个新的 CNode。
    ///
    /// # Safety
    /// **必须**在将 CNode 写入内存位置后调用 `set_lock_pointers()`，
    /// 以便初始化 Slot 中的自引用锁指针。
    pub fn new() -> Self {
        let mut node = Self {
            slots: [const {
                Slot {
                    cap: Capability::empty(),
                    cdt: CDTNode::new(),
                    cnode_lock: 0,
                    _padding: [0; 8],
                }
            }; CNODE_SLOTS],
        };

        // 在 Slot 0 中初始化元数据
        // 注意：这之后 Slot 0 不能作为常规 Slot 使用
        let metadata = CNodeMetadata { ref_count: AtomicUsize::new(1), lock: SpinLock::new(()) };

        unsafe {
            let ptr = node.slots.as_mut_ptr() as *mut CNodeMetadata;
            ptr.write(metadata);
        }

        node
    }

    /// 初始化槽位的锁指针 (必须在对象固定在内存后调用)
    pub unsafe fn set_lock_pointers(&mut self) {
        let lock_ptr = &self.metadata().lock as *const SpinLock<()> as usize;
        // 跳过 Slot 0 (元数据)
        for slot in self.slots[1..].iter_mut() {
            slot.cnode_lock = lock_ptr;
        }
    }

    /// 查找 Capability
    pub fn lookup(&self, cptr: CapPtr) -> Option<Capability> {
        self.lookup_with_lock(cptr)
    }

    fn lookup_with_lock(&self, cptr: CapPtr) -> Option<Capability> {
        if cptr.is_null() {
            return None;
        }

        // 1. Lock Current Node
        let _guard = self.metadata().lock.lock();

        let index = cptr.index();
        let next_cptr = cptr.next();

        if index >= CNODE_SLOTS {
            return None;
        }

        // Slot 0 被元数据占用，不可访问
        if index == 0 {
            return None;
        }

        let slot = &self.slots[index];
        let cap = slot.cap.clone();

        if next_cptr.is_null() {
            // Found leaf
            if cap.cap_type() == CapType::Empty { None } else { Some(cap) }
        } else {
            // Need to recurse
            // Drop Lock before recursing to avoid hold-and-wait deadlock?
            // Actually deadlock is only possible if we go back UP or cycle.
            // CSpace is a directed graph (usually tree).
            // But holding lock while recursing indefinitely is bad for latency.
            // So we drop, then recurse.
            drop(_guard);

            if cap.cap_type() == CapType::CNode {
                let next_cnode_addr = cap.obj_ptr();
                if next_cnode_addr == VirtAddr::null() {
                    return None;
                }
                let next_cnode = next_cnode_addr.as_ref::<CNode>();
                // Recurse
                next_cnode.lookup_with_lock(next_cptr)
            } else {
                None
            }
        }
    }

    /// 内部查找逻辑：返回找到的 Slot 指针
    /// 警告：返回的指针没有锁保护，仅供持有锁的上下文使用
    pub unsafe fn lookup_slot_ptr(&self, cptr: CapPtr) -> Option<*mut Slot> {
        if cptr.is_null() {
            return None;
        }

        // 注意：这个旧函数无法方便地加锁，保留可以但仅受限使用。
        // 为了兼容旧代码（如 insert），我们可能暂时不加锁，
        // 或者要求 insert 自己处理锁。

        let index = cptr.index();
        let next_cptr = cptr.next();

        if index == 0 {
            return None;
        }

        // 获取 Slot 指针
        // 使用 raw pointer cast 避免 &T -> &mut T UB 检查
        let cnode_mut_ptr = self as *const CNode as *mut CNode;
        let slot_ptr = unsafe { (*cnode_mut_ptr).slots.as_mut_ptr().add(index) };
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
                return unsafe { next_cnode.lookup_slot_ptr(next_cptr) };
            } else {
                return None;
            }
        }
    }

    pub fn insert(&mut self, cptr: CapPtr, cap: &Capability) -> bool {
        self.insert_with_lock(cptr, cap)
    }

    fn insert_with_lock(&self, cptr: CapPtr, cap: &Capability) -> bool {
        if cptr.is_null() {
            return false;
        }

        // 1. Lock Current Node
        let _guard = self.metadata().lock.lock();

        let index = cptr.index();
        let next_cptr = cptr.next();

        if index >= CNODE_SLOTS || index == 0 {
            return false;
        }

        // 我们需要由 &self 转为 &mut Slot，这是一个 UnsafeCell 转换，
        // 但由于我们持有 SpinLock，这是安全的。
        let cnode_mut_ptr = self as *const CNode as *mut CNode;
        let slot = unsafe { &mut (*cnode_mut_ptr).slots[index] };

        if next_cptr.is_null() {
            // Found leaf - check if empty
            if slot.cap.cap_type() != CapType::Empty {
                return false;
            }
            slot.cap = cap.clone();
            true
        } else {
            // Recurse
            let current_cap = slot.cap.clone();
            drop(_guard); // Unlock before recursing

            if current_cap.cap_type() == CapType::CNode {
                let next_cnode_addr = current_cap.obj_ptr();
                if next_cnode_addr == VirtAddr::null() {
                    return false;
                }
                let next_cnode = next_cnode_addr.as_ref::<CNode>();
                next_cnode.insert_with_lock(next_cptr, cap)
            } else {
                false
            }
        }
    }

    pub fn insert_child(&mut self, cptr: CapPtr, cap: &Capability, parent_slot: *mut Slot) -> bool {
        // 1. Lock Self
        let _self_lock_guard = self.metadata().lock.lock();

        if cptr.is_null() {
            return false;
        }
        // Unsafe lookup without lock is fine because we hold lock
        let slot = match unsafe { self.lookup_slot_ptr(cptr) } {
            None => return false,
            Some(ptr) => unsafe { &mut *ptr },
        };

        // 必须确保目标槽位为空，否则会破坏 CDT
        if slot.cap.cap_type() != CapType::Empty {
            return false;
        }

        let parent_slot_ref = unsafe { &mut *parent_slot };

        // 2. Lock Parent (if different from self)
        let parent_lock_ptr = parent_slot_ref.cnode_lock;
        let self_lock_ptr = &self.metadata().lock as *const SpinLock<()> as usize;

        let _parent_guard = if parent_lock_ptr != self_lock_ptr {
            unsafe { Some((*(parent_lock_ptr as *const SpinLock<()>)).lock()) }
        } else {
            None
        };

        slot.cap = cap.clone();

        // 3. 建立 CDT 关系
        let mut cdt = CDTNode::new();
        cdt.parent = VirtAddr::from(parent_slot as usize);

        let old_first_child = parent_slot_ref.cdt.first_child;

        cdt.next_sibling = old_first_child;
        if old_first_child != VirtAddr::null() {
            let next_sib_slot = &mut *old_first_child.as_mut::<Slot>();

            // Need to lock Next Sibling?
            let sib_lock_ptr = next_sib_slot.cnode_lock;
            let _sib_guard = if sib_lock_ptr != self_lock_ptr && sib_lock_ptr != parent_lock_ptr {
                unsafe { Some((*(sib_lock_ptr as *const SpinLock<()>)).lock()) }
            } else {
                None
            };

            next_sib_slot.cdt.prev_sibling = VirtAddr::from(slot as *mut Slot as usize);
        }
        parent_slot_ref.cdt.first_child = VirtAddr::from(slot as *mut Slot as usize);

        slot.cdt = cdt;
        true
    }

    pub fn revoke(&mut self, cptr: CapPtr) -> bool {
        let _guard = self.metadata().lock.lock();
        let slot = match unsafe { self.lookup_slot_ptr(cptr) } {
            None => return false,
            Some(ptr) => unsafe { &mut *ptr },
        };
        revoke_recursive(slot);
        true
    }

    pub fn delete(&mut self, cptr: CapPtr) -> bool {
        let _guard = self.metadata().lock.lock();
        if cptr.is_null() {
            return false;
        }
        let slot = match unsafe { self.lookup_slot_ptr(cptr) } {
            None => return false,
            Some(ptr) => unsafe { &mut *ptr },
        };
        delete_recursive(slot);
        true
    }

    pub fn ref_count(&self) -> &AtomicUsize {
        &self.metadata().ref_count
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
    slot.cdt.first_child = VirtAddr::null();

    while child_addr != VirtAddr::null() {
        let child_slot = &mut *child_addr.as_mut::<Slot>();

        let next_sibling = child_slot.cdt.next_sibling;

        // Decouple lifetime for recursion
        let lock_ptr = child_slot.cnode_lock as *const SpinLock<()>;
        unsafe {
            let lock = &*lock_ptr;
            let _guard = lock.lock();

            revoke_recursive(child_slot);

            // Clear slot
            child_slot.cdt = CDTNode::new();
            child_slot.cap = Capability::empty();
        }

        child_addr = next_sibling;
    }
}

fn delete_recursive(slot: &mut Slot) {
    // 1. 递归撤销所有子能力
    revoke_recursive(slot);

    // 2. 从 CDT 兄弟链表中移除
    let prev = slot.cdt.prev_sibling;
    let next = slot.cdt.next_sibling;
    let parent = slot.cdt.parent;

    // Use raw pointer for lock comparison
    let self_lock_ptr = slot.cnode_lock;

    unsafe {
        if prev != VirtAddr::null() {
            let prev_slot = &mut *prev.as_mut::<Slot>();
            // Lock Prev
            let lock_ptr = prev_slot.cnode_lock;
            let _guard = if lock_ptr != self_lock_ptr {
                Some((*(lock_ptr as *const SpinLock<()>)).lock())
            } else {
                None
            };

            prev_slot.cdt.next_sibling = next;
        } else if parent != VirtAddr::null() {
            let parent_slot = &mut *parent.as_mut::<Slot>();
            // Lock Parent
            let lock_ptr = parent_slot.cnode_lock;
            let _guard = if lock_ptr != self_lock_ptr {
                Some((*(lock_ptr as *const SpinLock<()>)).lock())
            } else {
                None
            };

            parent_slot.cdt.first_child = next;
        }

        if next != VirtAddr::null() {
            let next_slot = &mut *next.as_mut::<Slot>();
            // Lock Next
            let lock_ptr = next_slot.cnode_lock;
            let _guard = if lock_ptr != self_lock_ptr {
                Some((*(lock_ptr as *const SpinLock<()>)).lock())
            } else {
                None
            };

            next_slot.cdt.prev_sibling = prev;
        }
    }

    // 3. 清空槽位 (触发 Capability::drop)
    slot.cap = Capability::empty();
    slot.cdt = CDTNode::new();
}
