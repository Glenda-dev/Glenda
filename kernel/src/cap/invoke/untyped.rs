use super::super::method::*;
use crate::cap::{CNode, CapPtr, CapType, Capability};
use crate::error::Error;
use crate::mem::UntypedRegion;
use crate::proc::scheduler;

pub fn invoke_untyped(cap: &mut Capability, method: usize, cptr: usize) -> Result<(), Error> {
    let mut untyped = match UntypedRegion::from_cap(cap) {
        Some(u) => u,
        None => {
            log!("Untyped::invoke failed: invalid obj type {:?}", cap.cap_type());
            return Err(Error::InvalidType);
        }
    };

    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => {
            log!("Untyped::invoke failed: no UTCB");
            return Err(Error::MappingFailed);
        }
    };

    match method {
        untypedmethod::RETYPE => {
            // Retype: (type, flags,  dest_cnode, dest_slot_offset, dirty)
            let obj_type = utcb.mrs_regs[0];

            let flags = utcb.mrs_regs[1];
            let dest_cnode_cptr = CapPtr::from(utcb.mrs_regs[2]);
            let dest_slot = CapPtr::from(utcb.mrs_regs[3]);

            let dest_cnode_cap = match tcb.cap_lookup(dest_cnode_cptr) {
                Some(c) => c,
                None => {
                    log!("Untyped::Retype failed: dest CNode not found {:?}", dest_cnode_cptr);
                    return Err(Error::InvalidCapability);
                }
            };

            if dest_cnode_cap.cap_type() == CapType::CNode {
                let dest_cnode = unsafe { dest_cnode_cap.obj_ptr().as_mut::<CNode>() };
                if !dest_cnode.check_cptr(dest_slot) {
                    log!("Untyped::Retype failed: invalid dest slot {:?}", dest_slot);
                    return Err(Error::InvalidSlot);
                }
                match untyped.retype(CapType::from(obj_type), flags) {
                    Some(new_cap) => {
                        let slot_ptr = match tcb.lookup_slot(CapPtr::from(cptr)) {
                            Some(s) => s,
                            None => {
                                log!("Untyped::invoke failed: parent slot not found");
                                return Err(Error::InvalidCapability);
                            }
                        };

                        dest_cnode.insert_child(dest_slot, &new_cap, slot_ptr).map_err(|e| {
                            log!(
                                "Untyped::Retype failed: insert child failed at {:?}: {:?}",
                                dest_slot,
                                e
                            );
                            e
                        })?;

                        // 重要：将更新后的 watermark 写回原始 Untyped 能力
                        cap.set_data(untyped.pages | (untyped.watermark << 25));
                        Ok(())
                    }
                    None => {
                        log!(
                            "Untyped::Retype failed: retype failed (OOM or invalid type) {:?}",
                            CapType::from(obj_type)
                        );
                        Err(Error::InvalidType)
                    }
                }
            } else {
                log!(
                    "Untyped::Retype failed: dest cap is not CNode {:?}",
                    dest_cnode_cap.cap_type()
                );
                Err(Error::InvalidType)
            }
        }
        _ => {
            log!("Untyped::invoke failed: invalid method {}", method);
            Err(Error::InvalidMethod)
        }
    }
}
