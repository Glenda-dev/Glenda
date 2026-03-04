use crate::hal;
use crate::hal::mem::MAX_ASID;
use crate::sync::SpinLock;

/// ASID 令牌：这是 VSpace 应该持有的东西
/// 包含具体的 ID 和它所属的代数
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Asid {
    pub id: u16,
    pub generation: u64,
}

impl Asid {
    pub const fn default() -> Self {
        Self { id: 0, generation: 0 }
    }
    pub const fn from(id: u16, generation: u64) -> Asid {
        Asid { id, generation }
    }
}

/// ASID 管理器 (单例)
static ASID_MANAGER: SpinLock<AsidManager> = SpinLock::new(AsidManager::new());

struct AsidManager {
    /// 当前可分配的高水位线 (0..MAX_ASID)
    current_asid: u16,
    /// 全局代际计数器
    generation: u64,
}

impl AsidManager {
    const fn new() -> Self {
        Self {
            current_asid: 0,
            generation: 1, // 从 1 开始，0 表示未初始化
        }
    }

    /// 分配一个新的 ASID
    fn alloc(&mut self) -> Asid {
        if (self.current_asid as usize) < MAX_ASID - 1 {
            self.current_asid += 1;
        } else {
            // ASID 耗尽，进入下一代，重置计数器
            self.generation += 1;
            self.current_asid = 1;

            // 关键：刷新所有 TLB，因为旧代的 ASID 1 现在要被复用了
            hal::mem::flush_tlb(None, 0, None);
        }

        Asid { id: self.current_asid, generation: self.generation }
    }

    /// 检查 Token 是否对当前硬件代数有效
    fn check(&self, token: Asid) -> bool {
        token.id != 0 && token.generation == self.generation
    }
}

// --- Public Interface ---

pub fn alloc() -> Asid {
    let mut manager = ASID_MANAGER.lock();
    manager.alloc()
}

pub fn check(token: Asid) -> bool {
    let manager = ASID_MANAGER.lock();
    manager.check(token)
}
