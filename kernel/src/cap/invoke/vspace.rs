use super::super::method::*;
use crate::cap::{CapPtr, CapType, Capability};
use crate::hal;
use crate::hal::mem::PGSIZE;

use crate::mem::PageTable;
use crate::mem::Perms;
use crate::mem::VirtAddr;
use crate::proc::scheduler;
use crate::trap::syscall::errcode;

pub fn invoke_pagetable(cap: &mut Capability, method: usize) -> usize {
    let paddr = if cap.cap_type() == CapType::PageTable {
        cap.paddr()
    } else {
        log!("PageTable::invoke failed: invalid obj type {:?}", cap.cap_type());
        return errcode::INVALID_OBJ_TYPE;
    };

    // PageTable 需要物理地址转虚拟地址才能操作
    let pt_ptr = hal::mem::phys_to_virt(paddr);
    let pt = pt_ptr.as_mut::<PageTable>();
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => {
            log!("PageTable::invoke failed: no UTCB");
            return errcode::MAPPING_FAILED;
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
                    log!("PageTable::MapTable failed: table cap not found cptr={:?}", table_cptr);
                    return errcode::INVALID_CAP;
                }
            };

            let table_paddr = if table_cap.cap_type() == CapType::PageTable {
                table_cap.paddr()
            } else {
                log!(
                    "PageTable::MapTable failed: invalid table cap type {:?}\n",
                    table_cap.cap_type()
                );
                return errcode::INVALID_OBJ_TYPE;
            };

            match pt.map_table(vaddr, table_paddr, level) {
                Ok(()) => errcode::SUCCESS,
                Err(_) => {
                    log!(
                        "PageTable::MapTable failed: pt.map_table failed vaddr={:?} level={}\n",
                        vaddr,
                        level
                    );
                    errcode::MAPPING_FAILED
                }
            }
        }
        _ => {
            log!("PageTable::invoke failed: invalid method {}", method);
            errcode::INVALID_METHOD
        }
    }
}

pub fn invoke_vspace(cap: &mut Capability, method: usize) -> usize {
    let paddr = if cap.cap_type() == CapType::VSpace {
        cap.paddr()
    } else {
        log!("VSpace::invoke failed: invalid obj type {:?}", cap.cap_type());
        return errcode::INVALID_OBJ_TYPE;
    };

    // PageTable 需要物理地址转虚拟地址才能操作
    let pt_ptr = hal::mem::phys_to_virt(paddr);
    let pt = pt_ptr.as_mut::<PageTable>();
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => {
            log!("VSpace::invoke failed: no UTCB");
            return errcode::MAPPING_FAILED;
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
                    log!("VSpace::Map failed: frame cap not found cptr={:?}", frame_cptr);
                    return errcode::INVALID_CAP;
                }
            };

            let frame_paddr = if frame_cap.cap_type() == CapType::Frame {
                frame_cap.paddr()
            } else {
                log!("VSpace::Map failed: invalid frame cap type {:?}", frame_cap.cap_type());
                return errcode::INVALID_OBJ_TYPE;
            };

            let num_pages = frame_cap.get_data();

            // 执行映射
            match pt.map(vaddr, frame_paddr, num_pages * PGSIZE, flags) {
                Ok(()) => errcode::SUCCESS,
                Err(_) => {
                    log!(
                        "VSpace::Map failed: pt.map failed vaddr={:?} pages={}\n",
                        vaddr,
                        num_pages
                    );
                    errcode::MAPPING_FAILED
                }
            }
        }
        vspacemethod::MAP_TABLE => {
            // MapTable: (table_cap, vaddr, level)
            let table_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let vaddr = VirtAddr::from(utcb.mrs_regs[1]);
            let level = utcb.mrs_regs[2];

            let table_cap = match tcb.cap_lookup(table_cptr) {
                Some(c) => c,
                None => {
                    log!("VSpace::MapTable failed: table cap not found cptr={:?}", table_cptr);
                    return errcode::INVALID_CAP;
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
                return errcode::INVALID_OBJ_TYPE;
            };

            match pt.map_table(vaddr, table_paddr, level) {
                Ok(()) => errcode::SUCCESS,
                Err(_) => {
                    log!(
                        "VSpace::MapTable failed: pt.map_table failed vaddr={:?} level={}\n",
                        vaddr,
                        level
                    );
                    errcode::MAPPING_FAILED
                }
            }
        }
        vspacemethod::UNMAP => {
            // Unmap: (vaddr, size)
            let vaddr = VirtAddr::from(utcb.mrs_regs[0]);
            let size = utcb.mrs_regs[1];
            match pt.unmap(vaddr, size) {
                Ok(()) => errcode::SUCCESS,
                Err(_) => {
                    log!("VSpace::Unmap failed: vaddr={:?} size={}", vaddr, size);
                    errcode::MAPPING_FAILED
                }
            }
        }
        vspacemethod::SETUP => match hal::mem::pt_setup(pt) {
            Ok(()) => errcode::SUCCESS,
            Err(_) => {
                log!("VSpace::Setup failed");
                errcode::MAPPING_FAILED
            }
        },
        vspacemethod::DEBUG_PRINT => {
            pt.debug_print();
            errcode::SUCCESS
        }
        _ => {
            log!("VSpace::invoke failed: invalid method {}", method);
            errcode::INVALID_METHOD
        }
    }
}
