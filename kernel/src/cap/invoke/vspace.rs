use super::super::method::*;
use crate::cap::{CapPtr, CapType, Capability};
use crate::hal;
use crate::hal::mem::PGSIZE;

use crate::error::Error;
use crate::mem::PageTable;
use crate::mem::Perms;
use crate::mem::VirtAddr;
use crate::proc::scheduler;

pub fn invoke_pagetable(cap: &mut Capability, method: usize) -> Result<(), Error> {
    let paddr = if cap.cap_type() == CapType::PageTable {
        cap.paddr()
    } else {
        log!("vspace: PageTable invoke failed: invalid obj type {:?}", cap.cap_type());
        return Err(Error::InvalidType);
    };

    // PageTable 需要物理地址转虚拟地址才能操作
    let pt_ptr = hal::mem::phys_to_virt(paddr);
    let pt = unsafe { pt_ptr.as_mut::<PageTable>() };
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => {
            log!("vspace: PageTable invoke failed: no UTCB");
            return Err(Error::MappingFailed);
        }
    };

    match method {
        pagetablemethod::MAP_TABLE => {
            // MapTable: (table_cap, vaddr, level)
            let table_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let vaddr = VirtAddr::from(utcb.mrs_regs[1]);
            let level = utcb.mrs_regs[2];

            let table_cap = match tcb.cap_lookup(table_cptr) {
                Some(c) => c,
                None => {
                    log!(
                        "vspace: PageTable map_table failed: table cap not found cptr={}",
                        table_cptr
                    );
                    return Err(Error::InvalidCapability);
                }
            };

            let table_paddr = if table_cap.cap_type() == CapType::PageTable {
                table_cap.paddr()
            } else {
                log!(
                    "PageTable::MapTable failed: invalid table cap type {:?}\n",
                    table_cap.cap_type()
                );
                return Err(Error::InvalidType);
            };

            pt.map_table(vaddr, table_paddr, level).map_err(|_| {
                log!(
                    "PageTable::MapTable failed: pt.map_table failed vaddr={} level={}\n",
                    vaddr,
                    level
                );
                Error::MappingFailed
            })
        }
        _ => {
            log!("vspace: PageTable invoke failed: invalid method {}", method);
            Err(Error::InvalidMethod)
        }
    }
}

pub fn invoke_vspace(cap: &mut Capability, method: usize) -> Result<(), Error> {
    let paddr = if cap.cap_type() == CapType::VSpace {
        cap.paddr()
    } else {
        log!("vspace: VSpace invoke failed: invalid obj type {:?}", cap.cap_type());
        return Err(Error::InvalidType);
    };

    // PageTable 需要物理地址转虚拟地址才能操作
    let pt_ptr = hal::mem::phys_to_virt(paddr);
    let pt = unsafe { pt_ptr.as_mut::<PageTable>() };
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => {
            log!("vspace: VSpace invoke failed: no UTCB");
            return Err(Error::MappingFailed);
        }
    };

    match method {
        vspacemethod::MAP => {
            // Map: (frame_cap, vaddr, flags)
            let frame_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let vaddr = VirtAddr::from(utcb.mrs_regs[1]);
            let flags = Perms::from_bits_truncate(utcb.mrs_regs[2]);

            let frame_cap = match tcb.cap_lookup(frame_cptr) {
                Some(c) => c,
                None => {
                    log!("vspace: VSpace map failed: frame cap not found cptr={}", frame_cptr);
                    return Err(Error::InvalidCapability);
                }
            };

            let frame_paddr = if frame_cap.cap_type() == CapType::Frame {
                frame_cap.paddr()
            } else {
                log!(
                    "vspace: VSpace map failed: invalid frame cap type {:?}",
                    frame_cap.cap_type()
                );
                return Err(Error::InvalidType);
            };

            let num_pages = frame_cap.get_data();

            // 执行映射
            pt.map(vaddr, frame_paddr, num_pages * PGSIZE, flags).map_err(|_| {
                log!(
                    "vspace: VSpace map failed: pt.map failed vaddr={} pages={}\n",
                    vaddr,
                    num_pages
                );
                Error::MappingFailed
            })
        }
        vspacemethod::MAP_TABLE => {
            // MapTable: (table_cap, vaddr, level)
            let table_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let vaddr = VirtAddr::from(utcb.mrs_regs[1]);
            let level = utcb.mrs_regs[2];

            let table_cap = match tcb.cap_lookup(table_cptr) {
                Some(c) => c,
                None => {
                    log!("VSpace::MapTable failed: table cap not found cptr={}", table_cptr);
                    return Err(Error::InvalidCapability);
                }
            };

            let table_paddr = if table_cap.cap_type() == CapType::PageTable
                || table_cap.cap_type() == CapType::VSpace
            {
                table_cap.paddr()
            } else {
                log!(
                    "VSpace::MapTable failed: invalid table cap type {:?}\n",
                    table_cap.cap_type()
                );
                return Err(Error::InvalidType);
            };

            pt.map_table(vaddr, table_paddr, level).map_err(|_| {
                log!(
                    "VSpace::MapTable failed: pt.map_table failed vaddr={} level={}\n",
                    vaddr,
                    level
                );
                Error::MappingFailed
            })
        }
        vspacemethod::UNMAP => {
            // Unmap: (vaddr, size)
            let vaddr = VirtAddr::from(utcb.mrs_regs[0]);
            let size = utcb.mrs_regs[1];
            pt.unmap(vaddr, size).map_err(|_| {
                log!("VSpace::Unmap failed: vaddr={} size={}", vaddr, size);
                Error::MappingFailed
            })
        }
        vspacemethod::SETUP => hal::mem::pt_setup(pt).map_err(|_| {
            log!("VSpace::Setup failed");
            Error::MappingFailed
        }),
        vspacemethod::DEBUG_PRINT => {
            pt.debug_print();
            Ok(())
        }
        _ => {
            log!("vspace: VSpace invoke failed: invalid method {}", method);
            Err(Error::InvalidMethod)
        }
    }
}
