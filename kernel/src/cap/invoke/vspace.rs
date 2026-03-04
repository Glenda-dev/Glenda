use super::super::method::*;
use crate::cap::{CapPtr, CapType, Capability, Rights};
use crate::error::Error;
use crate::hal;
use crate::hal::mem::PGSIZE;
use crate::mem::addr::phys_to_virt;
use crate::mem::{PageTable, Perms, VirtAddr};
use crate::proc::scheduler;

pub fn invoke_pagetable(cap: &mut Capability, method: usize) -> Result<(), Error> {
    let paddr = if cap.cap_type() == CapType::PageTable {
        cap.paddr()
    } else {
        error!("vspace: PageTable invoke failed: invalid obj type {:?}", cap.cap_type());
        return Err(Error::InvalidType);
    };

    // PageTable 需要物理地址转虚拟地址才能操作
    let pt_ptr = phys_to_virt(paddr);
    let pt = unsafe { pt_ptr.as_mut::<PageTable>() };
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => {
            error!("vspace: PageTable invoke failed: no UTCB");
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
                    error!(
                        "vspace: PageTable map_table failed: table cap not found cptr={}",
                        table_cptr
                    );
                    return Err(Error::InvalidCapability);
                }
            };

            let table_paddr = if table_cap.cap_type() == CapType::PageTable {
                table_cap.paddr()
            } else {
                error!(
                    "PageTable::MapTable failed: invalid table cap type {:?}\n",
                    table_cap.cap_type()
                );
                return Err(Error::InvalidType);
            };

            pt.map_table(vaddr, table_paddr, level).map_err(|_| {
                error!(
                    "PageTable::MapTable failed: pt.map_table failed vaddr={} level={}\n",
                    vaddr, level
                );
                Error::MappingFailed
            })?;
            hal::mem::flush_tlb(Some(vaddr), PGSIZE, None);
            Ok(())
        }
        pagetablemethod::UNMAP_TABLE => {
            // UnmapTable: (vaddr, level)
            let vaddr = VirtAddr::from(utcb.mrs_regs[0]);
            let level = utcb.mrs_regs[1];

            pt.unmap_table(vaddr, level).map_err(|_| {
                error!(
                    "PageTable::UnmapTable failed: pt.unmap_table failed vaddr={} level={}\n",
                    vaddr, level
                );
                Error::MappingFailed
            })?;
            hal::mem::flush_tlb(Some(vaddr), PGSIZE, None);
            Ok(())
        }
        _ => {
            error!("vspace: PageTable invoke failed: invalid method {}", method);
            Err(Error::InvalidMethod)
        }
    }
}

