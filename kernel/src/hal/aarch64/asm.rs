use core::arch::asm;

#[inline(always)]
pub fn read_esr() -> usize {
    let esr: usize;
    unsafe {
        asm!("mrs {}, esr_el1", out(reg) esr, options(nomem, nostack, preserves_flags));
    }
    esr
}

#[inline(always)]
pub fn read_elr() -> usize {
    let elr: usize;
    unsafe {
        asm!("mrs {}, elr_el1", out(reg) elr, options(nomem, nostack, preserves_flags));
    }
    elr
}

#[inline(always)]
pub unsafe fn write_elr(value: usize) {
    unsafe {
        asm!("msr elr_el1, {}", in(reg) value, options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub fn read_far() -> usize {
    let far: usize;
    unsafe {
        asm!("mrs {}, far_el1", out(reg) far, options(nomem, nostack, preserves_flags));
    }
    far
}

#[inline(always)]
pub fn read_spsr() -> usize {
    let spsr: usize;
    unsafe {
        asm!("mrs {}, spsr_el1", out(reg) spsr, options(nomem, nostack, preserves_flags));
    }
    spsr
}

#[inline(always)]
pub unsafe fn write_spsr(value: usize) {
    unsafe {
        asm!("msr spsr_el1, {}", in(reg) value, options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub fn read_ttbr0() -> usize {
    let ttbr0: usize;
    unsafe {
        asm!("mrs {}, ttbr0_el1", out(reg) ttbr0, options(nomem, nostack, preserves_flags));
    }
    ttbr0
}

#[inline(always)]
pub unsafe fn write_ttbr0(value: usize) {
    unsafe {
        asm!("msr ttbr0_el1, {}", in(reg) value, options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub fn read_ttbr1() -> usize {
    let ttbr1: usize;
    unsafe {
        asm!("mrs {}, ttbr1_el1", out(reg) ttbr1, options(nomem, nostack, preserves_flags));
    }
    ttbr1
}

#[inline(always)]
pub unsafe fn write_ttbr1(value: usize) {
    unsafe {
        asm!("msr ttbr1_el1, {}", in(reg) value, options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub fn read_tpidr_el1() -> usize {
    let val: usize;
    unsafe {
        asm!("mrs {}, tpidr_el1", out(reg) val, options(nomem, nostack, preserves_flags));
    }
    val
}

#[inline(always)]
pub unsafe fn write_tpidr_el1(value: usize) {
    unsafe {
        asm!("msr tpidr_el1, {}", in(reg) value, options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub fn read_tpidr_el0() -> usize {
    let val: usize;
    unsafe {
        asm!("mrs {}, tpidr_el0", out(reg) val, options(nomem, nostack, preserves_flags));
    }
    val
}

#[inline(always)]
pub unsafe fn write_tpidr_el0(value: usize) {
    unsafe {
        asm!("msr tpidr_el0, {}", in(reg) value, options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub fn read_daif() -> usize {
    let val: usize;
    unsafe {
        asm!("mrs {}, daif", out(reg) val, options(nomem, nostack, preserves_flags));
    }
    val
}

#[inline(always)]
pub fn read_contextidr_el1() -> usize {
    let val: usize;
    unsafe {
        asm!("mrs {}, contextidr_el1", out(reg) val, options(nomem, nostack, preserves_flags));
    }
    val
}

#[inline(always)]
pub unsafe fn write_contextidr_el1(value: usize) {
    unsafe {
        asm!("msr contextidr_el1, {}", in(reg) value, options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub unsafe fn write_daif(value: usize) {
    unsafe {
        asm!("msr daif, {}", in(reg) value, options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub fn wfi() {
    unsafe {
        asm!("wfi", options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub fn isb() {
    unsafe {
        asm!("isb", options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub fn dsb_sy() {
    unsafe {
        asm!("dsb sy", options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub fn read_cntfrq() -> usize {
    let val: usize;
    unsafe {
        asm!("mrs {}, cntfrq_el0", out(reg) val, options(nomem, nostack, preserves_flags));
    }
    val
}

#[inline(always)]
pub fn read_cntvct() -> usize {
    let val: usize;
    unsafe {
        asm!("mrs {}, cntvct_el0", out(reg) val, options(nomem, nostack, preserves_flags));
    }
    val
}

#[inline(always)]
pub unsafe fn write_cntv_cval(value: u64) {
    unsafe {
        asm!("msr cntv_cval_el0, {}", in(reg) value, options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub unsafe fn write_cntv_ctl(value: u64) {
    unsafe {
        asm!("msr cntv_ctl_el0, {}", in(reg) value, options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub fn read_cntv_ctl() -> usize {
    let val: usize;
    unsafe {
        asm!("mrs {}, cntv_ctl_el0", out(reg) val, options(nomem, nostack, preserves_flags));
    }
    val
}

#[inline(always)]
pub fn read_mpidr() -> usize {
    let val: usize;
    unsafe {
        asm!("mrs {}, mpidr_el1", out(reg) val, options(nomem, nostack, preserves_flags));
    }
    val
}

#[inline(always)]
pub fn read_id_aa64pfr0() -> usize {
    let val: usize;
    unsafe {
        asm!("mrs {}, id_aa64pfr0_el1", out(reg) val, options(nomem, nostack, preserves_flags));
    }
    val
}

#[inline(always)]
pub unsafe fn write_vbar(value: usize) {
    unsafe {
        asm!("msr vbar_el1, {}", in(reg) value, options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub unsafe fn write_icc_sgi1r(value: u64) {
    unsafe {
        asm!("msr icc_sgi1r_el1, {}", in(reg) value, options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub unsafe fn write_mair(value: u64) {
    unsafe {
        asm!("msr mair_el1, {}", in(reg) value, options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub unsafe fn write_tcr(value: u64) {
    unsafe {
        asm!("msr tcr_el1, {}", in(reg) value, options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
pub unsafe fn write_ttbr0_zero() {
    unsafe {
        asm!("msr ttbr0_el1, xzr", "isb");
    }
}

#[inline(always)]
pub unsafe fn tlbi_vae1is(va_page: usize) {
    unsafe {
        asm!("tlbi vaae1is, {}", in(reg) va_page);
    }
}

#[inline(always)]
pub unsafe fn tlbi_vmalle1is() {
    unsafe {
        asm!("tlbi vmalle1is");
    }
}

#[inline(always)]
pub fn read_fp() -> usize {
    let ptr: usize;
    unsafe {
        asm!("mov {}, x29", out(reg) ptr);
    }
    ptr
}

#[inline(always)]
pub unsafe fn psci_hvc_call(func: usize, arg0: usize, arg1: usize, arg2: usize) -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "hvc #0",
            inlateout("x0") func => ret,
            in("x1") arg0,
            in("x2") arg1,
            in("x3") arg2,
            options(nostack)
        );
    }
    ret
}
