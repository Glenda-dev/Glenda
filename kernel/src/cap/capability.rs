use super::CapType;
use super::rights;
use crate::cap::cnode::CNodeHeader;
use crate::ipc::Endpoint;
use crate::mem::PGSIZE;
use crate::mem::{PhysAddr, VirtAddr};
use crate::proc::TCB;
use core::sync::atomic::Ordering;

/// 能力 (Capability)
/// 包含对象引用、权限和 Badge
#[derive(Debug)]
pub struct Capability {
    pub object: CapType,
    pub badge: Option<usize>, // Badge 用于服务端识别客户端身份
    pub rights: u8,
}

impl Clone for Capability {
    fn clone(&self) -> Self {
        self.inc_ref();
        Self { object: self.object, badge: self.badge, rights: self.rights }
    }
}

// TODO: Use template method to reduce code duplication

impl Capability {
    fn inc_ref(&self) {
        match self.object {
            CapType::Thread { tcb_ptr } => {
                let tcb = tcb_ptr.as_ref::<TCB>();
                tcb.ref_count.fetch_add(1, Ordering::Relaxed);
            }
            CapType::Endpoint { ep_ptr } => {
                let ep = ep_ptr.as_ref::<Endpoint>();
                ep.ref_count.fetch_add(1, Ordering::Relaxed);
            }
            CapType::CNode { paddr, .. } => {
                let header = paddr.as_ref::<CNodeHeader>();
                header.ref_count.fetch_add(1, Ordering::Relaxed);
            }
            // 其他类型暂不引用计数
            _ => {}
        }
    }

    pub const fn new(object: CapType, rights: u8) -> Self {
        Self { object, badge: None, rights }
    }

    pub const fn empty() -> Self {
        Self { object: CapType::Empty, badge: None, rights: 0 }
    }

    pub fn mint(&self, badge: Option<usize>, rights: u8) -> Self {
        let mut new_cap = self.clone();

        // 规范化：0 视为 None
        let effective_badge = match badge {
            Some(0) => None,
            b => b,
        };

        // 只有原始能力未标记，且新标记有效时，才允许注入
        if self.badge.is_none() && effective_badge.is_some() {
            new_cap.badge = effective_badge;
        }

        new_cap.rights &= rights;
        new_cap
    }

    pub fn obj_ptr(&self) -> VirtAddr {
        match self.object {
            CapType::Untyped { start_paddr, .. } => start_paddr.to_va(),
            CapType::Thread { tcb_ptr } => tcb_ptr,
            CapType::Endpoint { ep_ptr } => ep_ptr,
            CapType::Reply { tcb_ptr } => tcb_ptr,
            CapType::Frame { paddr, .. } => paddr.to_va(),
            CapType::PageTable { paddr, .. } => paddr.to_va(),
            CapType::CNode { paddr, .. } => paddr.to_va(),
            _ => VirtAddr::null(),
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

    /// 获取 Badge 值，若无则返回 0
    pub fn get_badge(&self) -> usize {
        self.badge.unwrap_or(0)
    }

    /// 检查是否已被标记
    pub fn is_badged(&self) -> bool {
        self.badge.is_some()
    }

    pub fn create_untyped(start_paddr: PhysAddr, total_pages: usize, rights: u8) -> Self {
        Self::new(CapType::Untyped { start_paddr, total_pages, free_pages: 0 }, rights)
    }

    pub fn create_thread(tcb_ptr: VirtAddr, rights: u8) -> Self {
        Self::new(CapType::Thread { tcb_ptr }, rights)
    }

    pub fn create_endpoint(ep_ptr: VirtAddr, rights: u8) -> Self {
        Self::new(CapType::Endpoint { ep_ptr }, rights)
    }

    pub fn create_reply(ro_ptr: VirtAddr, rights: u8) -> Self {
        Self::new(CapType::Reply { tcb_ptr: ro_ptr }, rights)
    }

    pub fn create_frame(paddr: PhysAddr, page_count: usize, rights: u8) -> Self {
        assert!(paddr.is_aligned(PGSIZE), "Frame paddr must be page-aligned");
        Self::new(CapType::Frame { paddr, page_count }, rights)
    }

    pub fn create_pagetable(paddr: PhysAddr, level: usize, rights: u8) -> Self {
        Self::new(CapType::PageTable { paddr, level }, rights)
    }

    pub fn create_cnode(paddr: PhysAddr, rights: u8) -> Self {
        Self::new(CapType::CNode { paddr }, rights)
    }

    pub fn create_irqhandler(irq: usize, rights: u8) -> Self {
        Self::new(CapType::IrqHandler { irq }, rights)
    }
}

impl Drop for Capability {
    fn drop(&mut self) {
        match self.object {
            CapType::Thread { tcb_ptr } => {
                let tcb = tcb_ptr.as_ref::<TCB>();
                if tcb.ref_count.fetch_sub(1, Ordering::Release) == 1 {
                    core::sync::atomic::fence(Ordering::Acquire);
                    // TODO: Destroy TCB
                    // 由于 TCB 可能在调度队列中，需要将其移除
                    // 但这里不能直接调用 scheduler::remove，因为可能导致死锁或递归
                    // 通常做法是将 TCB 标记为 Zombie 或加入垃圾回收队列
                    // 简单起见，我们假设 TCB 内存由 Untyped 管理，这里只做逻辑销毁
                }
            }
            CapType::Endpoint { ep_ptr } => {
                let ep = ep_ptr.as_ref::<Endpoint>();
                if ep.ref_count.fetch_sub(1, Ordering::Release) == 1 {
                    core::sync::atomic::fence(Ordering::Acquire);
                    // TODO: Destroy Endpoint
                }
            }
            CapType::CNode { paddr, .. } => {
                let header = paddr.as_ref::<CNodeHeader>();
                if header.ref_count.fetch_sub(1, Ordering::Release) == 1 {
                    core::sync::atomic::fence(Ordering::Acquire);
                    // TODO: Destroy CNode
                }
            }
            _ => {}
        }
    }
}
