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

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TrapFrame {
    pub kernel_satp: usize,       // 内核页表地址
    pub kernel_sp: usize,         // 内核栈指针
    pub kernel_trapvector: usize, // 内核陷阱向量地址
    pub kernel_epc: usize,        // 用户态程序计数器
    pub kernel_hartid: usize,     // 处理器核ID

    // 通用寄存器
    pub ra: usize,
    pub sp: usize,
    pub gp: usize,
    pub tp: usize,
    pub t0: usize,
    pub t1: usize,
    pub t2: usize,
    pub s0: usize,
    pub s1: usize,
    pub a0: usize,
    pub a1: usize,
    pub a2: usize,
    pub a3: usize,
    pub a4: usize,
    pub a5: usize,
    pub a6: usize,
    pub a7: usize,
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
    pub t3: usize,
    pub t4: usize,
    pub t5: usize,
    pub t6: usize,
}

impl TrapFrame {
    pub const fn new() -> Self {
        Self {
            kernel_satp: 0,
            kernel_sp: 0,
            kernel_trapvector: 0,
            kernel_epc: 0,
            kernel_hartid: 0,
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
    #[cfg(feature = "tests")]
    pub fn print(&self) {
        use crate::printk;
        printk!(
            "TrapFrame:\n  kernel_satp: 0x{:x}\n  kernel_sp: 0x{:x}\n  kernel_trapvector: 0x{:x}\n  kernel_epc: 0x{:x}\n  kernel_hartid: {}\n  ra: 0x{:x}\n  sp: 0x{:x}\n  gp: 0x{:x}\n  tp: 0x{:x}\n",
            self.kernel_satp,
            self.kernel_sp,
            self.kernel_trapvector,
            self.kernel_epc,
            self.kernel_hartid,
            self.ra,
            self.sp,
            self.gp,
            self.tp,
        );
    }
}
