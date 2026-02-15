use super::CapType;
use super::Rights;
use crate::cap::Badge;
use crate::cap::cnode::CNode;
use crate::hal::mem::{ASID_MASK, PGSIZE};
use crate::ipc::Endpoint;
use crate::mem::PageTable;
use crate::mem::addr::{phys_to_virt, virt_to_phys};
use crate::mem::{PhysAddr, PhysFrame, UntypedRegion, VirtAddr};
use crate::proc::TCB;
use crate::proc::asid::Asid;
use core::fmt::Display;
use core::sync::atomic::Ordering;
use num_enum::FromPrimitive;

/// Capability (Compressed to 16 bytes)
/// Word 0: Object Pointer / Data
/// Word 1: Metadata (Type, Rights, Badge, etc.)
#[repr(C)]
#[derive(Debug)]
pub struct Capability {
    words: [usize; 2],
}

impl Display for Capability {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let cap_type = self.cap_type();
        let mut s = f.debug_struct("Capability");
        s.field("type", &cap_type);
        s.field("rights", &self.rights().bits());

        match cap_type {
            CapType::Untyped => {
                let pages = (self.words[1] >> 13) & 0x1FFFFFF;
                let watermark = (self.words[1] >> 38) & 0x1FFFFFF;
                s.field("start_paddr", &PhysAddr::from(self.words[0]));
                s.field("total_pages", &pages);
                s.field("watermark", &watermark);
            }
            CapType::TCB => {
                s.field("tcb_ptr", &VirtAddr::from(self.words[0]));
            }
            CapType::Endpoint => {
                s.field("ep_ptr", &VirtAddr::from(self.words[0]));
                s.field("badge", &self.get_badge());
            }
            CapType::Reply => {
                s.field("tcb_ptr", &VirtAddr::from(self.words[0]));
            }
            CapType::Frame => {
                s.field("paddr", &PhysAddr::from(self.words[0]));
                s.field("pages", &(self.words[1] >> DATA_SHIFT));
            }
            CapType::PageTable => {
                s.field("paddr", &PhysAddr::from(self.words[0]));
                s.field("level", &self.pt_level());
            }
            CapType::CNode => {
                s.field("paddr", &PhysAddr::from(self.words[0]));
            }
            CapType::IrqHandler => {
                s.field("irq", &self.get_badge().get());
            }
            CapType::VSpace => {
                let (paddr, id) = self.vspace_info();
                let asid = id.id;
                let generation = id.generation;
                s.field("paddr", &paddr);
                s.field("asid", &asid);
                s.field("gen", &generation);
            }
            CapType::Mmio => {
                s.field("mmio", &"global");
            }
            _ => {}
        }
        s.finish()
    }
}

impl Clone for Capability {
    fn clone(&self) -> Self {
        self.inc_ref();
        Self { words: self.words }
    }
}

// Bitfield constants
pub const TYPE_MASK: usize = 0x1F; // 5 bits (32 types)
pub const RIGHTS_SHIFT: usize = 5;
pub const RIGHTS_MASK: usize = 0xFF; // 8 bits
pub const DATA_SHIFT: usize = 13;
const ASID_BITS: usize = 16;

impl Capability {
    // Helper to extract type
    pub fn cap_type(&self) -> CapType {
        let tag = self.words[1] & TYPE_MASK;
        CapType::from_primitive(tag)
    }

    // Helper to extract Rights
    pub const fn rights(&self) -> Rights {
        Rights::from_bits_truncate(((self.words[1] >> RIGHTS_SHIFT) & RIGHTS_MASK) as u8)
    }

    fn inc_ref(&self) {
        match self.cap_type() {
            CapType::TCB => {
                let tcb_ptr = VirtAddr::from(self.words[0]);
                let tcb = unsafe { tcb_ptr.as_ref::<TCB>() };
                tcb.ref_count.fetch_add(1, Ordering::Relaxed);
            }
            CapType::Endpoint => {
                let ep_ptr = VirtAddr::from(self.words[0]);
                let ep = unsafe { ep_ptr.as_ref::<Endpoint>() };
                ep.ref_count.fetch_add(1, Ordering::Relaxed);
            }
            CapType::CNode => {
                let vaddr = VirtAddr::from(self.words[0]);
                let header = unsafe { vaddr.as_ref::<CNode>() };
                header.ref_count().fetch_add(1, Ordering::Relaxed);
            }
            // 其他类型暂不引用计数
            _ => {}
        }
    }

