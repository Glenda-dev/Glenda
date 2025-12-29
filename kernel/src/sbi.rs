use core::arch::asm;

const SBI_EXT_HSM: usize = 0x48534d;
const SBI_EXT_TIME: usize = 0x54494D45;
const SBI_EXT_IPI: usize = 0x735049;

#[inline(always)]
unsafe fn sbi_call(eid: usize, fid: usize, arg0: usize, arg1: usize, arg2: usize) -> isize {
    let error;
    unsafe {
        asm!(
            "ecall",
            in("a7") eid,
            in("a6") fid,
            inlateout("a0") arg0 => error,
            inlateout("a1") arg1 => _,
            in("a2") arg2,
            options(nostack)
        );
    }
    error
}

pub fn set_timer(stime_value: u64) -> Result<(), isize> {
    let error = unsafe { sbi_call(SBI_EXT_TIME, 0, stime_value as usize, 0, 0) };
    if error == 0 { Ok(()) } else { Err(error) }
}

pub fn send_ipi(hart_mask: usize, hart_mask_base: usize) -> Result<(), isize> {
    // sbi_send_ipi(hart_mask, hart_mask_base)
    // 注意：SBI v0.2+ 接口参数略有不同，通常需要传入 mask 和 base
    let error = unsafe { sbi_call(SBI_EXT_IPI, 0, hart_mask, hart_mask_base, 0) };
    if error == 0 { Ok(()) } else { Err(error) }
}

pub fn hart_start(hartid: usize, start_addr: usize, opaque: usize) -> Result<(), isize> {
    let error = unsafe { sbi_call(SBI_EXT_HSM, 0, hartid, start_addr, opaque) };
    if error == 0 { Ok(()) } else { Err(error) }
}
