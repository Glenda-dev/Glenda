use super::{PGSIZE, VirtAddr};
use crate::cap::Capability;
use crate::mem::pmem;
use riscv::asm::sfence_vma_all;
use spin::Mutex;

// 256MB region for kernel stacks, below VA_MAX
pub const KSTACK_REGION_SIZE: usize = 0x1000_0000;
pub const VA_MAX: usize = 1 << 38;
pub const KSTACK_VA_BASE: usize = VA_MAX - KSTACK_REGION_SIZE;

// Increase kernel stack to 4 pages (16KB)
pub const KSTACK_SIZE: usize = PGSIZE * 4;
const KSTACK_PAGES: usize = KSTACK_SIZE / PGSIZE;
const SLOT_SIZE: usize = KSTACK_SIZE + PGSIZE;
const MAX_SLOTS: usize = KSTACK_REGION_SIZE / SLOT_SIZE;
const BITMAP_SIZE: usize = (MAX_SLOTS + 63) / 64;

static KSTACK_ALLOCATOR: Mutex<KStackAllocator> = Mutex::new(KStackAllocator::new());

struct KStackAllocator {
    bitmap: [u64; BITMAP_SIZE],
    hint: usize,
}

impl KStackAllocator {
    const fn new() -> Self {
        Self { bitmap: [0; BITMAP_SIZE], hint: 0 }
    }

    fn alloc(&mut self) -> Option<usize> {
        for i in 0..MAX_SLOTS {
            let idx = (self.hint + i) % MAX_SLOTS;
            let word_idx = idx / 64;
            let bit_idx = idx % 64;

            if (self.bitmap[word_idx] & (1 << bit_idx)) == 0 {
                self.bitmap[word_idx] |= 1 << bit_idx;
                self.hint = idx + 1;
                return Some(idx);
            }
        }
        None
    }

    fn free(&mut self, slot: usize) {
        if slot < MAX_SLOTS {
            let word_idx = slot / 64;
            let bit_idx = slot % 64;
            self.bitmap[word_idx] &= !(1 << bit_idx);
            self.hint = slot;
        }
    }
}

#[derive(Debug)]
pub struct KernelStack {
    base: VirtAddr,
    slot: usize,
    // 持有 Capability 的所有权，Drop 时自动释放物理内存
    frames: [Option<Capability>; KSTACK_PAGES],
}

impl KernelStack {
    pub fn alloc() -> Option<Self> {
        // 1. 分配虚拟地址槽位
        let slot = KSTACK_ALLOCATOR.lock().alloc()?;

        // 计算虚拟地址基址 (包含 1 页 Guard Page)
        let base = VirtAddr::from(KSTACK_VA_BASE + slot * SLOT_SIZE + PGSIZE);

        let mut frames: [Option<Capability>; KSTACK_PAGES] = [const { None }; KSTACK_PAGES];

        for i in 0..KSTACK_PAGES {
            // 2. 分配物理帧 Capability
            let frame_cap = match pmem::alloc_frame_cap() {
                Some(c) => c,
                None => {
                    // 回滚：释放已分配的帧和槽位
                    // frames 数组会被 Drop，其中的 Option<Capability> 会自动释放
                    KSTACK_ALLOCATOR.lock().free(slot);
                    return None;
                }
            };
            frames[i] = Some(frame_cap);
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
