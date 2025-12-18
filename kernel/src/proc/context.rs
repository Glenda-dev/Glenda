use core::arch::asm;
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ProcContext {
    pub ra: usize, // 返回地址
    pub sp: usize, // 栈指针

    // callee保存的寄存器
    pub s0: usize,
    pub s1: usize,
    pub s2: usize,
    pub s3: usize,
    pub s4: usize,
    pub s5: usize,
    pub s6: usize,
    pub s7: usize,
    pub s8: usize,
    pub s9: usize,
    pub s10: usize,
    pub s11: usize,
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
}

pub fn switch(old: &mut ProcContext, new: &ProcContext) {
    unsafe {
        asm!(
            // 保存旧上下文
            "sd ra, 0({0})",
            "sd sp, 8({0})",
            "sd s0, 16({0})",
            "sd s1, 24({0})",
            "sd s2, 32({0})",
            "sd s3, 40({0})",
            "sd s4, 48({0})",
            "sd s5, 56({0})",
            "sd s6, 64({0})",
            "sd s7, 72({0})",
            "sd s8, 80({0})",
            "sd s9, 88({0})",
            "sd s10, 96({0})",
            "sd s11, 104({0})",
            // 加载新上下文
            "ld ra, 0({1})",
            "ld sp, 8({1})",
            "ld s0, 16({1})",
            "ld s1, 24({1})",
            "ld s2, 32({1})",
            "ld s3, 40({1})",
            "ld s4, 48({1})",
            "ld s5, 56({1})",
            "ld s6, 64({1})",
            "ld s7, 72({1})",
            "ld s8, 80({1})",
            "ld s9, 88({1})",
            "ld s10, 96({1})",
            "ld s11, 104({1})",
            // 跳转到新上下文的返回地址
            "jr ra",
            in(reg) old,
            in(reg) new,
            options(noreturn)
        );
    }
}
