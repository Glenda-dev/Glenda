use core::arch::asm;

pub const CSR_CRMD: usize = 0x0;
pub const CSR_PRMD: usize = 0x1;
pub const CSR_ECFG: usize = 0x4;
pub const CSR_ESTAT: usize = 0x5;
pub const CSR_ERA: usize = 0x6;
pub const CSR_BADV: usize = 0x7;
pub const CSR_EENTRY: usize = 0xc;
pub const CSR_CPUID: usize = 0x20;
pub const CSR_DMW0: usize = 0x180;
pub const CSR_DMW1: usize = 0x181;

pub const CRMD_IE: usize = 1 << 2;

#[inline(always)]
pub fn read_csr(csr_num: usize) -> usize {
    let val: usize;
    unsafe {
        match csr_num {
            CSR_CRMD => asm!("csrrd {}, 0x0", out(reg) val),
            CSR_PRMD => asm!("csrrd {}, 0x1", out(reg) val),
            CSR_ECFG => asm!("csrrd {}, 0x4", out(reg) val),
            CSR_ESTAT => asm!("csrrd {}, 0x5", out(reg) val),
            CSR_ERA => asm!("csrrd {}, 0x6", out(reg) val),
            CSR_BADV => asm!("csrrd {}, 0x7", out(reg) val),
            CSR_EENTRY => asm!("csrrd {}, 0xc", out(reg) val),
            CSR_CPUID => {
                asm!("csrrd {}, 0x20", out(reg) val);
                return val & 0x1FF;
            }
            CSR_DMW0 => asm!("csrrd {}, 0x180", out(reg) val),
            CSR_DMW1 => asm!("csrrd {}, 0x181", out(reg) val),
            _ => panic!("Unsupported CSR read"),
        }
    }
    val
}

#[inline(always)]
pub unsafe fn write_csr(csr_num: usize, val: usize) {
    match csr_num {
        CSR_CRMD => asm!("csrwr {}, 0x0", in(reg) val),
        CSR_PRMD => asm!("csrwr {}, 0x1", in(reg) val),
        CSR_ECFG => asm!("csrwr {}, 0x4", in(reg) val),
        CSR_ESTAT => asm!("csrwr {}, 0x5", in(reg) val),
        CSR_ERA => asm!("csrwr {}, 0x6", in(reg) val),
        CSR_BADV => asm!("csrwr {}, 0x7", in(reg) val),
        CSR_EENTRY => asm!("csrwr {}, 0xc", in(reg) val),
        CSR_DMW0 => asm!("csrwr {}, 0x180", in(reg) val),
        CSR_DMW1 => asm!("csrwr {}, 0x181", in(reg) val),
        _ => panic!("Unsupported CSR write"),
    }
}

#[inline(always)]
pub unsafe fn xchg_csr(csr_num: usize, val: usize, mask: usize) -> usize {
    let mut old = val;
    match csr_num {
        CSR_CRMD => asm!("csrxchg {}, {}, 0x0", inout(reg) old, in(reg) mask),
        CSR_PRMD => asm!("csrxchg {}, {}, 0x1", inout(reg) old, in(reg) mask),
        CSR_ECFG => asm!("csrxchg {}, {}, 0x4", inout(reg) old, in(reg) mask),
        _ => panic!("Unsupported CSR xchg"),
    }
    old
}
