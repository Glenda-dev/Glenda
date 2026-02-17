use super::{CapType, Capability};
use crate::error::Error;
use crate::hal::mem::PGSIZE;
use crate::mem::VirtAddr;
// use crate::printk;
use crate::sync::{SpinLock, SpinLockGuard};
use core::cell::UnsafeCell;
use core::fmt::{Debug, Display};
use core::sync::atomic::{AtomicUsize, Ordering};

pub const SLOT_SIZE: usize = core::mem::size_of::<Slot>();
pub const CNODE_SIZE: usize = core::mem::size_of::<CNode>();
pub const CNODE_BITS: usize = 8; // 256 slots per CNode
pub const CNODE_SLOTS: usize = 1 << CNODE_BITS;
pub const CNODE_MASK: usize = CNODE_SLOTS - 1;
pub const CNODE_PAGES: usize = (CNODE_SIZE + PGSIZE - 1) / PGSIZE; // CNode 占用的页数

/// 每8位作为一层的索引号
#[repr(C)]
#[derive(Clone, Copy)]
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

    pub fn len(&self) -> usize {
        if self.0 >> CNODE_BITS == 0 {
            1
        } else if self.0 >> (CNODE_BITS * 2) == 0 {
            2
        } else if self.0 >> (CNODE_BITS * 3) == 0 {
            3
        } else if self.0 >> (CNODE_BITS * 4) == 0 {
            4
        } else if self.0 >> (CNODE_BITS * 5) == 0 {
            5
        } else if self.0 >> (CNODE_BITS * 6) == 0 {
            6
        } else if self.0 >> (CNODE_BITS * 7) == 0 {
            7
        } else {
            8
        }
    }

    pub fn concat(root: CapPtr, ptr: CapPtr) -> CapPtr {
        if root.is_null() {
            return ptr;
        }
        let root_len = root.len();
        CapPtr(root.0 | ptr.0 << (root_len * CNODE_BITS))
    }
}

impl Display for CapPtr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

