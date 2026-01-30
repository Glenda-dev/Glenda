use super::PhysAddr;
use crate::cap::CNODE_PAGES;
use crate::cap::{CNode, CapType, Capability, Rights, Slot};
use crate::hal;
use crate::hal::mem::{PGSIZE, PageTable};
use crate::ipc;
use crate::mem::{PhysFrame, VirtAddr};
use crate::proc::{TCB, asid};
use crate::trap::syscall::errcode;

#[derive(Debug, Clone, Copy)]
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

    pub fn retype(
        &mut self,
        obj_type: CapType,
        flags: usize,
        n_objects: usize,
        dest_cnode: &mut CNode,
        dest_slot_offset: usize,
        dirty: usize,
    ) -> usize {
        let obj_pages = match obj_type {
            CapType::CNode => CNODE_PAGES,
            CapType::TCB => sizes::TCB,
            CapType::Endpoint => sizes::ENDPOINT,
            CapType::Frame => flags,
            CapType::PageTable => sizes::PAGETABLE,
            CapType::VSpace => sizes::VSPACE,
            CapType::Untyped => flags,
            _ => return errcode::INVALID_OBJ_TYPE,
        };

        let needed_pages = n_objects * obj_pages;
        if self.watermark + needed_pages > self.pages {
            return errcode::UNTYPE_OOM;
        }

        for i in 0..n_objects {
            let slot_idx = dest_slot_offset + i;
            let slot_addr = dest_cnode.get_slot_addr(slot_idx);
            if slot_addr == VirtAddr::null() {
                return errcode::INVALID_SLOT;
            }
            let slot = unsafe { &*slot_addr.as_mut_ptr::<Slot>() };
            if !slot.cap.is_null() {
                return errcode::INVALID_SLOT;
            }
        }

        let current_page_offset = self.watermark;

        for i in 0..n_objects {
            let page_idx = current_page_offset + i * obj_pages;
            let obj_paddr = PhysAddr::from(self.start.as_usize() + page_idx * PGSIZE);
            let obj_vaddr = hal::mem::phys_to_virt(obj_paddr);
            let obj_size_bytes = obj_pages * PGSIZE;

            if dirty == 0 {
                unsafe { core::ptr::write_bytes(obj_vaddr.as_mut_ptr::<u8>(), 0, obj_size_bytes) };
            }

            let new_cap = match obj_type {
                CapType::CNode => {
                    let cnode_ptr = obj_vaddr.as_mut_ptr::<CNode>();
                    unsafe { cnode_ptr.write(CNode::new(flags as u8)) };
                    Capability::create_cnode(unsafe { &*cnode_ptr }, Rights::ALL)
                }
                CapType::TCB => {
                    let tcb_ptr = obj_vaddr.as_mut_ptr::<TCB>();
                    unsafe { tcb_ptr.write(TCB::new()) };
                    Capability::create_tcb(unsafe { &*tcb_ptr }, Rights::ALL)
                }
                CapType::Endpoint => {
                    let ep_ptr = obj_vaddr.as_mut_ptr::<ipc::Endpoint>();
                    unsafe { ep_ptr.write(ipc::Endpoint::new()) };
                    Capability::create_endpoint(unsafe { &*ep_ptr }, Rights::ALL)
                }
                CapType::Frame => {
                    let frame = PhysFrame { paddr: obj_paddr, pages: obj_pages };
                    Capability::create_frame(&frame, Rights::ALL)
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
                    let untyped =
                        UntypedRegion { start: obj_paddr, pages: obj_pages, watermark: 0 };
                    Capability::create_untyped(&untyped, Rights::ALL)
                }
                _ => return errcode::INVALID_OBJ_TYPE,
            };

            if !dest_cnode.insert(dest_slot_offset + i, &new_cap) {
                return errcode::INVALID_SLOT;
            }
        }

        self.watermark += needed_pages;
        errcode::SUCCESS
    }
}