    pub const fn empty() -> Self {
        Self { words: [0, 0] }
    }

    pub fn mint(&self, badge: Badge, rights: Rights) -> Self {
        let mut new_cap = self.clone();

        // 只有原始能力未标记，且新标记有效时，才允许注入
        // 目前 Endpoint 和 IrqHandler 支持 Badge
        if self.cap_type() == CapType::Endpoint || self.cap_type() == CapType::IrqHandler {
            let current_badge = self.get_badge();
            if current_badge.is_null() && !badge.is_null() {
                let b = badge;
                // 清除旧 Badge (虽然是0) 并设置新 Badge
                new_cap.set_badge(b);
            }
        }

        // 更新权限
        let current_rights = new_cap.rights();
        let new_rights = current_rights.intersection(rights);

        // 清除旧权限位并设置新权限
        let rights_clear_mask = !(RIGHTS_MASK << RIGHTS_SHIFT);
        new_cap.words[1] =
            (new_cap.words[1] & rights_clear_mask) | ((new_rights.bits() as usize) << RIGHTS_SHIFT);

        new_cap
    }

    #[inline(always)]
    pub fn obj_ptr(&self) -> VirtAddr {
        match self.cap_type() {
            CapType::TCB => VirtAddr::from(self.words[0]),
            CapType::Endpoint => VirtAddr::from(self.words[0]),
            CapType::Reply => VirtAddr::from(self.words[0]),
            CapType::CNode => VirtAddr::from(self.words[0]),
            CapType::Untyped => phys_to_virt(PhysAddr::from(self.words[0])),
            CapType::Frame => phys_to_virt(PhysAddr::from(self.words[0])),
            CapType::PageTable => phys_to_virt(PhysAddr::from(self.words[0])),
            CapType::VSpace => phys_to_virt(PhysAddr::from(self.words[0])),
            _ => VirtAddr::null(),
        }
    }

    #[inline(always)]
    pub fn paddr(&self) -> PhysAddr {
        match self.cap_type() {
            CapType::Untyped => PhysAddr::from(self.words[0]),
            CapType::Frame => PhysAddr::from(self.words[0]),
            CapType::PageTable => PhysAddr::from(self.words[0]),
            CapType::VSpace => PhysAddr::from(self.words[0]),
            _ => PhysAddr::null(),
        }
    }

    #[inline(always)]
    pub const fn value(&self) -> usize {
        self.words[0]
    }

    /// 检查是否拥有指定权限
    pub const fn has_rights(&self, required: Rights) -> bool {
        self.rights().contains(required)
    }

    /// 检查是否允许 Invoke (Call)
    pub const fn can_invoke(&self) -> bool {
        self.has_rights(Rights::CALL)
    }

    /// 检查是否允许 Grant (传递)
    pub const fn can_grant(&self) -> bool {
        self.has_rights(Rights::GRANT)
    }

    pub const fn get_data(&self) -> usize {
        self.words[1] >> DATA_SHIFT
    }

    pub fn set_data(&mut self, data: usize) {
        let mask = !((!0usize) << DATA_SHIFT);
        self.words[1] = (self.words[1] & mask) | (data << DATA_SHIFT);
    }

    pub fn create_untyped(untyped: &UntypedRegion, rights: Rights) -> Self {
        let w0 = untyped.start.as_usize();
        // Word 1: Type (5) | Rights (8) | TotalPages (25) | FreePages (25)
        let w1 = (CapType::Untyped as usize) & TYPE_MASK
            | ((rights.bits() as usize) & RIGHTS_MASK) << RIGHTS_SHIFT
            | ((untyped.pages & 0x1FFFFFF) << 13)
            | (0 << 38); // watermark starts at 0

        Self { words: [w0, w1] }
    }