pub fn invoke_vspace(cap: &mut Capability, method: usize) -> Result<(), Error> {
    let (paddr, asid) = if cap.cap_type() == CapType::VSpace {
        cap.vspace_info()
    } else {
        error!("vspace: VSpace invoke failed: invalid obj type {:?}", cap.cap_type());
        return Err(Error::InvalidType);
    };

    // PageTable 需要物理地址转虚拟地址才能操作
    let pt_ptr = phys_to_virt(paddr);
    let pt = unsafe { pt_ptr.as_mut::<PageTable>() };
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => {
            error!("vspace: VSpace invoke failed: no UTCB");
            return Err(Error::MappingFailed);
        }
    };

    match method {
        vspacemethod::MAP => {
            // Map: (frame_cap, vaddr, rights)
            let frame_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let vaddr = VirtAddr::from(utcb.mrs_regs[1]);
            let perms = Perms::from_bits_truncate(utcb.mrs_regs[2]);

            let frame_cap = match tcb.cap_lookup(frame_cptr) {
                Some(c) => c,
                None => {
                    error!("vspace: VSpace map failed: frame cap not found cptr={}", frame_cptr);
                    return Err(Error::InvalidCapability);
                }
            };

            let frame_paddr = if frame_cap.cap_type() == CapType::Frame {
                frame_cap.paddr()
            } else {
                error!(
                    "vspace: VSpace map failed: invalid frame cap type {:?}",
                    frame_cap.cap_type()
                );
                return Err(Error::InvalidType);
            };

            let effective_rights = frame_cap.rights();
            // Check permissions
            if !effective_rights.contains(Rights::READ) && perms.contains(Perms::READ) {
                error!("vspace: VSpace map failed: insufficient rights for READ");
                return Err(Error::PermissionDenied);
            }
            if !effective_rights.contains(Rights::WRITE) && perms.contains(Perms::WRITE) {
                error!("vspace: VSpace map failed: insufficient rights for WRITE");
                return Err(Error::PermissionDenied);
            }
            if !effective_rights.contains(Rights::EXECUTE) && perms.contains(Perms::EXECUTE) {
                error!("vspace: VSpace map failed: insufficient rights for EXECUTE");
                return Err(Error::PermissionDenied);
            }
            // Enforce that executable mappings must have READ permission
            if perms.contains(Perms::EXECUTE) && !perms.contains(Perms::READ) {
                error!("vspace: VSpace map failed: EXECUTE permission requires READ permission");
                return Err(Error::PermissionDenied);
            }
            // Enforce that writable mappings must have READ permission
            if perms.contains(Perms::WRITE) && !perms.contains(Perms::READ) {
                error!("vspace: VSpace map failed: WRITE permission requires READ permission");
                return Err(Error::PermissionDenied);
            }
            // Enforce W^X check: if executable, must not be writable
            if perms.contains(Perms::EXECUTE) && perms.contains(Perms::WRITE) {
                error!(
                    "vspace: VSpace map failed: W^X violation (cannot be both EXECUTE and WRITE)"
                );
                return Err(Error::PermissionDenied);
            }

            let cap_pages = frame_cap.get_data();
            let mut num_pages = utcb.mrs_regs[3];
            if num_pages == 0 || num_pages > cap_pages {
                num_pages = cap_pages;
            }

            // 执行映射
            pt.map(vaddr, frame_paddr, num_pages * PGSIZE, perms).map_err(|_| {
                error!(
                    "vspace: VSpace map failed: pt.map failed vaddr={} pages={}",
                    vaddr, num_pages
                );
                Error::MappingFailed
            })?;
            hal::mem::flush_tlb(Some(vaddr), num_pages * PGSIZE, Some(asid.id as usize));
            Ok(())
        }
        vspacemethod::MAP_TABLE => {
            // MapTable: (table_cap, vaddr, level)
            let table_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let vaddr = VirtAddr::from(utcb.mrs_regs[1]);
            let level = utcb.mrs_regs[2];

            let table_cap = match tcb.cap_lookup(table_cptr) {
                Some(c) => c,
                None => {
                    error!("VSpace::MapTable failed: table cap not found cptr={}", table_cptr);
                    return Err(Error::InvalidCapability);
                }
            };

            let table_paddr = if table_cap.cap_type() == CapType::PageTable
                || table_cap.cap_type() == CapType::VSpace
            {
                table_cap.paddr()
            } else {
                error!(
                    "VSpace::MapTable failed: invalid table cap type {:?}\n",
                    table_cap.cap_type()
                );
                return Err(Error::InvalidType);
            };

            pt.map_table(vaddr, table_paddr, level).map_err(|_| {
                error!(
                    "VSpace::MapTable failed: pt.map_table failed vaddr={} level={}\n",
                    vaddr, level
                );
                Error::MappingFailed
            })?;
            hal::mem::flush_tlb(Some(vaddr), PGSIZE, Some(asid.id as usize));
            Ok(())
        }
        vspacemethod::UNMAP => {
            // Unmap: (vaddr, size)
            let vaddr = VirtAddr::from(utcb.mrs_regs[0]);
            let size = utcb.mrs_regs[1];
            pt.unmap(vaddr, size).map_err(|_| {
                error!("VSpace::Unmap failed: vaddr={} size={}", vaddr, size);
                Error::MappingFailed
            })?;
            hal::mem::flush_tlb(Some(vaddr), size, Some(asid.id as usize));
            Ok(())
        }
        vspacemethod::UPDATE => {
            // Update: (vaddr, size, flags)
            let vaddr = VirtAddr::from(utcb.mrs_regs[0]);
            let size = utcb.mrs_regs[1];
            let flags = Perms::from_bits_truncate(utcb.mrs_regs[2]);

            pt.update(vaddr, size, flags).map_err(|_| {
                error!("VSpace::Update failed: vaddr={} size={} flags={:?}", vaddr, size, flags);
                Error::MappingFailed
            })?;
            hal::mem::flush_tlb(Some(vaddr), size, Some(asid.id as usize));
            Ok(())
        }
        vspacemethod::UNMAP_TABLE => {
            // UnmapTable: (vaddr, level)
            let vaddr = VirtAddr::from(utcb.mrs_regs[0]);
            let level = utcb.mrs_regs[1];
            pt.unmap_table(vaddr, level).map_err(|_| {
                error!("VSpace::UnmapTable failed: vaddr={} level={}", vaddr, level);
                Error::MappingFailed
            })?;
            hal::mem::flush_tlb(Some(vaddr), PGSIZE, Some(asid.id as usize));
            Ok(())
        }
        vspacemethod::SETUP => hal::mem::pt_setup(pt).map_err(|_| {
            error!("VSpace::Setup failed");
            Error::MappingFailed
        }),
        vspacemethod::DEBUG_PRINT => {
            printk!("VSpace at paddr {} asid {} gen {}:\n", paddr, asid.id, asid.generation);
            pt.debug_print();
            Ok(())
        }
        _ => {
            error!("vspace: VSpace invoke failed: invalid method {}", method);
            Err(Error::InvalidMethod)
        }
    }
}
