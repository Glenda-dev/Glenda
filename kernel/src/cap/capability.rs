use super::CapType;
use super::Rights;
use crate::cap::Badge;
use crate::cap::cnode::{CNODE_PAGES, CNode};
use crate::hal::mem::{ASID_MASK, PGSIZE};
use crate::ipc::Endpoint;
use crate::mem::PageTable;
use crate::mem::addr::{phys_to_virt, virt_to_phys};
use crate::mem::{PhysAddr, PhysFrame, UntypedRegion, VirtAddr};
use crate::proc::TCB;
use crate::proc::asid::Asid;
use crate::proc::scheduler;
use core::fmt::Display;
use core::sync::atomic::Ordering;
use num_enum::FromPrimitive;

/// Capability (Compressed to 16 bytes)
/// Word 0: Object Pointer / Data
/// Word 1: Metadata (Type, Rights, Badge, etc.)
#[repr(C)]
#[derive(Debug)]
pub struct Capability {
    pub words: [usize; 2],
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
                let tcb_ptr = VirtAddr::from(self.words[0]);
                s.field("tcb_ptr", &tcb_ptr);
            }
            CapType::Frame => {
                let paddr = PhysAddr::from(self.words[0]);
                s.field("paddr", &paddr);
                s.field("pages", &((self.words[1] >> DATA_SHIFT) & 0x3FFFFFFFFFFFF));
                s.field("device", &self.is_device());
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
            CapType::Console => {
                s.field("console", &"global");
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

impl Drop for Capability {
    fn drop(&mut self) {
        self.dec_ref();
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
            CapType::TCB | CapType::Reply => {
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

    fn get_ref(&self) -> usize {
        match self.cap_type() {
            CapType::TCB | CapType::Reply => {
                let tcb_ptr = VirtAddr::from(self.words[0]);
                let tcb = unsafe { tcb_ptr.as_ref::<TCB>() };
                tcb.ref_count.load(Ordering::Relaxed)
            }
            CapType::Endpoint => {
                let ep_ptr = VirtAddr::from(self.words[0]);
                let ep = unsafe { ep_ptr.as_ref::<Endpoint>() };
                ep.ref_count.load(Ordering::Relaxed)
            }
            CapType::CNode => {
                let vaddr = VirtAddr::from(self.words[0]);
                let header = unsafe { vaddr.as_ref::<CNode>() };
                header.ref_count().load(Ordering::Relaxed)
            }
            _ => 0,
        }
    }

    fn dec_ref(&self) {
        match self.cap_type() {
            CapType::TCB | CapType::Reply => {
                let tcb_ptr = VirtAddr::from(self.words[0]);
                let tcb = unsafe { tcb_ptr.as_ref::<TCB>() };
                tcb.ref_count.fetch_sub(1, Ordering::Relaxed);
            }
            CapType::Endpoint => {
                let ep_ptr = VirtAddr::from(self.words[0]);
                let ep = unsafe { ep_ptr.as_ref::<Endpoint>() };
                ep.ref_count.fetch_sub(1, Ordering::Relaxed);
            }
            CapType::CNode => {
                let vaddr = VirtAddr::from(self.words[0]);
                let header = unsafe { vaddr.as_ref::<CNode>() };
                header.ref_count().fetch_sub(1, Ordering::Relaxed);
            }
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

    #[inline(always)]
    pub fn set_value(&mut self, value: usize) {
        self.words[0] = value;
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

    pub const fn is_device(&self) -> bool {
        (self.words[1] & TYPE_MASK) == (CapType::Frame as usize) && (self.words[1] & (1 << 63)) != 0
    }

    pub fn set_is_device(&mut self, is_device: bool) {
        if is_device {
            self.words[1] |= 1 << 63;
        } else {
            self.words[1] &= !(1 << 63);
        }
    }

    pub const fn get_data(&self) -> usize {
        let mut data = self.words[1] >> DATA_SHIFT;
        // 如果是 Frame 类型，需要排除第 63 位 (is_device)
        if (self.words[1] & TYPE_MASK) == (CapType::Frame as usize) {
            data &= 0x3FFFFFFFFFFFF;
        }
        data
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

    pub fn create_frame(frame: &PhysFrame, rights: Rights, is_device: bool) -> Self {
        assert!(frame.paddr.is_aligned(PGSIZE), "Frame paddr must be page-aligned");
        let w0 = frame.paddr.as_usize();
        let pages = frame.pages & 0x3FFFFFFFFFFFF; // 50 bits (excluding bit 63)
        let mut w1 = (CapType::Frame as usize) & TYPE_MASK
            | ((rights.bits() as usize) & RIGHTS_MASK) << RIGHTS_SHIFT
            | (pages << DATA_SHIFT);

        if is_device {
            w1 |= 1 << 63;
        }

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

    pub fn create_irqhandler(irq: usize, rights: Rights) -> Self {
        let w0 = irq;
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

    pub fn create_console(rights: Rights) -> Self {
        let w0 = 0;
        let w1 = (CapType::Console as usize) & TYPE_MASK
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

    pub fn set_asid(&mut self, asid: Asid) {
        if self.cap_type() == CapType::VSpace {
            let asid_val = asid.id as usize & ASID_MASK;
            let gen_val = asid.generation as usize;
            let mask = !((ASID_MASK << DATA_SHIFT) | (!0usize << (DATA_SHIFT + ASID_BITS)));
            self.words[1] = (self.words[1] & mask)
                | (asid_val << DATA_SHIFT)
                | (gen_val << (DATA_SHIFT + ASID_BITS));
        }
    }

    pub fn frame_info(&self) -> Option<(PhysAddr, usize, bool)> {
        if self.cap_type() == CapType::Frame {
            let paddr = PhysAddr::from(self.words[0]);
            let pages = (self.words[1] >> DATA_SHIFT) & 0x3FFFFFFFFFFFF;
            let is_device = self.is_device();
            Some((paddr, pages, is_device))
        } else {
            None
        }
    }

    pub fn is_null(&self) -> bool {
        self.cap_type() == CapType::Empty
    }

    pub fn pt_level(&self) -> usize {
        if self.cap_type() == CapType::PageTable { self.words[1] >> DATA_SHIFT } else { 0 }
    }

    pub fn recycle(&self) -> Option<Self> {
        self.dec_ref(); // 先递减引用计数，如果这是最后一个引用，才允许回收
        match self.cap_type() {
            CapType::Frame => {
                if self.is_device() {
                    return Some(Self::empty());
                }
                let paddr = PhysAddr::from(self.words[0]);
                let pages = (self.words[1] >> DATA_SHIFT) & 0x3FFFFFFFFFFFF;
                Some(Self::create_untyped(
                    &UntypedRegion { start: paddr, pages, watermark: 0 },
                    Rights::all(),
                ))
            }
            CapType::PageTable | CapType::VSpace => {
                let paddr = PhysAddr::from(self.words[0]);
                Some(Self::create_untyped(
                    &UntypedRegion { start: paddr, pages: 1, watermark: 0 },
                    Rights::all(),
                ))
            }
            CapType::CNode => {
                let vaddr = VirtAddr::from(self.words[0]);
                let cnode_mut = unsafe { &mut *(vaddr.as_usize() as *mut CNode) };
                if cnode_mut.ref_count().load(Ordering::Relaxed) > 1 {
                    warn!(
                        "cap: Cannot recycle CNode {:p}: still has {} references",
                        cnode_mut,
                        cnode_mut.ref_count().load(Ordering::Relaxed)
                    );
                    return None;
                }

                // Explicitly trigger Drop to use bitmap for efficient cleanup of used slots.
                // This will call delete_recursive on all active slots.
                unsafe { core::ptr::drop_in_place(cnode_mut) };

                let paddr = virt_to_phys(vaddr);
                Some(Self::create_untyped(
                    &UntypedRegion { start: paddr, pages: CNODE_PAGES, watermark: 0 },
                    Rights::all(),
                ))
            }
            CapType::TCB => {
                let vaddr = VirtAddr::from(self.words[0]);
                let tcb = unsafe { vaddr.as_ref::<TCB>() };
                if tcb.ref_count.load(Ordering::Relaxed) > 1 {
                    warn!(
                        "cap_recycle: Cannot recycle TCB {:p}: still has {} references",
                        tcb,
                        tcb.ref_count.load(Ordering::Relaxed)
                    );
                    return None;
                }

                // Remove from scheduler if present
                let tcb_mut = unsafe { vaddr.as_mut::<TCB>() };
                scheduler::remove_thread(tcb_mut);

                // Drop TCB to decrement ref counts of its internal capabilities (CSpace, VSpace, etc.)
                unsafe { core::ptr::drop_in_place(tcb_mut) };

                let paddr = virt_to_phys(vaddr);
                Some(Self::create_untyped(
                    &UntypedRegion { start: paddr, pages: 1, watermark: 0 },
                    Rights::all(),
                ))
            }
            CapType::Endpoint => {
                let vaddr = VirtAddr::from(self.words[0]);
                let ep = unsafe { vaddr.as_ref::<Endpoint>() };

                // If this is a badged endpoint (client), it might be shared with the server (unbadged).
                // Recycling is only allowed if this is the last reference to the Endpoint object.
                // Note: The caller (CNode::recycle) has already revoked all children of this capability.

                if ep.ref_count.load(Ordering::Relaxed) > 1 {
                    warn!(
                        "cap_recycle: Cannot recycle Endpoint {:p}: still has {} references. (is_badged: {})",
                        ep,
                        ep.ref_count.load(Ordering::Relaxed),
                        self.is_badged()
                    );
                    return None;
                }

                // If we are here, we are the last owner.
                // We must unblock any pending threads before reclaiming memory.
                ep.destroy();

                let paddr = virt_to_phys(vaddr);
                Some(Self::create_untyped(
                    &UntypedRegion { start: paddr, pages: 1, watermark: 0 },
                    Rights::all(),
                ))
            }
            CapType::Untyped => {
                let paddr = PhysAddr::from(self.words[0]);
                let data = self.get_data();
                let pages = data & 0x1FFFFFF;
                Some(Self::create_untyped(
                    &UntypedRegion { start: paddr, pages, watermark: 0 },
                    Rights::all(),
                ))
            }
            _ => None,
        }
    }
}
