use super::super::method::*;
use crate::cap::{Badge, CNode, CapPtr, CapType, Capability, Rights};
use crate::proc::scheduler;
use crate::trap::syscall::errcode;

pub fn invoke_cnode(cap: &mut Capability, method: usize) -> usize {
    let vaddr = if cap.cap_type() == CapType::CNode {
        cap.obj_ptr()
    } else {
        return errcode::INVALID_OBJ_TYPE;
    };

    let cnode = vaddr.as_mut::<CNode>();
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => return errcode::MAPPING_FAILED,
    };

    match method {
        cnodemethod::MINT => {
            // Mint: (src_cptr, dest_slot, badge, rights)
            let src_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let dest_cptr = CapPtr::from(utcb.mrs_regs[1]);
            let new_badge = Badge::from(utcb.mrs_regs[2]);
            let req_rights = Rights::from_bits_truncate(utcb.mrs_regs[3] as u8);

            if let Some(src_cap) = tcb.cap_lookup(src_cptr) {
                // 1. 权限收缩：新权限必须是源权限的子集
                let final_rights = req_rights.intersection(src_cap.rights());

                // 2. Badge 检查：
                // - 如果源 Cap 已有 Badge，则不能再次设置新 Badge (seL4 语义)
                // - 新 Cap 必须继承源 Cap 的 Badge (如果存在)
                let src_badge = src_cap.get_badge();
                if !src_badge.is_null() && !new_badge.is_null() {
                    return errcode::INVALID_CAP; // Cannot re-badge an already badged cap
                }
                let final_badge = if !src_badge.is_null() { src_badge } else { new_badge };

                let new_cap = src_cap.mint(final_badge, final_rights);
                if cnode.insert_child(dest_cptr, &new_cap, &src_cap) {
                    errcode::SUCCESS
                } else {
                    errcode::INVALID_SLOT
                }
            } else {
                errcode::INVALID_CAP
            }
        }
        cnodemethod::COPY => {
            // Copy: (src_ dest_slot, rights)
            let src_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let dest_cptr = CapPtr::from(utcb.mrs_regs[1]);
            let rights = utcb.mrs_regs[2] as u8;

            if let Some(src_cap) = tcb.cap_lookup(src_cptr) {
                let new_cap = src_cap.mint(Badge::null(), Rights::from_bits_truncate(rights));
                if cnode.insert_child(dest_cptr, &new_cap, &src_cap) {
                    errcode::SUCCESS
                } else {
                    errcode::INVALID_SLOT
                }
            } else {
                errcode::INVALID_CAP
            }
        }
        cnodemethod::DELETE => {
            // Delete: (slot)
            let cptr = CapPtr::from(utcb.mrs_regs[0]);
            if cnode.delete(cptr) { errcode::SUCCESS } else { errcode::INVALID_SLOT }
        }
        cnodemethod::REVOKE => {
            // Revoke: (slot)
            let cptr = CapPtr::from(utcb.mrs_regs[0]);
            if cnode.revoke(cptr) { errcode::SUCCESS } else { errcode::INVALID_SLOT }
        }
        cnodemethod::DEBUG_PRINT => {
            cnode.debug_print();
            errcode::SUCCESS
        }
        _ => errcode::INVALID_METHOD,
    }
}