    pub fn create_tcb(tcb: &TCB, rights: Rights) -> Self {
        tcb.ref_count.fetch_add(1, Ordering::Relaxed);
        let tcb_ptr = VirtAddr::from(tcb as *const TCB as usize);
        let w0 = tcb_ptr.as_usize();
        let w1 = (CapType::TCB as usize) & TYPE_MASK
            | ((rights.bits() as usize) & RIGHTS_MASK) << RIGHTS_SHIFT;
        Self { words: [w0, w1] }
    }

    pub fn create_endpoint(ep: &Endpoint, rights: Rights) -> Self {
        ep.ref_count.fetch_add(1, Ordering::Relaxed);
        let ep_ptr = VirtAddr::from(ep as *const Endpoint as usize);
        let w0 = ep_ptr.as_usize();
        let w1 = (CapType::Endpoint as usize) & TYPE_MASK
            | ((rights.bits() as usize) & RIGHTS_MASK) << RIGHTS_SHIFT;
        Self { words: [w0, w1] }
    }

    pub fn create_reply(tcb: &TCB, rights: Rights) -> Self {
        tcb.ref_count.fetch_add(1, Ordering::Relaxed);
        let tcb_ptr = VirtAddr::from(tcb as *const TCB as usize);
        let w0 = tcb_ptr.as_usize();
        let w1 = (CapType::Reply as usize) & TYPE_MASK
            | ((rights.bits() as usize) & RIGHTS_MASK) << RIGHTS_SHIFT;
        Self { words: [w0, w1] }
    }

    pub fn create_frame(frame: &PhysFrame, rights: Rights) -> Self {
        assert!(frame.paddr.is_aligned(PGSIZE), "Frame paddr must be page-aligned");
        let w0 = frame.paddr.as_usize();
        let w1 = (CapType::Frame as usize) & TYPE_MASK
            | ((rights.bits() as usize) & RIGHTS_MASK) << RIGHTS_SHIFT
            | (frame.pages << DATA_SHIFT);
        Self { words: [w0, w1] }
    }

    pub fn create_pagetable(pt: &PageTable, level: usize, rights: Rights) -> Self {
        let paddr = virt_to_phys(VirtAddr::from(pt as *const PageTable as usize));
        let w0 = paddr.as_usize();
        let w1 = (CapType::PageTable as usize) & TYPE_MASK
            | ((rights.bits() as usize) & RIGHTS_MASK) << RIGHTS_SHIFT
            | (level << DATA_SHIFT);
        Self { words: [w0, w1] }
    }

    pub fn create_cnode(cnode: &CNode, rights: Rights) -> Self {
        cnode.ref_count().fetch_add(1, Ordering::Relaxed);
        let cnode_ptr = VirtAddr::from(cnode as *const CNode as usize);
        let w0 = cnode_ptr.as_usize();
        let w1 = (CapType::CNode as usize) & TYPE_MASK
            | ((rights.bits() as usize) & RIGHTS_MASK) << RIGHTS_SHIFT;
        Self { words: [w0, w1] }
    }

    pub fn create_irqhandler(rights: Rights) -> Self {
        let w0 = 0;
        let w1 = (CapType::IrqHandler as usize) & TYPE_MASK
            | ((rights.bits() as usize) & RIGHTS_MASK) << RIGHTS_SHIFT;
        Self { words: [w0, w1] }
    }

    pub fn create_kernel(rights: Rights) -> Self {
        let w0 = 0;
        let w1 = (CapType::Kernel as usize) & TYPE_MASK
            | ((rights.bits() as usize) & RIGHTS_MASK) << RIGHTS_SHIFT;
        Self { words: [w0, w1] }
    }
    pub fn create_vspace(pt: &PageTable, asid: Asid, rights: Rights) -> Self {
        let asid_val = asid.id as usize & ASID_MASK;
        let paddr = virt_to_phys(VirtAddr::from(pt as *const PageTable as usize));
        // 获取 Generation (保留低 35 位: 64 - 13 - 16 = 35)
        // Bit 29 到 63
        let gen_val = asid.generation as usize; // 高位会被移位自动处理，或者可以按需在这里mask
        let w0 = paddr.as_usize();
        let w1 = (CapType::VSpace as usize) & TYPE_MASK
            | ((rights.bits() as usize) & RIGHTS_MASK) << RIGHTS_SHIFT
            | (asid_val << DATA_SHIFT)
            | (gen_val << (DATA_SHIFT + ASID_BITS));
        Self { words: [w0, w1] }
    }