impl Debug for CapPtr {
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
    pub unsafe fn get_slot_ptr(&self, index: usize) -> *mut Slot {
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

    pub fn insert(&self, cptr: CapPtr, cap: &Capability) -> Result<(), Error> {
        if cptr.is_null() {
            error!("cnode: Insert failed, null CPtr");
            return Err(Error::InvalidSlot);
        }

        // 1. Lock Current Node
        let _guard = self.metadata().lock.lock();

        let index = cptr.index();
        let next_cptr = cptr.next();

        if index >= CNODE_SLOTS || index == 0 {
            error!("cnode: Insert failed, invalid index {} in CPtr {}", index, cptr);
            return Err(Error::InvalidSlot);
        }

        // 获取 Slot 指针 (通过 UnsafeCell 合法获取可变引用)
        let slot_ptr = unsafe { self.get_slot_ptr(index) };
        let slot = unsafe { &mut *slot_ptr };

        if next_cptr.is_null() {
            // Found leaf - check if empty
            if slot.cap.cap_type() != CapType::Empty {
                error!("cnode: Insert failed, slot not empty at index {}", index);
                return Err(Error::AlreadyExists);
            }
            slot.cap = cap.clone();
            Ok(())
        } else {
            // Recurse
            let current_cap = slot.cap.clone();
            drop(_guard); // Unlock before recursing

            if current_cap.cap_type() == CapType::CNode {
                let next_cnode_addr = current_cap.obj_ptr();
                if next_cnode_addr == VirtAddr::null() {
                    error!("cnode: Insert failed, next CNode pointer is null at index {}", index);
                    return Err(Error::InvalidCapability);
                }
                let next_cnode = unsafe { next_cnode_addr.as_ref::<CNode>() };
                next_cnode.insert(next_cptr, cap)
            } else {
                error!(
                    "cnode: Insert failed, expected CNode at index {}, found {:?}",
                    index,
                    current_cap.cap_type()
                );
                Err(Error::InvalidType)
            }
        }
    }

    pub fn move_cap(&self, src_cptr: CapPtr, dest_cptr: CapPtr) -> Result<(), Error> {
        if src_cptr.is_null() || dest_cptr.is_null() {
            return Err(Error::InvalidSlot);
        }

        // 1. 获取 Source Slot 和 Dest Slot 指针
        let src_slot_ptr = unsafe { self.lookup_slot_ptr(src_cptr).ok_or(Error::InvalidSlot)? };
        let dest_slot_ptr = unsafe { self.lookup_slot_ptr(dest_cptr).ok_or(Error::InvalidSlot)? };

        if src_slot_ptr == dest_slot_ptr {
            return Ok(());
        }

        // 2. 加锁 (按位顺序以避免死锁)
        let src_slot = unsafe { &mut *src_slot_ptr };
        let dest_slot = unsafe { &mut *dest_slot_ptr };

        let src_lock_ptr = src_slot.cnode_lock;
        let dest_lock_ptr = dest_slot.cnode_lock;

        let _guards = if src_lock_ptr.as_usize() < dest_lock_ptr.as_usize() {
            let g1 = unsafe { src_lock_ptr.as_ref::<SpinLock<()>>().lock() };
            let g2 = unsafe { dest_lock_ptr.as_ref::<SpinLock<()>>().lock() };
            (Some(g1), Some(g2))
        } else if src_lock_ptr.as_usize() > dest_lock_ptr.as_usize() {
            let g1 = unsafe { dest_lock_ptr.as_ref::<SpinLock<()>>().lock() };
            let g2 = unsafe { src_lock_ptr.as_ref::<SpinLock<()>>().lock() };
            (Some(g1), Some(g2))
        } else {
            let g = unsafe { src_lock_ptr.as_ref::<SpinLock<()>>().lock() };
            (Some(g), None)
        };

        // 3. 检查状态
        if src_slot.cap.cap_type() == CapType::Empty {
            error!("cnode: Move failed, source slot is empty src_cptr={}", src_cptr);
            return Err(Error::InvalidCapability);
        }
        if dest_slot.cap.cap_type() != CapType::Empty {
            error!(
                "cnode: Move failed, destination slot not empty dest_cptr={}, dest_cap={:?}",
                dest_cptr,
                dest_slot.cap.cap_type()
            );
            return Err(Error::AlreadyExists);
        }

        // 4. 执行移动
        // 直接位拷贝，不触发 Clone/Drop, 从而保持引用计数不变
        unsafe {
            core::ptr::copy_nonoverlapping(&src_slot.cap, &mut dest_slot.cap, 1);
            core::ptr::copy_nonoverlapping(&src_slot.cdt, &mut dest_slot.cdt, 1);

            // 清理原槽位 (手动构造，不触发旧值的 Drop)
            core::ptr::write(&mut src_slot.cap, Capability::empty());
            core::ptr::write(&mut src_slot.cdt, CDTNode::new());
        }

        // 5. 更新 CDT 树关系
        let dest_ptr_val = dest_slot_ptr as usize;
        let cdt = &dest_slot.cdt;

        // 更新父节点的 first_child 或前一个兄弟的 next_sibling
        if cdt.prev_sibling != VirtAddr::null() {
            let prev_slot = unsafe { &mut *cdt.prev_sibling.as_mut::<Slot>() };
            prev_slot.cdt.next_sibling = VirtAddr::from(dest_ptr_val);
        } else if cdt.parent != VirtAddr::null() {
            let parent_slot = unsafe { &mut *cdt.parent.as_mut::<Slot>() };
            parent_slot.cdt.first_child = VirtAddr::from(dest_ptr_val);
        }

        // 更新下一个兄弟的 prev_sibling
        if cdt.next_sibling != VirtAddr::null() {
            let next_slot = unsafe { &mut *cdt.next_sibling.as_mut::<Slot>() };
            next_slot.cdt.prev_sibling = VirtAddr::from(dest_ptr_val);
        }

        // 更新所有子节点的 parent
        let mut child_addr = cdt.first_child;
        while child_addr != VirtAddr::null() {
            let child_slot = unsafe { &mut *child_addr.as_mut::<Slot>() };
            child_slot.cdt.parent = VirtAddr::from(dest_ptr_val);
            child_addr = child_slot.cdt.next_sibling;
        }

        Ok(())
    }

    pub fn insert_child(
        &mut self,
        cptr: CapPtr,
        cap: &Capability,
        parent_slot: *mut Slot,
    ) -> Result<(), Error> {
        // 1. Lock Self
        let _self_lock_guard = self.metadata().lock.lock();

        if cptr.is_null() {
            error!("CNode::insert_child failed: null CPtr");
            return Err(Error::InvalidSlot);
        }
        // Unsafe lookup without lock is fine because we hold lock
        let slot = match unsafe { self.lookup_slot_ptr(cptr) } {
            None => {
                error!("CNode::insert_child failed: lookup_slot_ptr returned None cptr={}", cptr);
                return Err(Error::InvalidSlot);
            }
            Some(ptr) => unsafe { &mut *ptr },
        };

        // 必须确保目标槽位为空，否则会破坏 CDT
        if slot.cap.cap_type() != CapType::Empty {
            error!("CNode::insert_child failed: target slot not empty cptr={}", cptr);
            return Err(Error::AlreadyExists);
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
        Ok(())
    }

    pub fn revoke(&mut self, cptr: CapPtr) -> Result<(), Error> {
        let _guard = self.metadata().lock.lock();
        let slot = match unsafe { self.lookup_slot_ptr(cptr) } {
            None => return Err(Error::InvalidSlot),
            Some(ptr) => unsafe { &mut *ptr },
        };
        revoke_recursive(slot);
        Ok(())
    }

    pub fn recycle(&mut self, cptr: CapPtr) -> Result<usize, Error> {
        let _guard = self.metadata().lock.lock();
        let slot = match unsafe { self.lookup_slot_ptr(cptr) } {
            None => return Err(Error::InvalidSlot),
            Some(ptr) => unsafe { &mut *ptr },
        };

        // 1. Revoke children first to ensure no one else is using derived caps
        revoke_recursive(slot);

        // 2. Try to recycle current cap
        if let Some(new_cap) = slot.cap.recycle() {
            let pages = match new_cap.cap_type() {
                CapType::Untyped => new_cap.get_data() & 0x1FFFFFF,
                _ => 0,
            };
            slot.cap = new_cap;
            slot.cdt.first_child = VirtAddr::null();
            Ok(pages)
        } else {
            Err(Error::InvalidCapability)
        }
    }

    pub fn delete(&mut self, cptr: CapPtr) -> Result<(), Error> {
        let _guard = self.metadata().lock.lock();
        if cptr.is_null() {
            return Err(Error::InvalidSlot);
        }
        let slot = match unsafe { self.lookup_slot_ptr(cptr) } {
            None => return Err(Error::InvalidSlot),
            Some(ptr) => unsafe { &mut *ptr },
        };
        delete_recursive(slot);
        Ok(())
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
    log!("cap: Revoking slot {:p} with cap {:?}, CDT: {:?}", slot as *mut Slot, slot.cap, slot.cdt);
    let mut child_addr = slot.cdt.first_child;
    slot.cdt.first_child = VirtAddr::null();

    // The caller (or parent recursion level) guarantees that `slot`'s CNode is locked.
    let current_cnode_lock = slot.cnode_lock;

    while child_addr != VirtAddr::null() {
        let child_slot = unsafe { &mut *child_addr.as_mut::<Slot>() };

        let next_sibling = child_slot.cdt.next_sibling;

        let child_cnode_lock = child_slot.cnode_lock;

        // If the child is in a different CNode, we must lock it.
        // If it's in the same CNode, simply proceeding is fine because we already hold the lock.
        // WARNING: This assumes a strict tree hierarchy with no cycles, which CDT guarantees.
        // It also assumes we don't hold any OTHER locks that could cause AB-BA deadlocks if the tree spans multiple CNodes
        // in a weird way. But CDT is strictly hierarchical.

        let _guard = if child_cnode_lock != current_cnode_lock {
            let lock_ref = unsafe { child_cnode_lock.as_ref::<SpinLock<()>>() };
            // Check if we already hold this lock (re-entrancy check)
            // This can happen if the tree loops back to a parent CNode (should not happen in CDT)
            // OR if we hold it from a higher level operation (e.g. Move between two CNodes)
            if lock_ref.holding() { None } else { Some(lock_ref.lock()) }
        } else {
            None
        };

        revoke_recursive(child_slot);

        // Clear slot
        child_slot.cdt = CDTNode::new();
        child_slot.cap = Capability::empty();
        child_addr = next_sibling;
    }
}

fn delete_recursive(slot: &mut Slot) {
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
            let lock_ref = unsafe { lock_ptr.as_ref::<SpinLock<()>>() };
            if lock_ref.holding() { None } else { Some(lock_ref.lock()) }
        } else {
            None
        };

        prev_slot.cdt.next_sibling = next;
    } else if parent != VirtAddr::null() {
        let parent_slot = unsafe { &mut *parent.as_mut::<Slot>() };
        // Lock Parent
        let lock_ptr = parent_slot.cnode_lock;
        let _guard = if lock_ptr != self_lock_ptr {
            let lock_ref = unsafe { lock_ptr.as_ref::<SpinLock<()>>() };
            if lock_ref.holding() { None } else { Some(lock_ref.lock()) }
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
            let lock_ref = unsafe { lock_ptr.as_ref::<SpinLock<()>>() };
            if lock_ref.holding() { None } else { Some(lock_ref.lock()) }
        } else {
            None
        };

        next_slot.cdt.prev_sibling = prev;
    }

    // 3. 清空槽位 (触发 Capability::drop)
    slot.cap = Capability::empty();
    slot.cdt = CDTNode::new();
}
