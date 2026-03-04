use super::super::method::*;
use crate::cap::{CapPtr, CapType, Capability};
use crate::error::Error;
use crate::mem::UntypedRegion;
use crate::proc::scheduler;
use num_enum::FromPrimitive;

pub fn invoke_untyped(cap: &mut Capability, method: usize, cptr: usize) -> Result<(), Error> {
    let mut untyped = match UntypedRegion::from_cap(cap) {
        Some(u) => u,
        None => {
            error!("Untyped::invoke failed: invalid obj type {:?}", cap.cap_type());
            return Err(Error::InvalidType);
        }
    };

    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => {
            error!("Untyped::invoke failed: no UTCB");
            return Err(Error::MappingFailed);
        }
    };

    match method {
        untypedmethod::RETYPE => {
            let obj_type = CapType::from_primitive(utcb.mrs_regs[0]);
            let flags = utcb.mrs_regs[1];
            let dest_slot = CapPtr::from(utcb.mrs_regs[2]);
            let root_cnode = tcb.get_cspace_mut().ok_or(Error::InvalidCapability)?;
            log!(
                "untyped: Retype request for type {:?} with flags {} to slot {:?}",
                obj_type,
                flags,
                dest_slot
            );

            match untyped.retype(obj_type, flags) {
                Some(new_cap) => {
                    let slot_ptr = match tcb.lookup_slot(CapPtr::from(cptr)) {
                        Some(s) => s,
                        None => {
                            error!("Untyped::invoke failed: parent slot not found");
                            return Err(Error::InvalidCapability);
                        }
                    };
                    root_cnode.insert_child(dest_slot, &new_cap, slot_ptr).map_err(|e| {
                        error!(
                            "Untyped::Retype failed: insert child failed at {:?}: {:?}",
                            dest_slot, e
                        );
                        e
                    })?;

                    // 重要：将更新后的 watermark 写回原始 Untyped 能力
                    cap.set_data(untyped.pages | (untyped.watermark << 25));
                    Ok(())
                }
                None => {
                    error!(
                        "Untyped::Retype failed: retype failed (OOM or invalid type) {:?}",
                        CapType::from(obj_type)
                    );
                    Err(Error::InvalidType)
                }
            }
        }
        untypedmethod::GET_INFO => {
            utcb.mrs_regs[0] = untyped.pages;
            utcb.mrs_regs[1] = untyped.watermark;
            Ok(())
        }
        _ => {
            error!("Untyped::invoke failed: invalid method {}", method);
            Err(Error::InvalidMethod)
        }
    }
}
