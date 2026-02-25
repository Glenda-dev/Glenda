use num_enum::FromPrimitive;

use super::super::method::*;
use crate::cap::{CapPtr, CapType, Capability};
use crate::error::Error;
use crate::mem::UntypedRegion;
use crate::proc::scheduler;

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

            let full_dest = CapPtr::concat(dest_cnode_cptr, dest_slot);

            let root_cnode = tcb.get_cspace_mut().ok_or(Error::InvalidCapability)?;

            match untyped.retype(obj_type, flags) {
                Some(new_cap) => {
                    let slot_ptr = match tcb.lookup_slot(CapPtr::from(cptr)) {
                        Some(s) => s,
                        None => {
                            error!("Untyped::invoke failed: parent slot not found");
                            return Err(Error::InvalidCapability);
                        }
                    };

                    root_cnode.insert_child(full_dest, &new_cap, slot_ptr).map_err(|e| {
                        error!(
                            "Untyped::Retype failed: insert child failed at {:?}: {:?}",
                            full_dest, e
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
        untypedmethod::MERGE => {
            let other_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let root_cnode = tcb.get_cspace_mut().ok_or(Error::InvalidCapability)?;

            // 查找第二个能力
            let other_slot_ptr = tcb.lookup_slot(other_cptr).ok_or(Error::InvalidCapability)?;
            let other_cap = unsafe { &mut (*other_slot_ptr).cap };

            // 检查类型
            if cap.cap_type() != CapType::Untyped || other_cap.cap_type() != CapType::Untyped {
                return Err(Error::InvalidType);
            }

            // 检查权限 - 必须有相同的权限位 (bits 5-12)
            use crate::cap::capability::{RIGHTS_MASK, RIGHTS_SHIFT, TYPE_MASK};
            let mask = TYPE_MASK | (RIGHTS_MASK << RIGHTS_SHIFT);
            if (cap.words[1] & mask) != (other_cap.words[1] & mask) {
                error!("Untyped::Merge failed: rights mismatch");
                return Err(Error::PermissionDenied);
            }

            let u1 = UntypedRegion::from_cap(cap).unwrap();
            let u2 = UntypedRegion::from_cap(other_cap).unwrap();

            // 检查 watermark - 必须整洁 (没有子对象)
            if u1.watermark != 0 || u2.watermark != 0 {
                error!(
                    "Untyped::Merge failed: watermark not zero ({}, {})",
                    u1.watermark, u2.watermark
                );
                return Err(Error::ResourceBusy);
            }

            // 检查物理位置是否相邻
            use crate::hal::mem::PGSIZE;
            let start1 = u1.start.as_usize();
            let end1 = start1 + u1.pages * PGSIZE;
            let start2 = u2.start.as_usize();
            let end2 = start2 + u2.pages * PGSIZE;

            if end1 == start2 {
                // u1 在前, u2 在后
                let total_pages = u1.pages + u2.pages;
                if total_pages > 0x1FFFFFF {
                    return Err(Error::InvalidArgs);
                }

                // 更新当前能力的大小
                cap.set_data(total_pages | (0 << 25));
                // 删除第二个能力 (通过 CapPtr)
                root_cnode.delete(other_cptr)?;

                log!(
                    "Untyped::MERGE: merged [0x{:x}, 0x{:x}) and [0x{:x}, 0x{:x}) into [0x{:x}, 0x{:x})",
                    start1,
                    end1,
                    start2,
                    end2,
                    start1,
                    start1 + total_pages * PGSIZE
                );
                Ok(())
            } else if end2 == start1 {
                // u2 在前, u1 在后
                let total_pages = u1.pages + u2.pages;
                if total_pages > 0x1FFFFFF {
                    return Err(Error::InvalidArgs);
                }

                // 更新基址和大小
                cap.words[0] = start2;
                cap.set_data(total_pages | (0 << 25));
                // 删除第二个能力
                root_cnode.delete(other_cptr)?;

                log!(
                    "Untyped::MERGE: merged [0x{:x}, 0x{:x}) and [0x{:x}, 0x{:x}) into [0x{:x}, 0x{:x})",
                    start2,
                    end2,
                    start1,
                    end1,
                    start2,
                    start2 + total_pages * PGSIZE
                );
                Ok(())
            } else {
                error!(
                    "Untyped::Merge failed: regions not adjacent: [0x{:x}, 0x{:x}) and [0x{:x}, 0x{:x})",
                    start1, end1, start2, end2
                );
                Err(Error::InvalidAddress)
            }
        }
        _ => {
            error!("Untyped::invoke failed: invalid method {}", method);
            Err(Error::InvalidMethod)
        }
    }
}
