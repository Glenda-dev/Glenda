use crate::cap::method::*;
use crate::cap::{CapPtr, Capability, Rights};
use crate::error::Error;
use crate::hal::mem::PGSIZE;
use crate::mem::{PhysAddr, PhysFrame};
use crate::platform::MemoryType;
use crate::proc::scheduler;

pub fn invoke_mmio(_cap: &mut Capability, method: usize, cptr: usize) -> Result<(), Error> {
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => return Err(Error::MappingFailed),
    };

    match method {
        mmiomethod::GET_FRAME => {
            let paddr = PhysAddr::from(utcb.mrs_regs[0]);
            let pages = utcb.mrs_regs[1];
            let dest_cptr = CapPtr::from(utcb.mrs_regs[2]);

            if pages == 0 {
                return Err(Error::InvalidArgs);
            }

            let start = paddr;
            let end = paddr + (pages * PGSIZE);

            // 检查内存范围是否与 RAM 或 Reserved 重合
            let mmap = crate::boot::get_mem_map();
            for entry in mmap {
                let r_start = entry.base;
                let r_end = entry.base + entry.length;

                // 检查重叠
                if start < r_end && end > r_start {
                    if entry.kind == MemoryType::Ram
                        || entry.kind == MemoryType::Reserved
                        || entry.kind == MemoryType::Reclaimable
                    {
                        error!(
                            "Mmio::GET_FRAME: Requested range [{}, {}) overlaps with {:?} region [{}, {})",
                            start, end, entry.kind, r_start, r_end
                        );
                        return Err(Error::InvalidArgs);
                    }
                }
            }

            // 创建 Frame 能力
            let frame = PhysFrame { paddr, pages };
            let new_cap = Capability::create_frame(&frame, Rights::ALL);

            // 插入到目标槽位
            let root_cnode = tcb.get_cspace_mut().ok_or(Error::InvalidCapability)?;

            let slot_ptr = match tcb.lookup_slot(CapPtr::from(cptr)) {
                Some(s) => s,
                None => {
                    error!("Mmio::GET_FRAME: parent slot not found");
                    return Err(Error::InvalidCapability);
                }
            };

            root_cnode.insert_child(dest_cptr, &new_cap, slot_ptr).map_err(|e| {
                error!("Mmio::GET_FRAME: insert failed at {:?}: {:?}", dest_cptr, e);
                e
            })?;

            Ok(())
        }
        _ => Err(Error::InvalidMethod),
    }
}
