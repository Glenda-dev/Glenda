use super::super::method::*;
use crate::cap::{CapPtr, CapType, Capability};
use crate::error::Error;
use crate::mem::{UntypedRegion, VirtAddr};
use crate::proc::scheduler;
use num_enum::FromPrimitive;

fn resolve_dest_cnode(
    tcb: &crate::proc::thread::TCB,
    dest_cnode_cptr: CapPtr,
) -> Result<&'static mut crate::cap::CNode, Error> {
    let dest_cap = tcb.cap_lookup(dest_cnode_cptr).ok_or(Error::InvalidCapability)?;
    if dest_cap.cap_type() != CapType::CNode {
        error!(
            "Untyped op failed: dest_cnode is not CNode, cptr={:?}, type={:?}",
            dest_cnode_cptr,
            dest_cap.cap_type()
        );
        return Err(Error::InvalidType);
    }
    Ok(unsafe { dest_cap.obj_ptr().as_mut::<crate::cap::CNode>() })
}

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
            let dest_cnode_cptr = CapPtr::from(utcb.mrs_regs[2]);
            let dest_slot = CapPtr::from(utcb.mrs_regs[3]);
            if dest_slot.is_null() {
                error!("Untyped::Retype failed: dest_slot is null/invalid");
                return Err(Error::InvalidSlot);
            }
            let dest_cnode = resolve_dest_cnode(tcb, dest_cnode_cptr)?;
            log!(
                "untyped: Retype request for type {:?} with flags {} to cnode {:?} slot {:?}",
                obj_type,
                flags,
                dest_cnode_cptr,
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
                    dest_cnode.insert_child(dest_slot, &new_cap, slot_ptr).map_err(|e| {
                        error!(
                            "Untyped::Retype failed: insert child failed at cnode {:?} slot {:?}: {:?}",
                            dest_cnode_cptr,
                            dest_slot,
                            e
                        );
                        e
                    })?;

                    // 重要：将更新后的 watermark 写回原始 Untyped 能力
                    cap.set_untyped_pages_and_watermark(untyped.pages, untyped.watermark);
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
        untypedmethod::RECYCLE => {
            let slot_ptr = tcb.lookup_slot(CapPtr::from(cptr)).ok_or(Error::InvalidCapability)?;
            let slot = unsafe { &*slot_ptr };

            if slot.cdt.first_child != VirtAddr::null() {
                warn!(
                    "Untyped::ResetWatermark denied: cap {:?} still has descendants",
                    CapPtr::from(cptr)
                );
                return Err(Error::ResourceBusy);
            }

            untyped.watermark = 0;
            cap.set_untyped_pages_and_watermark(untyped.pages, untyped.watermark);
            Ok(())
        }
        _ => {
            error!("Untyped::invoke failed: invalid method {}", method);
            Err(Error::InvalidMethod)
        }
    }
}
