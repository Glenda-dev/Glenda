use core::arch::asm;

const SBI_EXT_HSM: usize = 0x48534d;
const SBI_EXT_SRST: usize = 0x53525354;
const SBI_EXT_TIME: usize = 0x54494D45;
const SBI_EXT_IPI: usize = 0x735049;
const SBI_EXT_0_1_SET_TIMER: usize = 0x0;

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

pub fn set_timer(stime_value: usize) -> Result<(), isize> {
    let error = unsafe { sbi_call(SBI_EXT_TIME, 0, stime_value as usize, 0, 0) };
    if error == 0 {
        Ok(())
    } else if error == -2 {
        // SBI_ERR_NOT_SUPPORTED: Fallback to legacy SBI
        unsafe { sbi_call(SBI_EXT_0_1_SET_TIMER, 0, stime_value as usize, 0, 0) };
        Ok(())
    } else {
        Err(error)
    }
}

pub fn send_ipi(hart_mask: usize, hart_mask_base: usize) -> Result<(), isize> {
    // sbi_send_ipi(hart_mask, hart_mask_base)
    // 注意：SBI v0.2+ 接口参数略有不同，通常需要传入 mask 和 base
    let error = unsafe { sbi_call(SBI_EXT_IPI, 0, hart_mask, hart_mask_base, 0) };
    if error == 0 { Ok(()) } else { Err(error) }
}

pub fn send_hsm(hartid: usize, command: usize, arg0: usize, opaque: usize) -> Result<usize, isize> {
    let error = unsafe { sbi_call(SBI_EXT_HSM, command, hartid, arg0, opaque) };
    if error >= 0 { Ok(error as usize) } else { Err(error) }
}

pub fn system_reset(reset_type: usize, reset_reason: usize) -> Result<(), isize> {
    let error = unsafe { sbi_call(SBI_EXT_SRST, 0, reset_type, reset_reason, 0) };
    if error == 0 { Ok(()) } else { Err(error) }
}

const SBI_EXT_RFENCE: usize = 0x52464E43;
const SBI_FID_REMOTE_SFENCE_VMA: usize = 0;
const SBI_FID_REMOTE_SFENCE_VMA_ASID: usize = 1;

pub fn remote_sfence_vma(hart_mask: usize, hart_mask_base: usize, addr: usize, size: usize) {
    let _ = unsafe {
        sbi_call(SBI_EXT_RFENCE, SBI_FID_REMOTE_SFENCE_VMA, hart_mask, hart_mask_base, addr);
        sbi_call(SBI_EXT_RFENCE, SBI_FID_REMOTE_SFENCE_VMA, 0, 0, size); // size needs to be passed too, but standard SBI interface is a bit more complex for size. 
        // Simplification: We use the v0.2 RFENCE extension.
        // Real SBI call for RFENCE: a0=hart_mask, a1=hart_mask_base, a2=start_addr, a3=size
        asm!(
            "ecall",
            in("a7") SBI_EXT_RFENCE,
            in("a6") SBI_FID_REMOTE_SFENCE_VMA,
            in("a0") hart_mask,
            in("a1") hart_mask_base,
            in("a2") addr,
            in("a3") size,
        );
    };
}

pub fn remote_sfence_vma_asid(
    hart_mask: usize,
    hart_mask_base: usize,
    addr: usize,
    size: usize,
    asid: usize,
) {
    let _ = unsafe {
        asm!(
            "ecall",
            in("a7") SBI_EXT_RFENCE,
            in("a6") SBI_FID_REMOTE_SFENCE_VMA_ASID,
            in("a0") hart_mask,
            in("a1") hart_mask_base,
            in("a2") addr,
            in("a3") size,
            in("a4") asid,
        );
    };
}

pub fn put_char(c: u8) {
    let _ = unsafe { sbi_call(1, 0, c as usize, 0, 0) };
}

pub fn get_char() -> Option<u8> {
    let ret = unsafe { sbi_call(1, 1, 0, 0, 0) };
    if ret < 0 { None } else { Some(ret as u8) }
}
