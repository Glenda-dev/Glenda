use super::{CapType, Capability};
use crate::hal::mem::PGSIZE;
use crate::mem::VirtAddr;
use crate::printk;
use crate::sync::{SpinLock, SpinLockGuard};
use core::cell::UnsafeCell;
use core::fmt::Display;
use core::sync::atomic::{AtomicUsize, Ordering};

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
#[derive(Debug)]
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
    pub cnode_lock: VirtAddr, // Pointer to CNode::lock
    pub _padding: [u8; 8],    // 16 (Cap) + 32 (CDT) + 8 + 8 = 64 字节
}

impl Slot {
    /// Lock the CNode containing this slot.
    /// Unsafe because it dereferences a raw pointer stored in the slot.
    /// Returns a guard that is completely detached from the Slot's lifetime
    /// (by extending lifetime to 'static due to CNode immobility).
    pub unsafe fn lock_cnode<'a>(&self) -> SpinLockGuard<'a, ()> {
        let lock_ref = unsafe { self.cnode_lock.as_ref::<SpinLock<()>>() };
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
    pub slots: UnsafeCell<[Slot; CNODE_SLOTS]>,
}

unsafe impl Sync for CNode {}

impl CNode {
    fn metadata(&self) -> &CNodeMetadata {
        unsafe { &*(self.slots.get() as *const CNodeMetadata) }
    }

    /// 初始化 CNode。(原地初始化，避免栈溢出)
    pub fn init(&mut self) {
        unsafe {
            // 1. Zero out content (All Empty caps)
            core::ptr::write_bytes(self.slots.get() as *mut u8, 0, CNODE_SIZE);

            // 2. 在 Slot 0 中初始化元数据
            // 使用 write 而不是 assignment 来避免析构旧数据（虽然是0）
            let metadata =
                CNodeMetadata { ref_count: AtomicUsize::new(0), lock: SpinLock::new(()) };
            let metadata_ptr = self.slots.get() as *mut CNodeMetadata;
            metadata_ptr.write(metadata);

            // 3. 设置所有槽位的锁指针
            // 重要：必须使用我们刚刚写入的 metadata_ptr 来获取锁地址，
            // 否则编译器可能会重新从 self.slots 读取，导致别名分析错误。
            let lock_ptr = VirtAddr::from(&(*metadata_ptr).lock as *const SpinLock<()> as usize);

            // 跳过 Slot 0 (元数据)
            // 直接操作裸指针以避免切片迭代器带来的任何别名假设
            let mut current_slot = (self.slots.get() as *mut Slot).add(1);
            for _ in 1..CNODE_SLOTS {
                (*current_slot).cnode_lock = lock_ptr;
                current_slot = current_slot.add(1);
            }
        }
    }

    /// 辅助函数：获取槽位指针
    unsafe fn get_slot_ptr(&self, index: usize) -> *mut Slot {
        unsafe { (self.slots.get() as *mut Slot).add(index) }
    }

