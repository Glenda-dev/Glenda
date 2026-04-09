use super::super::method::{vcpumethod, vmspacemethod};
use crate::cap::{CapType, Capability, Rights};
use crate::error::Error;
use crate::hal;
use crate::proc::scheduler;

pub fn invoke_vcpu(cap: &mut Capability, method: usize) -> Result<(), Error> {
    if cap.cap_type() != CapType::Vcpu {
        error!("Vcpu::invoke failed: invalid obj type {:?}", cap.cap_type());
        return Err(Error::InvalidType);
    }

    if !hal::virt::is_enabled() {
        error!("Vcpu::invoke failed: virtualization extension is not enabled");
        return Err(Error::NotSupported);
    }

    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = tcb.get_utcb().ok_or(Error::MappingFailed)?;

    if !cap.has_rights(Rights::CALL) {
        error!("Vcpu::invoke failed: permission denied");
        return Err(Error::PermissionDenied);
    }

    match method {
        vcpumethod::BIND_TCB => {
            let tcb_cptr = utcb.mrs_regs[0];
            let tcb_cap = tcb.cap_lookup(crate::cap::CapPtr::from(tcb_cptr)).ok_or_else(|| {
                error!("Vcpu::BindTcb failed: TCB cap not found cptr={}", tcb_cptr);
                Error::InvalidCapability
            })?;
            if tcb_cap.cap_type() != CapType::TCB {
                error!("Vcpu::BindTcb failed: cap is not TCB");
                return Err(Error::InvalidType);
            }
            Err(Error::NotImplemented)
        }
        vcpumethod::RUN => Err(Error::NotImplemented),
        vcpumethod::INJECT_IRQ => Err(Error::NotImplemented),
        vcpumethod::READ_REG => Err(Error::NotImplemented),
        vcpumethod::WRITE_REG => Err(Error::NotImplemented),
        _ => {
            error!("Vcpu::invoke failed: invalid method {}", method);
            Err(Error::InvalidMethod)
        }
    }
}

pub fn invoke_vmspace(cap: &mut Capability, method: usize) -> Result<(), Error> {
    if cap.cap_type() != CapType::Vmspace {
        error!("Vmspace::invoke failed: invalid obj type {:?}", cap.cap_type());
        return Err(Error::InvalidType);
    }

    if !hal::virt::is_enabled() {
        error!("Vmspace::invoke failed: virtualization extension is not enabled");
        return Err(Error::NotSupported);
    }

    if !cap.has_rights(Rights::CALL) {
        error!("Vmspace::invoke failed: permission denied");
        return Err(Error::PermissionDenied);
    }

    match method {
        vmspacemethod::MAP_STAGE2 => Err(Error::NotImplemented),
        vmspacemethod::UNMAP_STAGE2 => Err(Error::NotImplemented),
        vmspacemethod::SETUP_STAGE2 => Err(Error::NotImplemented),
        _ => {
            error!("Vmspace::invoke failed: invalid method {}", method);
            Err(Error::InvalidMethod)
        }
    }
}
