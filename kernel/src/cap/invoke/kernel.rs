use super::super::method::*;
use crate::cap::{Badge, CapPtr, CapType, Capability, Rights};
use crate::error::Error;
use crate::hal::mem::PGSIZE;
use crate::mem::{PhysAddr, PhysFrame};
use crate::platform::MemoryType;
use crate::proc::scheduler;

pub fn invoke_kernel(cap: &mut Capability, method: usize, cptr: usize) -> Result<(), Error> {
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => {
            error!("Kernel::invoke failed: no UTCB");
            return Err(Error::MappingFailed);
        }
    };

    match method {
        kernelmethod::SHELL => {
            if !cap.has_rights(Rights::EXECUTE) {
                error!("Kernel::Shell failed: permission denied");
                return Err(Error::PermissionDenied);
            }
            #[cfg(feature = "shell")]
            crate::shell::run();
            Ok(())
        }
        kernelmethod::GET_IRQ => {
            if !cap.has_rights(Rights::EXECUTE) {
                error!("Kernel::GET_IRQ failed: permission denied");
                return Err(Error::PermissionDenied);
            }
            let irq = utcb.mrs_regs[0];
            let dest_cptr = CapPtr::from(utcb.mrs_regs[1]);

            // 创建带有中断号作为值的 IRQ Cap
            let mut new_cap = Capability::create_irqhandler(irq, Rights::all());
            new_cap.set_badge(Badge::null()); // Badge 为空

            // 存入目标 Slot
            let root_cnode = tcb.get_cspace_mut().ok_or(Error::InvalidCapability)?;
            // 这里我们使用当前 Kernel Cap 所在的 Slot 作为 Parent
            let parent_slot = match tcb.lookup_slot(CapPtr::from(cptr)) {
                Some(s) => s,
                None => {
                    error!("Kernel::GET_IRQ: parent slot not found");
                    return Err(Error::InvalidCapability);
                }
            };

            root_cnode.insert_child(dest_cptr, &new_cap, parent_slot).map_err(|e| {
                error!("Kernel::GET_IRQ: insert failed at {:?}: {:?}", dest_cptr, e);
                e
            })?;
            Ok(())
        }
        kernelmethod::GET_MMIO => {
            if !cap.has_rights(Rights::EXECUTE) {
                error!("Kernel::GET_MMIO failed: permission denied");
                return Err(Error::PermissionDenied);
            }
            let paddr = PhysAddr::from(utcb.mrs_regs[0]);
            let pages = utcb.mrs_regs[1];
            let dest_cptr = CapPtr::from(utcb.mrs_regs[2]);

            if pages == 0 || paddr.as_usize() % PGSIZE != 0 {
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
                    if entry.kind == MemoryType::Ram || entry.kind == MemoryType::Reserved {
                        error!(
                            "Kernel::GET_MMIO: Requested range [{}, {}) overlaps with {:?} region [{}, {})",
                            start, end, entry.kind, r_start, r_end
                        );
                        return Err(Error::InvalidArgs);
                    }
                }
            }

            // 创建 Frame 能力 (标记为 is_device)
            let frame = PhysFrame { paddr, pages };
            let new_cap = Capability::create_frame(&frame, Rights::ALL, true);

            // 插入到目标槽位
            let root_cnode = tcb.get_cspace_mut().ok_or(Error::InvalidCapability)?;
            let parent_slot = match tcb.lookup_slot(CapPtr::from(cptr)) {
                Some(s) => s,
                None => {
                    error!("Kernel::GET_MMIO: parent slot not found");
                    return Err(Error::InvalidCapability);
                }
            };

            root_cnode.insert_child(dest_cptr, &new_cap, parent_slot).map_err(|e| {
                error!("Kernel::GET_MMIO: insert failed at {:?}: {:?}", dest_cptr, e);
                e
            })?;

            Ok(())
        }
        kernelmethod::SET_ALARM => {
            if !cap.has_rights(Rights::EXECUTE) {
                error!("Kernel::SET_ALARM failed: permission denied");
                return Err(Error::PermissionDenied);
            }
            let ms = utcb.mrs_regs[0];
            let ntfn_cptr = CapPtr::from(utcb.mrs_regs[1]);

            // 查找通知能力
            let ntfn_cap = match tcb.cap_lookup(ntfn_cptr) {
                Some(c) => {
                    if c.cap_type() != CapType::Endpoint {
                        error!("Kernel::SET_ALARM: capability is not an endpoint");
                        return Err(Error::InvalidCapability);
                    }
                    c
                }
                None => {
                    error!("Kernel::SET_ALARM: endpoint not found");
                    return Err(Error::InvalidCapability);
                }
            };

            crate::irq::timer::set_alarm(ms, ntfn_cap);
            Ok(())
        }
        _ => {
            error!("Kernel::invoke failed: invalid method {}", method);
            Err(Error::InvalidMethod)
        }
    }
}
