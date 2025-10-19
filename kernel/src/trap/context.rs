/// 陷阱处理时的寄存器上下文结构
/// 对应汇编代码中栈上的布局
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TrapContext {
    // 通用寄存器（x0-x31）
    // 注意：x0(zero)不能修改，所以这里占位但不使用
    pub ra: usize,  // x1
    pub sp: usize,  // x2
    pub gp: usize,  // x3
    pub tp: usize,  // x4
    pub t0: usize,  // x5
    pub t1: usize,  // x6
    pub t2: usize,  // x7
    pub s0: usize,  // x8
    pub s1: usize,  // x9
    pub a0: usize,  // x10
    pub a1: usize,  // x11
    pub a2: usize,  // x12
    pub a3: usize,  // x13
    pub a4: usize,  // x14
    pub a5: usize,  // x15
    pub a6: usize,  // x16
    pub a7: usize,  // x17
    pub s2: usize,  // x18
    pub s3: usize,  // x19
    pub s4: usize,  // x20
    pub s5: usize,  // x21
    pub s6: usize,  // x22
    pub s7: usize,  // x23
    pub s8: usize,  // x24
    pub s9: usize,  // x25
    pub s10: usize, // x26
    pub s11: usize, // x27
    pub t3: usize,  // x28
    pub t4: usize,  // x29
    pub t5: usize,  // x30
    pub t6: usize,  // x31
}

impl TrapContext {
    /// 创建一个空的陷阱上下文
    pub const fn new() -> Self {
        Self {
            ra: 0,
            sp: 0,
            gp: 0,
            tp: 0,
            t0: 0,
            t1: 0,
            t2: 0,
            s0: 0,
            s1: 0,
            a0: 0,
            a1: 0,
            a2: 0,
            a3: 0,
            a4: 0,
            a5: 0,
            a6: 0,
            a7: 0,
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
            t3: 0,
            t4: 0,
            t5: 0,
            t6: 0,
        }
    }
}
