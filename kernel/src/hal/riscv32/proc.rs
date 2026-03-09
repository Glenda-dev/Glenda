use core::arch::naked_asm;

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn switch_context(old: *mut ProcContext, new: *const ProcContext) {
    #[cfg(target_pointer_width = "64")]
    naked_asm!(
        // 保存旧上下文 (a0)
        "sd ra, 0(a0)",
        "sd sp, 8(a0)",
        "sd s0, 16(a0)",
        "sd s1, 24(a0)",
        "sd s2, 32(a0)",
        "sd s3, 40(a0)",
        "sd s4, 48(a0)",
        "sd s5, 56(a0)",
        "sd s6, 64(a0)",
        "sd s7, 72(a0)",
        "sd s8, 80(a0)",
        "sd s9, 88(a0)",
        "sd s10, 96(a0)",
        "sd s11, 104(a0)",
        // 加载新上下文 (a1)
        "ld ra, 0(a1)",
        "ld sp, 8(a1)",
        "ld s0, 16(a1)",
        "ld s1, 24(a1)",
        "ld s2, 32(a1)",
        "ld s3, 40(a1)",
        "ld s4, 48(a1)",
        "ld s5, 56(a1)",
        "ld s6, 64(a1)",
        "ld s7, 72(a1)",
        "ld s8, 80(a1)",
        "ld s9, 88(a1)",
        "ld s10, 96(a1)",
        "ld s11, 104(a1)",
        "ret",
    );
    #[cfg(target_pointer_width = "32")]
    naked_asm!(
        // 保存旧上下文 (a0)
        "sw ra, 0(a0)",
        "sw sp, 4(a0)",
        "sw s0, 8(a0)",
        "sw s1, 12(a0)",
        "sw s2, 16(a0)",
        "sw s3, 20(a0)",
        "sw s4, 24(a0)",
        "sw s5, 28(a0)",
        "sw s6, 32(a0)",
        "sw s7, 36(a0)",
        "sw s8, 40(a0)",
        "sw s9, 44(a0)",
        "sw s10, 48(a0)",
        "sw s11, 52(a0)",
        // 加载新上下文 (a1)
        "lw ra, 0(a1)",
        "lw sp, 4(a1)",
        "lw s0, 8(a1)",
        "lw s1, 12(a1)",
        "lw s2, 16(a1)",
        "lw s3, 20(a1)",
        "lw s4, 24(a1)",
        "lw s5, 28(a1)",
        "lw s6, 32(a1)",
        "lw s7, 36(a1)",
        "lw s8, 40(a1)",
        "lw s9, 44(a1)",
        "lw s10, 48(a1)",
        "lw s11, 52(a1)",
        "ret",
    );
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ProcContext {
    ra: usize, // 返回地址
    sp: usize, // 栈指针

    // callee保存的寄存器
    s0: usize,
    s1: usize,
    s2: usize,
    s3: usize,
    s4: usize,
    s5: usize,
    s6: usize,
    s7: usize,
    s8: usize,
    s9: usize,
    s10: usize,
    s11: usize,
}

impl ProcContext {
    /// 创建一个空的上下文
    pub const fn new() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s0: 0,
            s1: 0,
            s2: 0,
            s3: 0,
            s4: 0,
            s5: 0,
            s6: 0,
            s7: 0,
            s8: 0,
            s9: 0,
            s10: 0,
            s11: 0,
        }
    }
    pub fn configure(&mut self, entry_point: usize, stack_pointer: usize) {
        self.ra = entry_point;
        self.sp = stack_pointer;
    }
    pub fn set_fp(&mut self, fp: usize) {
        self.s0 = fp;
    }
}
