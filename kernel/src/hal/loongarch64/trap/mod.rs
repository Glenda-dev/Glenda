use super::csr;
use core::arch::global_asm;
use crate::trap::cause::TrapCause;

global_asm!(
    r#"
    .align 12
    .globl __exception_vector
__exception_vector:
1:
    b 1b
    "#
);

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TrapFrame {
    pub regs: [usize; 32],
    pub era: usize,
    pub prmd: usize,
    pub badv: usize,
}

impl TrapFrame {
    pub const fn new() -> Self {
        Self {
            regs: [0; 32],
            era: 0,
            prmd: 0,
            badv: 0,
        }
    }

    pub fn configure(&mut self, entry: usize, sp: usize, _tp: usize) {
        self.era = entry;
        self.regs[3] = sp;
    }

    pub fn configure_kernel(&mut self, _mmu: usize, _cpuid: usize, _kstack: usize, _handler: usize) {
    }

    pub fn set_cpuid(&mut self, _id: usize) {}

    pub fn advance_pc(&mut self) {
        self.era += 4;
    }

    pub fn set_return_value(&mut self, ret: usize) {
        self.regs[4] = ret;
    }

    pub fn get_syscall_args(&self) -> (usize, usize) {
        (self.regs[4], self.regs[11])
    }

    pub fn get_epc(&self) -> usize { self.era }
    pub fn get_ra(&self) -> usize { self.regs[1] }
    pub fn get_sp(&self) -> usize { self.regs[3] }

    pub fn set_registers(&mut self, regs: &[usize]) {
        let len = regs.len().min(32);
        self.regs[..len].copy_from_slice(&regs[..len]);
    }

    pub fn get_registers(&self) -> [usize; 8] {
        let mut res = [0usize; 8];
        res.copy_from_slice(&self.regs[4..12]);
        res
    }
}

pub fn get_cause() -> usize { csr::read_csr(csr::CSR_ESTAT) }
pub fn get_pc() -> usize { csr::read_csr(csr::CSR_ERA) }
pub fn get_value() -> usize { csr::read_csr(csr::CSR_BADV) }
pub fn get_status() -> usize { csr::read_csr(csr::CSR_CRMD) }
pub fn is_user_mode(status: usize) -> bool { (status & 0x3) != 0 }

pub fn match_cause(_cause: usize) -> TrapCause {
    TrapCause::Unknown(0)
}

pub fn vector_init() {
    unsafe {
        unsafe extern "C" {
            fn __exception_vector();
        }
        csr::write_csr(csr::CSR_EENTRY, __exception_vector as usize);
    }
}

pub fn trap_user_handler() {}
pub fn trap_user_return() {}
