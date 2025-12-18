use super::pmem;
use super::pte;
use super::pte::{PTE_A, PTE_D, PTE_R, PTE_W, PTE_X};
use super::vm::KERNEL_PAGE_TABLE;
use super::{PGSIZE, PhysAddr, Pte, VA_MAX, VirtAddr};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use riscv::asm::sfence_vma_all;
use spin::Mutex;

// 256MB region for kernel stacks, below VA_MAX
pub const KSTACK_REGION_SIZE: usize = 0x1000_0000;
pub const KSTACK_VA_BASE: usize = VA_MAX - KSTACK_REGION_SIZE;
pub static NEXT_KSTACK_SLOT: AtomicUsize = AtomicUsize::new(0);

// Increase kernel stack to 4 pages (16KB)
pub const KSTACK_SIZE: usize = PGSIZE * 4;
// 新的状态管理：
// 1. max_slot: 记录当前分配到的最大槽位号 (水位线)
// 2. free_slots: 记录被回收的槽位号 (优先复用)
static KSTACK_ALLOCATOR: Mutex<KStackAllocator> = Mutex::new(KStackAllocator::new());

struct KStackAllocator {
    max_slot: usize,
    free_slots: Vec<usize>,
}

impl KStackAllocator {
    const fn new() -> Self {
        Self { max_slot: 0, free_slots: Vec::new() }
    }

    fn alloc(&mut self) -> Option<usize> {
        // 1. 优先从回收池中取
        if let Some(slot) = self.free_slots.pop() {
            return Some(slot);
        }

        // 2. 没有回收的，则分配新的
        let slot = self.max_slot;
        // 检查是否超出区域限制
        if slot * KSTACK_SIZE >= KSTACK_REGION_SIZE {
            return None;
        }
        self.max_slot += 1;
        Some(slot)
    }

    fn free(&mut self, slot: usize) {
        self.free_slots.push(slot);
    }
}

pub struct KernelStack {
    base: VirtAddr,
    slot: usize, // 记录自己的槽位号，以便 Drop 时归还
}

impl KernelStack {
    pub const fn new() -> Self {
        Self { base: 0, slot: 0 }
    }

    pub fn alloc() -> Option<Self> {
        // 1. 分配槽位
        let slot = KSTACK_ALLOCATOR.lock().alloc()?;

        // 计算虚拟地址基址
        // 假设 KSTACK_SIZE = 4页，我们实际占用 5页的虚拟空间 (4页栈 + 1页空洞)
        let slot_size = KSTACK_SIZE + super::PGSIZE;
        let base = KSTACK_VA_BASE + slot * slot_size + super::PGSIZE;

        // 2. 分配物理页并映射
        let mut kpt = KERNEL_PAGE_TABLE.lock();
        for i in 0..(KSTACK_SIZE / super::PGSIZE) {
            let va = base + i * super::PGSIZE;

            // 必须使用 pmem::alloc(true) 从内核区域分配
            let pa = pmem::alloc(true) as PhysAddr;
            if pa == 0 {
                // OOM 处理：回滚已分配的页
                for j in 0..i {
                    let clean_va = base + j * super::PGSIZE;
                    if let Some(pte) = kpt.walk(clean_va, false) {
                        let clean_pa = pte::get_ppn(pte as usize) << 12;
                        pmem::free(clean_pa, true);
                        kpt.unmap(clean_va, super::PGSIZE, false);
                    }
                }
                // 归还槽位
                KSTACK_ALLOCATOR.lock().free(slot);
                return None;
            }

            kpt.map(va, pa, PGSIZE, PTE_R | PTE_W | PTE_A | PTE_D);
        }

        sfence_vma_all();
        Some(Self { base, slot })
    }

    pub fn top(&self) -> VirtAddr {
        self.base + KSTACK_SIZE
    }
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        let mut kpt = KERNEL_PAGE_TABLE.lock();

        // 1. 释放物理内存并解除映射
        for i in 0..(KSTACK_SIZE / super::PGSIZE) {
            let va = self.base + i * super::PGSIZE;

            // walk 查找页表项
            if let Some(pte) = kpt.walk(va, false) {
                if pte::is_valid(pte as usize) {
                    let pa = pte::get_ppn(pte as usize) << 12;
                    // 归还物理页给 pmem
                    pmem::free(pa, true);
                    // 解除映射 (false: 不要让 unmap 再次 free，我们刚刚手动 free 了)
                    kpt.unmap(va, super::PGSIZE, false);
                }
            }
        }

        // 2. 归还虚拟地址槽位
        KSTACK_ALLOCATOR.lock().free(self.slot);

        sfence_vma_all();
    }
}
