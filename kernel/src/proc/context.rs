use core::arch::naked_asm;
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
    // 这是一个 hack，用于在 switch_context 后传递参数给 ra 指向的函数
    // switch_context 不会恢复 a0，但我们可以手动将其放在栈上或者修改 switch_context
    // 但最简单的方法是让 switch_context 恢复 s0/s1，然后让 wrapper 函数从 s0/s1 移动到 a0
    // 或者，我们修改 switch_context 来恢复更多的寄存器？不，这会增加开销。

    // 更好的方法：
    // 我们不修改 ProcContext 结构，而是利用 switch_context 的特性。
    // switch_context(old, new) -> old 是 a0, new 是 a1
    // 当我们切换到新线程时，ra 会从栈上恢复。
    // 如果我们想给新线程传递参数，通常需要一个 trampoline。
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

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn switch_context(old: *mut ProcContext, new: *const ProcContext) {
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
}
