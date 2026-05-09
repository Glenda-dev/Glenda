use crate::error::Error;
use crate::mem::PhysAddr;

pub const VCPU_REG_COUNT: usize = 32;

#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtExitReason {
    None = 0,
    HostTrap = 1,
    GuestPageFault = 2,
    VirtualInstruction = 3,
    ExternalInterrupt = 4,
    TimerInterrupt = 5,
    Unknown = 255,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VirtExit {
    pub reason: VirtExitReason,
    pub detail0: usize,
    pub detail1: usize,
    pub detail2: usize,
}

impl VirtExit {
    pub const fn none() -> Self {
        Self { reason: VirtExitReason::None, detail0: 0, detail1: 0, detail2: 0 }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VcpuRunState {
    pub regs: [usize; VCPU_REG_COUNT],
    pub pc: usize,
    pub hgatp: usize,
    pub pending_virq_bitmap: usize,
}

pub fn is_enabled() -> bool {
    super::platform::is_virtualization_enabled()
}

pub fn init_cpu() {
    if !is_enabled() {
        return;
    }
}

pub fn setup_vmspace(root: PhysAddr, vmid: usize) -> Result<usize, Error> {
    if !is_enabled() {
        return Err(Error::NotSupported);
    }
    if !root.is_aligned(4096) {
        return Err(Error::InvalidAddress);
    }

    // Treat "hgatp" as architecture-specific opaque VM control value on aarch64.
    // Bits: [63:48] VMID, [47:1] root base (aligned), [0] valid marker.
    let value = ((vmid & 0xffff) << 48) | (root.as_usize() & !0xfff) | 1;
    Ok(value)
}

pub fn run_vcpu(state: &mut VcpuRunState) -> Result<VirtExit, Error> {
    if !is_enabled() {
        return Err(Error::NotSupported);
    }
    if state.hgatp == 0 {
        return Err(Error::InvalidArgs);
    }

    if state.pending_virq_bitmap != 0 {
        let pending = state.pending_virq_bitmap;
        state.pending_virq_bitmap = 0;
        return Ok(VirtExit {
            reason: VirtExitReason::ExternalInterrupt,
            detail0: pending,
            detail1: state.pc,
            detail2: 0,
        });
    }

    Ok(VirtExit { reason: VirtExitReason::HostTrap, detail0: state.pc, detail1: 0, detail2: 0 })
}
