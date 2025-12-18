use super::frame::PhysFrame;
use super::{PGSIZE, PhysAddr, VirtAddr};
use alloc::vec::Vec;
use riscv::asm::sfence_vma_all;
use spin::Mutex;

// 256MB region for kernel stacks, below VA_MAX
pub const KSTACK_REGION_SIZE: usize = 0x1000_0000;
pub const VA_MAX: usize = 1 << 38;
pub const KSTACK_VA_BASE: usize = VA_MAX - KSTACK_REGION_SIZE;

// Increase kernel stack to 4 pages (16KB)
pub const KSTACK_SIZE: usize = PGSIZE * 4;

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
        if let Some(slot) = self.free_slots.pop() {
            return Some(slot);
        }
        let slot = self.max_slot;
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
    slot: usize,
    // 持有 PhysFrame 的所有权，Drop 时自动释放物理内存
    frames: Vec<PhysFrame>,
}

impl KernelStack {
    pub fn alloc() -> Option<Self> {
        // 1. 分配虚拟地址槽位
        let slot = KSTACK_ALLOCATOR.lock().alloc()?;

        // 计算虚拟地址基址 (包含 1 页 Guard Page)
        let slot_size = KSTACK_SIZE + PGSIZE;
        let base = KSTACK_VA_BASE + slot * slot_size + PGSIZE;

        let mut frames = Vec::new();

        for i in 0..(KSTACK_SIZE / PGSIZE) {
            // 2. 分配物理帧
            let mut frame = match PhysFrame::alloc() {
                Some(f) => f,
                None => {
                    KSTACK_ALLOCATOR.lock().free(slot);
                    return None;
                }
            };

            // 3. 安全初始化：清零栈空间
            frame.zero();

            // 5. 保存帧所有权
            frames.push(frame);
        }

        sfence_vma_all();
        Some(Self { base, slot, frames })
    }

    pub fn top(&self) -> VirtAddr {
        self.base + KSTACK_SIZE
    }
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        // 归还虚拟地址槽位
        KSTACK_ALLOCATOR.lock().free(self.slot);

        sfence_vma_all();

        // 3. self.frames 自动 Drop，释放物理内存
    }
}
