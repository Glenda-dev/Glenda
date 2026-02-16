use super::csr;

pub const MAX_CPUS: usize = 4;

pub fn cpu_id() -> usize {
    csr::read_csr(csr::CSR_CPUID)
}

pub fn halt() -> ! {
    loop {
        unsafe { core::arch::asm!("idle 0"); }
    }
}
