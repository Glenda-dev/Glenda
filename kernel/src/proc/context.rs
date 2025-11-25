#![allow(dead_code)]
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
    #[cfg(debug_assertions)]
    pub fn print(&self) {
        use crate::printk;
        printk!(
            "ProcContext:\n  ra: 0x{:x}\n  sp: 0x{:x}\n  s0: 0x{:x}\n  s1: 0x{:x}\n  s2: 0x{:x}\n  s3: 0x{:x}\n  s4: 0x{:x}\n  s5: 0x{:x}\n  s6: 0x{:x}\n  s7: 0x{:x}\n  s8: 0x{:x}\n  s9: 0x{:x}\n  s10: 0x{:x}\n  s11: 0x{:x}\n",
            self.ra,
            self.sp,
            self.s0,
            self.s1,
            self.s2,
            self.s3,
            self.s4,
            self.s5,
            self.s6,
            self.s7,
            self.s8,
            self.s9,
            self.s10,
            self.s11,
        );
    }
}
