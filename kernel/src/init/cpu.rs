use crate::cpu;

pub fn init() {
    cpu::init();

    #[cfg(target_arch = "riscv64")]
    unsafe {
        // Enable Zicbom
        // Bit 6: CBCFE
        // Bit 7: CBIE
        // senvcfg CSR = 0x10A
        crate::hal::asm::senvcfg_set(6);
        crate::hal::asm::senvcfg_set(7);
    }
}
