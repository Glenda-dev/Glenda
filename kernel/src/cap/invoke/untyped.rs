use super::super::method::*;
use crate::cap::{CNode, CapPtr, CapType, Capability};
use crate::mem::UntypedRegion;
use crate::proc::scheduler;
use crate::trap::syscall::errcode;

pub fn invoke_untyped(cap: &mut Capability, method: usize) -> usize {
    let mut untyped = match UntypedRegion::from_cap(cap) {
        Some(u) => u,
        None => return errcode::INVALID_OBJ_TYPE,
    };

    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => return errcode::MAPPING_FAILED,
    };

    match method {
        untypedmethod::RETYPE => {
            // Retype: (type, flags,  dest_cnode, dest_slot_offset, dirty)
            let obj_type = utcb.mrs_regs[0];

            let flags = utcb.mrs_regs[1];
            let dest_cnode_cptr = CapPtr::from(utcb.mrs_regs[2]);
            let dest_slot = CapPtr::from(utcb.mrs_regs[3]);
            let dirty = utcb.mrs_regs[4];

            let dest_cnode_cap = match tcb.cap_lookup(dest_cnode_cptr) {
                Some(c) => c,
                None => return errcode::INVALID_CAP,
            };

            if dest_cnode_cap.cap_type() == CapType::CNode {
                let dest_cnode = dest_cnode_cap.obj_ptr().as_mut::<CNode>();
                if !dest_cnode.check_cptr(dest_slot) {
                    return errcode::INVALID_SLOT;
                }
                match untyped.retype(CapType::from(obj_type), flags, dirty) {
                    Some(new_cap) => {
                        if dest_cnode.insert_child(dest_slot, &new_cap, cap) {
                            // 重要：将更新后的 watermark 写回原始 Untyped 能力
                            cap.set_data(untyped.pages | (untyped.watermark << 25));
                            errcode::SUCCESS
                        } else {
                            errcode::INVALID_SLOT
                        }
                    }
                    None => errcode::INVALID_OBJ_TYPE,
                }
            } else {
                errcode::INVALID_OBJ_TYPE
            }
        }
        _ => errcode::INVALID_METHOD,
    }
}
