use super::super::method::{vcpumethod, vmspacemethod};
use crate::cap::{CapPtr, CapType, Capability, Rights};
use crate::error::Error;
use crate::hal;
use crate::hal::mem::PGSIZE;
use crate::ipc::{MsgFlags, MsgTag, protocol};
use crate::mem::{PageTable, Perms, VirtAddr, addr::phys_to_virt};
use crate::proc::scheduler;
use crate::proc::virt::vcpu_state_from_cap;

pub fn invoke_vcpu(cap: &mut Capability, method: usize) -> Result<(), Error> {
    if cap.cap_type() != CapType::VCPU {
        error!("VCPU::invoke failed: invalid obj type {:?}", cap.cap_type());
        return Err(Error::InvalidType);
    }

    if !hal::virt::is_enabled() {
        error!("VCPU::invoke failed: virtualization extension is not enabled");
        return Err(Error::NotSupported);
    }

    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = tcb.get_utcb().ok_or(Error::MappingFailed)?;

    if !cap.has_rights(Rights::CALL) {
        error!("VCPU::invoke failed: permission denied");
        return Err(Error::PermissionDenied);
    }

    match method {
        vcpumethod::BIND_TCB => {
            let tcb_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let tcb_cap = tcb.cap_lookup(tcb_cptr).ok_or_else(|| {
                error!("VCPU::BindTcb failed: TCB cap not found cptr={}", tcb_cptr);
                Error::InvalidCapability
            })?;
            if tcb_cap.cap_type() != CapType::TCB {
                error!("VCPU::BindTcb failed: cap is not TCB");
                return Err(Error::InvalidType);
            }
            let bound_tcb = unsafe { tcb_cap.obj_ptr().as_mut::<crate::proc::TCB>() };

            let vcpu = vcpu_state_from_cap(cap)?;
            vcpu.bind_tcb(tcb_cap.obj_ptr().as_usize())?;
            let vmspace_cptr = CapPtr::from(utcb.mrs_regs[1]);
            if vmspace_cptr.bits() != 0 {
                let vmspace_cap = tcb.cap_lookup(vmspace_cptr).ok_or_else(|| {
                    error!("VCPU::BindTcb failed: VMSpace cap not found cptr={}", vmspace_cptr);
                    Error::InvalidCapability
                })?;
                if vmspace_cap.cap_type() != CapType::VMSpace {
                    error!("VCPU::BindTcb failed: cap is not VMSpace");
                    return Err(Error::InvalidType);
                }
                vcpu.bind_vmspace(vmspace_cap.paddr());
            }
            bound_tcb.set_bound_vcpu(cap.clone());
            Ok(())
        }
        vcpumethod::RUN => {
            let vcpu = vcpu_state_from_cap(cap)?;
            let _ = vcpu.run_once()?;

            utcb.mrs_regs[0] = vcpu.exit_reason as usize;
            utcb.mrs_regs[1] = vcpu.exit_detail0;
            utcb.mrs_regs[2] = vcpu.exit_detail1;
            utcb.mrs_regs[3] = vcpu.exit_detail2;
            utcb.msg_tag = MsgTag::new(protocol::KERNEL_PROTO, protocol::VIRT_EXIT, MsgFlags::NONE);
            Ok(())
        }
        vcpumethod::INJECT_IRQ => {
            let irq = utcb.mrs_regs[0];
            let vcpu = vcpu_state_from_cap(cap)?;
            vcpu.inject_irq(irq)
        }
        vcpumethod::READ_REG => {
            let reg = utcb.mrs_regs[0];
            let vcpu = vcpu_state_from_cap(cap)?;
            utcb.mrs_regs[0] = vcpu.read_reg(reg)?;
            Ok(())
        }
        vcpumethod::WRITE_REG => {
            let reg = utcb.mrs_regs[0];
            let value = utcb.mrs_regs[1];
            let vcpu = vcpu_state_from_cap(cap)?;
            vcpu.write_reg(reg, value)
        }
        _ => {
            error!("VCPU::invoke failed: invalid method {}", method);
            Err(Error::InvalidMethod)
        }
    }
}

pub fn invoke_vmspace(cap: &mut Capability, method: usize) -> Result<(), Error> {
    if cap.cap_type() != CapType::VMSpace {
        error!("VMSpace::invoke failed: invalid obj type {:?}", cap.cap_type());
        return Err(Error::InvalidType);
    }

    if !hal::virt::is_enabled() {
        error!("VMSpace::invoke failed: virtualization extension is not enabled");
        return Err(Error::NotSupported);
    }

    if !cap.has_rights(Rights::CALL) {
        error!("VMSpace::invoke failed: permission denied");
        return Err(Error::PermissionDenied);
    }

    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = tcb.get_utcb().ok_or(Error::MappingFailed)?;
    let vm_pt = unsafe { phys_to_virt(cap.paddr()).as_mut::<PageTable>() };

    match method {
        vmspacemethod::MAP_STAGE2 => {
            // map_stage2(frame_cptr, guest_paddr, host_paddr, pages)
            let frame_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let guest_paddr = VirtAddr::from(utcb.mrs_regs[1]);
            let host_paddr = crate::mem::PhysAddr::from(utcb.mrs_regs[2]);
            let mut pages = utcb.mrs_regs[3];

            let frame_cap = tcb.cap_lookup(frame_cptr).ok_or_else(|| {
                error!("VMSpace::MapStage2 failed: frame cap not found cptr={}", frame_cptr);
                Error::InvalidCapability
            })?;
            if frame_cap.cap_type() != CapType::Page {
                error!("VMSpace::MapStage2 failed: cap is not Frame");
                return Err(Error::InvalidType);
            }
            let frame_pages = frame_cap.page_pages().ok_or_else(|| {
                error!("VMSpace::MapStage2 failed: page metadata missing");
                Error::InvalidCapability
            })?;
            if pages == 0 {
                pages = frame_pages;
            } else if pages > frame_pages {
                error!(
                    "VMSpace::MapStage2 failed: requested pages {} exceeds page span {}",
                    pages, frame_pages
                );
                return Err(Error::InvalidArgs);
            }

            vm_pt
                .map(
                    guest_paddr,
                    host_paddr,
                    pages * PGSIZE,
                    Perms::READ | Perms::WRITE | Perms::EXECUTE,
                )
                .map_err(|_| Error::MappingFailed)
        }
        vmspacemethod::UNMAP_STAGE2 => {
            let guest_paddr = VirtAddr::from(utcb.mrs_regs[0]);
            let pages = utcb.mrs_regs[1];
            if pages == 0 {
                return Err(Error::InvalidArgs);
            }
            vm_pt.unmap(guest_paddr, pages * PGSIZE).map_err(|_| Error::UnmappingFailed)
        }
        vmspacemethod::SETUP_STAGE2 => {
            // 轻量初始化：清 root 表并允许后续 map_stage2 构建
            unsafe {
                core::ptr::write_bytes(vm_pt as *mut PageTable as *mut u8, 0, PGSIZE);
            }
            Ok(())
        }
        _ => {
            error!("VMSpace::invoke failed: invalid method {}", method);
            Err(Error::InvalidMethod)
        }
    }
}