    pub fn create_mmio(rights: Rights) -> Self {
        let w0 = 0;
        let w1 = (CapType::Mmio as usize) & TYPE_MASK
            | ((rights.bits() as usize) & RIGHTS_MASK) << RIGHTS_SHIFT;
        Self { words: [w0, w1] }
    }

    pub fn get_badge(&self) -> Badge {
        if self.cap_type() == CapType::Endpoint || self.cap_type() == CapType::IrqHandler {
            Badge::from(self.words[1] >> DATA_SHIFT)
        } else {
            Badge::null()
        }
    }

    /// 检查是否已被标记
    pub fn is_badged(&self) -> bool {
        !self.get_badge().is_null()
    }

    pub fn set_badge(&mut self, badge: Badge) {
        if self.cap_type() == CapType::Endpoint || self.cap_type() == CapType::IrqHandler {
            let mask = !((!0usize) << DATA_SHIFT);
            self.words[1] = (self.words[1] & mask) | (badge.get() << DATA_SHIFT);
        }
    }

    pub fn vspace_info(&self) -> (PhysAddr, Asid) {
        let paddr = PhysAddr::from(self.words[0]);

        let asid = (self.words[1] >> DATA_SHIFT) & ASID_MASK;
        let generation = self.words[1] >> (DATA_SHIFT + ASID_BITS);
        (paddr, Asid::from(asid as u16, generation as u64))
    }

    pub fn is_null(&self) -> bool {
        self.cap_type() == CapType::Empty
    }

    pub fn pt_level(&self) -> usize {
        if self.cap_type() == CapType::PageTable { self.words[1] >> DATA_SHIFT } else { 0 }
    }
}

impl Drop for Capability {
    fn drop(&mut self) {
        match self.cap_type() {
            CapType::TCB => {
                let tcb_ptr = VirtAddr::from(self.words[0]);
                let tcb = unsafe { tcb_ptr.as_ref::<TCB>() };
                if tcb.ref_count.fetch_sub(1, Ordering::Release) == 1 {
                    core::sync::atomic::fence(Ordering::Acquire);
                    // TODO: Destroy TCB
                    // 由于 TCB 可能在调度队列中，需要将其移除
                    // 但这里不能直接调用 scheduler::remove，因为可能导致死锁或递归
                    // 通常做法是将 TCB 标记为 Zombie 或加入垃圾回收队列
                    // 简单起见，我们假设 TCB 内存由 Untyped 管理，这里只做逻辑销毁
                    log!("cap: Dropped TCB Cap at {}", tcb_ptr);
                }
            }
            CapType::Endpoint => {
                let ep_ptr = VirtAddr::from(self.words[0]);
                let ep = unsafe { ep_ptr.as_ref::<Endpoint>() };
                if ep.ref_count.fetch_sub(1, Ordering::Release) == 1 {
                    core::sync::atomic::fence(Ordering::Acquire);
                    // TODO: Destroy Endpoint
                    log!("cap: Dropped Endpoint Cap at {}", ep_ptr);
                }
            }
            CapType::CNode => {
                let vaddr = VirtAddr::from(self.words[0]);
                let header = unsafe { vaddr.as_ref::<CNode>() };
                if header.ref_count().fetch_sub(1, Ordering::Release) == 1 {
                    core::sync::atomic::fence(Ordering::Acquire);
                    // TODO: Destroy CNode
                    log!("cap: Dropped CNode Cap at {}", vaddr);
                }
            }
            _ => {}
        }
    }
}