    /// 查找 Capability
    pub fn lookup(&self, cptr: CapPtr) -> Option<Capability> {
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

        let slot = unsafe { &*self.get_slot_ptr(index) };
        let cap = slot.cap.clone();

        if next_cptr.is_null() {
            // Found leaf
            if cap.cap_type() == CapType::Empty { None } else { Some(cap) }
        } else {
            // Need to recurse
            drop(_guard);

            if cap.cap_type() == CapType::CNode {
                let next_cnode_addr = cap.obj_ptr();
                if next_cnode_addr == VirtAddr::null() {
                    return None;
                }
                let next_cnode = unsafe { next_cnode_addr.as_ref::<CNode>() };
                // Recurse
                next_cnode.lookup(next_cptr)
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

        let index = cptr.index();
        let next_cptr = cptr.next();

        if index == 0 {
            return None;
        }

        // 获取 Slot 指针
        let slot_ptr = unsafe { self.get_slot_ptr(index) };
        let slot = unsafe { &*slot_ptr };

        if next_cptr.is_null() {
            return Some(slot_ptr);
        } else {
            if slot.cap.cap_type() == CapType::CNode {
                let next_cnode_addr = slot.cap.obj_ptr();
                if next_cnode_addr == VirtAddr::null() {
                    return None;
                }
                let next_cnode = unsafe { next_cnode_addr.as_ref::<CNode>() };
                // 递归调用
                return unsafe { next_cnode.lookup_slot_ptr(next_cptr) };
            } else {
                return None;
            }
        }
    }

    pub fn insert(&self, cptr: CapPtr, cap: &Capability) -> bool {
        if cptr.is_null() {
            log!("cnode: Insert failed, null CPtr");
            return false;
        }

        // 1. Lock Current Node
        let _guard = self.metadata().lock.lock();

        let index = cptr.index();
        let next_cptr = cptr.next();

        if index >= CNODE_SLOTS || index == 0 {
            log!("cnode: Insert failed, invalid index {} in CPtr {}", index, cptr);
            return false;
        }

        // 获取 Slot 指针 (通过 UnsafeCell 合法获取可变引用)
        let slot_ptr = unsafe { self.get_slot_ptr(index) };
        let slot = unsafe { &mut *slot_ptr };

        if next_cptr.is_null() {
            // Found leaf - check if empty
            if slot.cap.cap_type() != CapType::Empty {
                log!("cnode: Insert failed, slot not empty at index {}", index);
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
                    log!("cnode: Insert failed, next CNode pointer is null at index {}", index);
                    return false;
                }
                let next_cnode = unsafe { next_cnode_addr.as_ref::<CNode>() };
                next_cnode.insert(next_cptr, cap)
            } else {
                log!(
                    "cnode: Insert failed, expected CNode at index {}, found {:?}",
                    index,
                    current_cap.cap_type()
                );
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
        let self_lock_ptr = VirtAddr::from(&self.metadata().lock as *const SpinLock<()> as usize);

        let _parent_guard = if parent_lock_ptr != self_lock_ptr {
            unsafe { Some(parent_lock_ptr.as_ref::<SpinLock<()>>().lock()) }
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
            let next_sib_slot = unsafe { &mut *old_first_child.as_mut::<Slot>() };

            // Need to lock Next Sibling?
            let sib_lock_ptr = next_sib_slot.cnode_lock;
            let _sib_guard = if sib_lock_ptr != self_lock_ptr && sib_lock_ptr != parent_lock_ptr {
                unsafe { Some(sib_lock_ptr.as_ref::<SpinLock<()>>().lock()) }
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
        let rc = self.ref_count().load(Ordering::Relaxed);
        printk!("CNode ptr: {:#x}, RefCount: {}\n", self as *const _ as usize, rc);
        self.debug_print_recursive(0);
    }

    fn debug_print_recursive(&self, depth: usize) {
        if depth > 8 {
            printk!("... (Recursion limit reached)\n");
            return;
        }

        // Slot 0 is reserved for Metadata, iterate from 1
        for i in 1..CNODE_SLOTS {
            let slot = unsafe { &*self.get_slot_ptr(i) };

            if slot.cap.cap_type() == CapType::Empty {
                continue;
            }

            for _ in 0..depth {
                printk!("  ");
            }
            printk!("[{:#x}] {}, {}\n", i, slot.cap, slot.cnode_lock);

            if slot.cap.cap_type() == CapType::CNode {
                let child_ptr = slot.cap.obj_ptr();
                let self_ptr = VirtAddr::from(self as *const CNode as usize);

                if child_ptr != VirtAddr::null() && child_ptr != self_ptr {
                    unsafe { child_ptr.as_ref::<CNode>().debug_print_recursive(depth + 1) };
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
        let slot_ptr = unsafe { self.get_slot_ptr(index) };
        let slot = unsafe { &*slot_ptr };

        if next_cptr.is_null() {
            return true;
        } else {
            if slot.cap.cap_type() == CapType::CNode {
                let next_cnode_addr = slot.cap.obj_ptr();
                if next_cnode_addr == VirtAddr::null() {
                    return false;
                }
                let next_cnode = unsafe { next_cnode_addr.as_ref::<CNode>() };
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
        let child_slot = unsafe { &mut *child_addr.as_mut::<Slot>() };

        let next_sibling = child_slot.cdt.next_sibling;

        // Decouple lifetime for recursion
        let lock_ref = unsafe { child_slot.cnode_lock.as_ref::<SpinLock<()>>() };
        let _guard = lock_ref.lock();

        revoke_recursive(child_slot);

        // Clear slot
        child_slot.cdt = CDTNode::new();
        child_slot.cap = Capability::empty();
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

    if prev != VirtAddr::null() {
        let prev_slot = unsafe { &mut *prev.as_mut::<Slot>() };
        // Lock Prev
        let lock_ptr = prev_slot.cnode_lock;
        let _guard = if lock_ptr != self_lock_ptr {
            unsafe { Some((lock_ptr.as_mut::<SpinLock<()>>()).lock()) }
        } else {
            None
        };

        prev_slot.cdt.next_sibling = next;
    } else if parent != VirtAddr::null() {
        let parent_slot = unsafe { &mut *parent.as_mut::<Slot>() };
        // Lock Parent
        let lock_ptr = parent_slot.cnode_lock;
        let _guard = if lock_ptr != self_lock_ptr {
            unsafe { Some((lock_ptr.as_mut::<SpinLock<()>>()).lock()) }
        } else {
            None
        };

        parent_slot.cdt.first_child = next;
    }

    if next != VirtAddr::null() {
        let next_slot = unsafe { &mut *next.as_mut::<Slot>() };
        // Lock Next
        let lock_ptr = next_slot.cnode_lock;
        let _guard = if lock_ptr != self_lock_ptr {
            unsafe { Some((lock_ptr.as_mut::<SpinLock<()>>()).lock()) }
        } else {
            None
        };

        next_slot.cdt.prev_sibling = prev;
    }

    // 3. 清空槽位 (触发 Capability::drop)
    slot.cap = Capability::empty();
    slot.cdt = CDTNode::new();
}
