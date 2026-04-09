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

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VcpuRunState {
    pub regs: [usize; VCPU_REG_COUNT],
    pub pc: usize,
    pub hgatp: usize,
    pub pending_virq_bitmap: usize,
}

pub fn init_cpu() {}

pub fn is_enabled() -> bool {
    false
}

pub fn setup_vmspace(_root: PhysAddr, _vmid: usize) -> Result<usize, Error> {
    Err(Error::NotSupported)
}

pub fn run_vcpu(_state: &mut VcpuRunState) -> Result<VirtExit, Error> {
    Err(Error::NotSupported)
}
