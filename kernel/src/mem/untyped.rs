use super::PhysAddr;
use crate::cap::CNODE_PAGES;
use crate::cap::{CNode, CapType, Capability, Rights};
use crate::hal::mem::PGSIZE;
use crate::ipc;
use crate::mem::PageTable;
use crate::mem::PhysFrame;
use crate::mem::addr::phys_to_virt;
use crate::proc::{TCB, asid};

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct UntypedRegion {
    pub start: PhysAddr,
    pub pages: usize,
    pub watermark: usize,
}

mod sizes {
    pub const TCB: usize = 1; // 1 page
    pub const ENDPOINT: usize = 1; // 1 page
    pub const PAGETABLE: usize = 1; // 1 page
    pub const VSPACE: usize = 1; // 4 pages
}

impl UntypedRegion {
    pub fn empty() -> Self {
        Self { start: PhysAddr::null(), pages: 0, watermark: 0 }
    }

    pub fn from_cap(cap: &Capability) -> Option<Self> {
        if cap.cap_type() != CapType::Untyped {
            return None;
        }
        let data = cap.get_data();
        let pages = data & 0x1FFFFFF;
        let watermark = (data >> 25) & 0x1FFFFFF;
        Some(Self { start: cap.paddr(), pages: pages, watermark: watermark })
    }

    pub fn check(&self, need_pages: usize) -> bool {
        self.watermark + need_pages <= self.pages
    }

    pub fn retype(&mut self, obj_type: CapType, flags: usize) -> Option<Capability> {
        let obj_pages = match obj_type {
            CapType::CNode => CNODE_PAGES,
            CapType::TCB => sizes::TCB,
            CapType::Endpoint => sizes::ENDPOINT,
            CapType::Frame => flags,
            CapType::PageTable => sizes::PAGETABLE,
            CapType::VSpace => sizes::VSPACE,
            CapType::Untyped => flags,
            _ => {
                error!("Untyped::Retype failed: unsupported type {:?}", obj_type);
                return None;
            }
        };

        let needed_pages = obj_pages;

        let current_page_offset = self.watermark;

        let page_idx = current_page_offset;
        let obj_paddr = PhysAddr::from(self.start.as_usize() + page_idx * PGSIZE);
        log!("untyped: Retyping paddr {:?} to {:?} (pages: {})", obj_paddr, obj_type, obj_pages);
        let obj_vaddr = phys_to_virt(obj_paddr);
        let obj_size_bytes = obj_pages * PGSIZE;
        if self.watermark + needed_pages > self.pages {
            error!(
                "Untyped::Retype OOM: need {} pages, {} available (total: {}, watermark: {})",
                needed_pages,
                self.pages - self.watermark,
                self.pages,
                self.watermark
            );
            return None;
        }

        // Security: Zero out memory before delegating to typed objects.
        // Optimization: Defer zeroing when retyping into smaller Untyped blocks,
        // it will be zeroed eventually when retyped into concrete objects.
        if obj_type != CapType::Untyped {
            unsafe { core::ptr::write_bytes(obj_vaddr.as_mut_ptr::<u8>(), 0, obj_size_bytes) };
        }

        let new_cap = match obj_type {
            CapType::CNode => {
                let cnode = unsafe { obj_vaddr.as_mut::<CNode>() };
                cnode.init();
                Capability::create_cnode(cnode, Rights::ALL)
            }
            CapType::TCB => {
                let tcb_ptr = obj_vaddr.as_mut_ptr::<TCB>();
                unsafe { tcb_ptr.write(TCB::new()) };
                // Register TCB for GDB debugging
                TCB::register(tcb_ptr);
                Capability::create_tcb(unsafe { &*tcb_ptr }, Rights::ALL)
            }
            CapType::Endpoint => {
                let ep_ptr = obj_vaddr.as_mut_ptr::<ipc::Endpoint>();
                unsafe { ep_ptr.write(ipc::Endpoint::new()) };
                Capability::create_endpoint(unsafe { &*ep_ptr }, Rights::ALL)
            }
            CapType::Frame => {
                let frame = PhysFrame { paddr: obj_paddr, pages: obj_pages };
                Capability::create_frame(&frame, Rights::ALL, false)
            }
            CapType::PageTable => {
                let pt_ptr = obj_vaddr.as_mut_ptr::<PageTable>();
                unsafe { pt_ptr.write(PageTable::new()) };
                Capability::create_pagetable(unsafe { &*pt_ptr }, flags, Rights::ALL)
            }
            CapType::VSpace => {
                let pt_ptr = obj_vaddr.as_mut_ptr::<PageTable>();
                unsafe { pt_ptr.write(PageTable::new()) };
                Capability::create_vspace(unsafe { &*pt_ptr }, asid::alloc(), Rights::ALL)
            }
            CapType::Untyped => {
                let untyped = UntypedRegion { start: obj_paddr, pages: obj_pages, watermark: 0 };
                Capability::create_untyped(&untyped, Rights::ALL)
            }
            _ => {
                error!("Untyped::Retype failed: unknown type");
                return None;
            }
        };

        self.watermark += needed_pages;
        Some(new_cap)
    }
}
