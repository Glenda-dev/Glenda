use super::super::method::*;
use crate::cap::{Badge, CNode, CapPtr, CapType, Capability, Rights};
use crate::error::Error;
use crate::proc::scheduler;

fn resolve_dest_cnode(
    tcb: &crate::proc::thread::TCB,
    current_cnode: &mut CNode,
    dest_cnode_cptr: CapPtr,
) -> Result<&'static mut CNode, Error> {
    if dest_cnode_cptr.is_null() {
        // Null destination cnode means "operate on current invoked cnode".
        return Ok(unsafe { &mut *(current_cnode as *mut CNode) });
    }

    let dest_cap = tcb.cap_lookup(dest_cnode_cptr).ok_or(Error::InvalidCapability)?;
    if dest_cap.cap_type() != CapType::CNode {
        error!(
            "CNode op failed: dest_cnode is not CNode, cptr={:?}, type={:?}",
            dest_cnode_cptr,
            dest_cap.cap_type()
        );
        return Err(Error::InvalidType);
    }
    Ok(unsafe { dest_cap.obj_ptr().as_mut::<CNode>() })
}

fn validate_non_null_slot(op: &str, slot: CapPtr, slot_name: &str) -> Result<(), Error> {
    if slot.is_null() {
        error!("CNode::{} failed: {} is null/invalid ({:?})", op, slot_name, slot);
        return Err(Error::InvalidSlot);
    }
    Ok(())
}

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
            // Mint: (src_cptr, dest_cnode_cptr, dest_slot, badge, rights)
            let src_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let dest_cnode_cptr = CapPtr::from(utcb.mrs_regs[1]);
            let dest_slot = CapPtr::from(utcb.mrs_regs[2]);
            let new_badge = Badge::from(utcb.mrs_regs[3]);
            let req_rights = Rights::from_bits_truncate(utcb.mrs_regs[4] as u8);
            validate_non_null_slot("Mint", src_cptr, "src_cptr")?;
            validate_non_null_slot("Mint", dest_slot, "dest_slot")?;
            let dest_cnode = resolve_dest_cnode(tcb, cnode, dest_cnode_cptr)?;

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

                dest_cnode.insert_child(dest_slot, &new_cap, src_slot).map_err(|e| {
                    error!(
                        "CNode::Mint failed: insert_child failed dest={:?} error={:?}",
                        dest_slot, e
                    );
                    e
                })
            } else {
                error!("CNode::Mint failed: src not found cptr={:?}", src_cptr);
                Err(Error::InvalidCapability)
            }
        }
        cnodemethod::COPY => {
            // Copy: (src_cptr, dest_cnode_cptr, dest_slot, rights)
            let src_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let dest_cnode_cptr = CapPtr::from(utcb.mrs_regs[1]);
            let dest_slot = CapPtr::from(utcb.mrs_regs[2]);
            let rights = utcb.mrs_regs[3] as u8;
            validate_non_null_slot("Copy", src_cptr, "src_cptr")?;
            validate_non_null_slot("Copy", dest_slot, "dest_slot")?;
            let dest_cnode = resolve_dest_cnode(tcb, cnode, dest_cnode_cptr)?;

            if let Some(src_slot) = tcb.lookup_slot(src_cptr) {
                let src_cap = unsafe { &(*src_slot).cap };
                let new_cap = src_cap.mint(Badge::null(), Rights::from_bits_truncate(rights));

                dest_cnode.insert_child(dest_slot, &new_cap, src_slot).map_err(|e| {
                    error!(
                        "CNode::Copy failed: insert_child failed dest={:?} error={:?}",
                        dest_slot, e
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
        cnodemethod::TRANSFER => {
            // Transfer: (src_cptr, dest_cnode_cptr, dest_slot)
            let src_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let dest_cnode_cptr = CapPtr::from(utcb.mrs_regs[1]);
            let dest_slot = CapPtr::from(utcb.mrs_regs[2]);
            validate_non_null_slot("Transfer", src_cptr, "src_cptr")?;
            validate_non_null_slot("Transfer", dest_slot, "dest_slot")?;
            if dest_cnode_cptr.is_null() {
                // Null destination means transfer within current invoked cnode.
                cnode.transfer(src_cptr, dest_slot).map_err(|e| {
                    error!(
                        "CNode::Transfer failed (self): move src={:?} dest_slot={:?} error={:?}",
                        src_cptr, dest_slot, e
                    );
                    e
                })
            } else {
                // Validate destination cnode capability exists and has correct type.
                let _ = resolve_dest_cnode(tcb, cnode, dest_cnode_cptr)?;

                // Perform true move semantics on root cspace:
                // - move slot content without cloning/dropping cap
                // - preserve and re-link CDT ownership tree
                let dest_cptr = CapPtr::concat(dest_cnode_cptr, dest_slot);
                tcb.get_cspace_mut().ok_or(Error::InvalidCapability)?.transfer(src_cptr, dest_cptr).map_err(
                    |e| {
                        error!(
                            "CNode::Transfer failed: move src={:?} dest_cnode={:?} dest_slot={:?} (abs={:?}) error={:?}",
                            src_cptr,
                            dest_cnode_cptr,
                            dest_slot,
                            dest_cptr,
                            e
                        );
                        e
                    },
                )
            }
        }
        cnodemethod::DEBUG_PRINT => {
            cnode.debug_print();
            Ok(())
        }
        _ => {
            error!("CNode::invoke failed: invalid method {}", method);
            Err(Error::InvalidMethod)
        }
    }
}
