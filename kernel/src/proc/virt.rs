use crate::cap::CapType;
use crate::cap::Capability;
use crate::error::Error;
use crate::hal;
use crate::mem::PhysAddr;
use crate::sync::SpinLock;

#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VcpuExitReason {
    None = 0,
    HostTrap = 1,
    GuestPageFault = 2,
    VirtualInstruction = 3,
    ExternalInterrupt = 4,
    Unknown = 255,
}

#[repr(C)]
#[derive(Debug)]
pub struct VcpuState {
    pub lock: SpinLock<()>,
    pub bound_tcb: usize,
    pub vmspace_paddr: usize,
    pub pending_virq_bitmap: usize,
    pub regs: [usize; 32],
    pub exit_reason: VcpuExitReason,
    pub exit_detail0: usize,
    pub exit_detail1: usize,
    pub exit_detail2: usize,
}

impl VcpuState {
    pub const fn new() -> Self {
        Self {
            lock: SpinLock::new(()),
            bound_tcb: 0,
            vmspace_paddr: 0,
            pending_virq_bitmap: 0,
            regs: [0; 32],
            exit_reason: VcpuExitReason::None,
            exit_detail0: 0,
            exit_detail1: 0,
            exit_detail2: 0,
        }
    }

    pub fn bind_tcb(&mut self, tcb_ptr: usize) -> Result<(), Error> {
        if tcb_ptr == 0 {
            return Err(Error::InvalidArgs);
        }
        self.bound_tcb = tcb_ptr;
        Ok(())
    }

    pub fn bind_vmspace(&mut self, vmspace_paddr: PhysAddr) {
        self.vmspace_paddr = vmspace_paddr.as_usize();
    }

    pub fn inject_irq(&mut self, irq: usize) -> Result<(), Error> {
        if irq >= usize::BITS as usize {
            return Err(Error::InvalidArgs);
        }
        self.pending_virq_bitmap |= 1usize << irq;
        Ok(())
    }

    pub fn read_reg(&self, reg: usize) -> Result<usize, Error> {
        self.regs.get(reg).copied().ok_or(Error::InvalidArgs)
    }

    pub fn write_reg(&mut self, reg: usize, value: usize) -> Result<(), Error> {
        let slot = self.regs.get_mut(reg).ok_or(Error::InvalidArgs)?;
        *slot = value;
        Ok(())
    }

    pub fn run_once(&mut self) -> Result<VcpuExitReason, Error> {
        if self.bound_tcb == 0 {
            return Err(Error::InvalidCapability);
        }
        if !hal::virt::is_enabled() {
            return Err(Error::NotSupported);
        }

        if self.pending_virq_bitmap != 0 {
            self.exit_reason = VcpuExitReason::ExternalInterrupt;
            self.exit_detail0 = self.pending_virq_bitmap;
            self.pending_virq_bitmap = 0;
        } else {
            self.exit_reason = VcpuExitReason::HostTrap;
            self.exit_detail0 = 0;
        }
        Ok(self.exit_reason)
    }
}

pub fn vcpu_state_from_cap(cap: &Capability) -> Result<&'static mut VcpuState, Error> {
    if cap.cap_type() != CapType::Vcpu {
        return Err(Error::InvalidType);
    }
    let vaddr = cap.obj_ptr();
    Ok(unsafe { vaddr.as_mut::<VcpuState>() })
}
