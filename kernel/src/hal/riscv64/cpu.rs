use super::asm;

pub const MAX_CPUS: usize = 8;

pub fn cpu_id() -> usize {
    asm::read_tp()
}
pub fn read_cycle() -> usize {
    asm::rdcycle()
}
pub fn set_cpuid(cpuid: usize) {
    unsafe {
        asm::write_tp(cpuid);
    }
}

pub fn init() {
    unsafe {
        // Enable Zicbom
        // Bit 6: CBCFE
        // Bit 7: CBIE
        // senvcfg CSR = 0x10A
        asm::senvcfg_set(6);
        asm::senvcfg_set(7);
    }
}
