use crate::cap::CapType;
use crate::cap::Capability;
use crate::error::Error;
use crate::hal;
use crate::hal::virt::{VcpuRunState, VirtExitReason};
use crate::mem::{PhysAddr, addr::phys_to_virt};
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
    pub hgatp: usize,
    pub vmid: usize,
    pub pending_virq_bitmap: usize,
    pub regs: [usize; 32],
    pub pc: usize,
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
            hgatp: 0,
            vmid: 0,
            pending_virq_bitmap: 0,
            regs: [0; 32],
            pc: 0,
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
        self.vmid = (vmspace_paddr.as_usize() >> 12) & 0x3fff;
        self.hgatp = hal::virt::setup_vmspace(vmspace_paddr, self.vmid).unwrap_or(0);
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
        if self.vmspace_paddr == 0 || self.hgatp == 0 {
            return Err(Error::InvalidCapability);
        }
        if !hal::virt::is_enabled() {
            return Err(Error::NotSupported);
        }

        let mut run = VcpuRunState {
            regs: self.regs,
            pc: self.pc,
            hgatp: self.hgatp,
            pending_virq_bitmap: self.pending_virq_bitmap,
        };
        let exit = hal::virt::run_vcpu(&mut run)?;
        self.regs = run.regs;
        self.pc = run.pc;
        self.pending_virq_bitmap = run.pending_virq_bitmap;

        self.exit_reason = match exit.reason {
            VirtExitReason::None => VcpuExitReason::None,
            VirtExitReason::HostTrap => VcpuExitReason::HostTrap,
            VirtExitReason::GuestPageFault => VcpuExitReason::GuestPageFault,
            VirtExitReason::VirtualInstruction => VcpuExitReason::VirtualInstruction,
            VirtExitReason::ExternalInterrupt => VcpuExitReason::ExternalInterrupt,
            VirtExitReason::TimerInterrupt => VcpuExitReason::ExternalInterrupt,
            VirtExitReason::Unknown => VcpuExitReason::Unknown,
        };
        self.exit_detail0 = exit.detail0;
        self.exit_detail1 = exit.detail1;
        self.exit_detail2 = exit.detail2;

        // 把关键 guest 执行状态回写到绑定线程 trapframe，供 VMM 读取/调度。
        let tcb = unsafe { &mut *(self.bound_tcb as *mut crate::proc::TCB) };
        let tf = tcb.get_tf();
        tf.set_epc(self.pc);
        tf.set_registers(&[
            self.regs[10],
            self.regs[11],
            self.regs[12],
            self.regs[13],
            self.regs[14],
            self.regs[15],
            self.regs[16],
            self.regs[17],
        ]);

        if self.exit_reason == VcpuExitReason::GuestPageFault && self.vmspace_paddr != 0 {
            let vm_pt = unsafe {
                phys_to_virt(PhysAddr::from(self.vmspace_paddr)).as_mut::<crate::mem::PageTable>()
            };
            let _ = vm_pt.walk(crate::mem::VirtAddr::from(self.exit_detail0));
        }
        Ok(self.exit_reason)
    }
}

pub fn vcpu_state_from_cap(cap: &Capability) -> Result<&'static mut VcpuState, Error> {
    if cap.cap_type() != CapType::VCPU {
        return Err(Error::InvalidType);
    }
    let vaddr = cap.obj_ptr();
    Ok(unsafe { vaddr.as_mut::<VcpuState>() })
}
