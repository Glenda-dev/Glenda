use super::super::method::*;
use crate::cap::{Badge, CNode, CapPtr, CapType, Capability, Rights};
use crate::error::Error;
use crate::proc::scheduler;

pub fn invoke_cnode(cap: &mut Capability, method: usize) -> Result<(), Error> {
    let vaddr = if cap.cap_type() == CapType::CNode {
        cap.obj_ptr()
    } else {
        error!("CNode::invoke failed: invalid obj type {:?}", cap.cap_type());
        return Err(Error::InvalidType);
    };

    let cnode = unsafe { vaddr.as_mut::<CNode>() };
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => {
            error!("CNode::invoke failed: no UTCB");
            return Err(Error::MappingFailed);
        }
    };

    match method {
        cnodemethod::MINT => {
            // Mint: (src_cptr, dest_slot, badge, rights)
            let src_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let dest_cptr = CapPtr::from(utcb.mrs_regs[1]);
            let new_badge = Badge::from(utcb.mrs_regs[2]);
            let req_rights = Rights::from_bits_truncate(utcb.mrs_regs[3] as u8);

            if let Some(src_slot) = tcb.lookup_slot(src_cptr) {
                let src_cap = unsafe { &(*src_slot).cap };

                // 1. 权限收缩：新权限必须是源权限的子集
                let final_rights = req_rights.intersection(src_cap.rights());

                // 2. Badge 检查
                let src_badge = src_cap.get_badge();
                if !src_badge.is_null() && !new_badge.is_null() {
                    error!("CNode::Mint failed: rebadge attempt");
                    return Err(Error::InvalidCapability);
                }
                let final_badge = if !src_badge.is_null() { src_badge } else { new_badge };

                let new_cap = src_cap.mint(final_badge, final_rights);

                cnode.insert_child(dest_cptr, &new_cap, src_slot).map_err(|e| {
                    error!(
                        "CNode::Mint failed: insert_child failed dest={:?} error={:?}",
                        dest_cptr, e
                    );
                    e
                })
            } else {
                error!("CNode::Mint failed: src not found cptr={:?}", src_cptr);
                Err(Error::InvalidCapability)
            }
        }
        cnodemethod::COPY => {
            // Copy: (src_ dest_slot, rights)
            let src_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let dest_cptr = CapPtr::from(utcb.mrs_regs[1]);
            let rights = utcb.mrs_regs[2] as u8;

            if let Some(src_slot) = tcb.lookup_slot(src_cptr) {
                let src_cap = unsafe { &(*src_slot).cap };
                let new_cap = src_cap.mint(Badge::null(), Rights::from_bits_truncate(rights));

                cnode.insert_child(dest_cptr, &new_cap, src_slot).map_err(|e| {
                    error!(
                        "CNode::Copy failed: insert_child failed dest={:?} error={:?}",
                        dest_cptr, e
                    );
                    e
                })
            } else {
                error!("CNode::Copy failed: src not found cptr={:?}", src_cptr);
                Err(Error::InvalidCapability)
            }
        }
        cnodemethod::DELETE => {
            // Delete: (slot)
            let cptr = CapPtr::from(utcb.mrs_regs[0]);
            cnode.delete(cptr).map_err(|e| {
                error!("CNode::Delete failed: cptr={:?} error={:?}", cptr, e);
                e
            })
        }
        cnodemethod::REVOKE => {
            // Revoke: (slot)
            let cptr = CapPtr::from(utcb.mrs_regs[0]);
            cnode.revoke(cptr).map_err(|e| {
                error!("CNode::Revoke failed: cptr={:?} error={:?}", cptr, e);
                e
            })
        }
        cnodemethod::MOVE => {
            // Move: (src_cptr, dest_slot)
            let src_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let dest_cptr = CapPtr::from(utcb.mrs_regs[1]);

            cnode.move_cap(src_cptr, dest_cptr).map_err(|e| {
                error!("CNode::Move failed: src={:?} dest={:?} error={:?}", src_cptr, dest_cptr, e);
                e
            })
        }
        cnodemethod::DEBUG_PRINT => {
            cnode.debug_print();
            Ok(())
        }
        cnodemethod::RECYCLE => {
            let cptr = CapPtr::from(utcb.mrs_regs[0]);
            match cnode.recycle(cptr) {
                Ok((paddr, pages)) => {
                    utcb.mrs_regs[0] = paddr;
                    utcb.mrs_regs[1] = pages;
                    Ok(())
                }
                Err(e) => {
                    error!("CNode::Recycle failed: cptr={:?} error={:?}", cptr, e);
                    Err(e)
                }
            }
        }
        _ => {
            error!("CNode::invoke failed: invalid method {}", method);
            Err(Error::InvalidMethod)
        }
    }
}
