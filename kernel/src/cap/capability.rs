use super::CapType;
use super::rights;
use crate::mem::{PhysAddr, VirtAddr};
use crate::proc::TCB;

/// 能力 (Capability)
/// 包含对象引用、权限和 Badge
#[derive(Debug, Clone)]
pub struct Capability {
    pub object: CapType,
    pub badge: Option<usize>, // Badge 用于服务端识别客户端身份
    pub rights: u8,
}

impl Capability {
    pub const fn new(object: CapType, rights: u8) -> Self {
        Self { object, badge: None, rights }
    }

    pub const fn empty() -> Self {
        Self { object: CapType::Empty, badge: None, rights: 0 }
    }

    /// Mint 操作：创建一个新的 Cap，可以附加 Badge
    pub fn mint(&self, badge: Option<usize>) -> Self {
        Self {
            object: self.object,
            badge: badge.or(self.badge), // 如果已有 Badge 则保留，否则使用新的
            rights: self.rights,
        }
    }

    pub fn obj_ptr(&self) -> VirtAddr {
        match self.object {
            CapType::Untyped { start_paddr, .. } => start_paddr,
            CapType::Thread { tcb_ptr } => tcb_ptr,
            CapType::Endpoint { ep_ptr } => ep_ptr,
            CapType::Reply { tcb_ptr } => tcb_ptr,
            CapType::Frame { paddr } => paddr,
            CapType::PageTable { paddr, .. } => paddr,
            CapType::CNode { paddr, .. } => paddr,
            CapType::IrqHandler { irq } => irq,
            CapType::Empty => 0,
        }
    }

    /// 检查是否拥有指定权限
    pub fn has_rights(&self, required: u8) -> bool {
        (self.rights & required) == required
    }

    /// 检查是否允许 Invoke (Call)
    pub fn can_invoke(&self) -> bool {
        self.has_rights(rights::CALL)
    }

    /// 检查是否允许 Grant (传递)
    pub fn can_grant(&self) -> bool {
        self.has_rights(rights::GRANT)
    }

    pub fn create_untyped(start_paddr: usize, size: usize, rights: u8) -> Self {
        Self::new(CapType::Untyped { start_paddr, size }, rights)
    }

    pub fn create_thread(tcb_ptr: usize, rights: u8) -> Self {
        Self::new(CapType::Thread { tcb_ptr }, rights)
    }

    pub fn create_endpoint(ep_ptr: usize, rights: u8) -> Self {
        Self::new(CapType::Endpoint { ep_ptr }, rights)
    }

    pub fn create_reply(ro_ptr: usize, rights: u8) -> Self {
        Self::new(CapType::Reply { tcb_ptr: ro_ptr }, rights)
    }

    pub fn create_frame(paddr: usize, rights: u8) -> Self {
        Self::new(CapType::Frame { paddr }, rights)
    }

    pub fn create_pagetable(paddr: PhysAddr, level: usize, rights: u8) -> Self {
        Self::new(CapType::PageTable { paddr, level }, rights)
    }

    pub fn create_cnode(paddr: PhysAddr, bits: u8, rights: u8) -> Self {
        Self::new(CapType::CNode { paddr, bits }, rights)
    }

    pub fn create_irqhandler(irq: usize, rights: u8) -> Self {
        Self::new(CapType::IrqHandler { irq }, rights)
    }
}

impl Drop for Capability {
    fn drop(&mut self) {
        // 这里可以添加资源释放逻辑，例如回收未类型化内存等
        // 目前简化处理，什么都不做
    }
}

// TODO: 辅助函数：在当前线程的 CSpace 中查找 Cap
pub fn lookup(tcb: &TCB, cptr: usize) -> Option<Capability> {
    // 实现 CSpace 查找逻辑
    // 1. 获取 Root CNode
    // 2. 解析 cptr (通常是 index)
    // 3. 返回 Capability 副本
    // 暂时返回 None 占位，需要对接 CSpace 模块
    None
}
