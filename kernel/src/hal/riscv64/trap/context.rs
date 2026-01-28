/// 陷阱处理时的寄存器上下文结构
/// 对应汇编代码中栈上的布局
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TrapContext {
    // 通用寄存器（x0-x31）
    // 注意：x0(zero)不能修改，所以这里占位但不使用
    ra: usize,  // x1
    sp: usize,  // x2
    gp: usize,  // x3
    tp: usize,  // x4
    t0: usize,  // x5
    t1: usize,  // x6
    t2: usize,  // x7
    s0: usize,  // x8
    s1: usize,  // x9
    a0: usize,  // x10
    a1: usize,  // x11
    a2: usize,  // x12
    a3: usize,  // x13
    a4: usize,  // x14
    a5: usize,  // x15
    a6: usize,  // x16
    a7: usize,  // x17
    s2: usize,  // x18
    s3: usize,  // x19
    s4: usize,  // x20
    s5: usize,  // x21
    s6: usize,  // x22
    s7: usize,  // x23
    s8: usize,  // x24
    s9: usize,  // x25
    s10: usize, // x26
    s11: usize, // x27
    t3: usize,  // x28
    t4: usize,  // x29
    t5: usize,  // x30
    t6: usize,  // x31
}

impl TrapContext {
    pub fn set_return_value(&mut self, value: usize) {
        self.a0 = value;
    }
    pub fn get_syscall_args(&self) -> (usize, usize) {
        (self.a0, self.a7)
    }
    pub fn from_trapframe(ctx: &TrapFrame) -> Self {
        Self {
            ra: ctx.ra,
            sp: ctx.sp,
            gp: ctx.gp,
            tp: ctx.tp,
            t0: ctx.t0,
            t1: ctx.t1,
            t2: ctx.t2,
            s0: ctx.s0,
            s1: ctx.s1,
            a0: ctx.a0,
            a1: ctx.a1,
            a2: ctx.a2,
            a3: ctx.a3,
            a4: ctx.a4,
            a5: ctx.a5,
            a6: ctx.a6,
            a7: ctx.a7,
            s2: ctx.s2,
            s3: ctx.s3,
            s4: ctx.s4,
            s5: ctx.s5,
            s6: ctx.s6,
            s7: ctx.s7,
            s8: ctx.s8,
            s9: ctx.s9,
            s10: ctx.s10,
            s11: ctx.s11,
            t3: ctx.t3,
            t4: ctx.t4,
            t5: ctx.t5,
            t6: ctx.t6,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TrapFrame {
    kernel_satp: usize,       // 内核页表地址
    kernel_sp: usize,         // 内核栈指针
    kernel_trapvector: usize, // 内核陷阱向量地址
    kernel_epc: usize,        // 用户态程序计数器
    kernel_hartid: usize,     // 处理器核ID
    // 通用寄存器
    ra: usize,
    sp: usize,
    gp: usize,
    tp: usize,
    t0: usize,
    t1: usize,
    t2: usize,
    s0: usize,
    s1: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
    a7: usize,
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
    t3: usize,
    t4: usize,
    t5: usize,
    t6: usize,
}

impl TrapFrame {
    pub fn set_badge(&mut self, badge: usize) {
        self.a1 = badge;
    }
    pub fn configure(&mut self, entry_point: usize, stack_pointer: usize) {
        self.kernel_epc = entry_point;
        self.sp = stack_pointer;
    }
    pub fn configure_kernel(
        &mut self,
        satp: usize,
        cpuid: usize,
        kstack_top: usize,
        trap_vector: usize,
    ) {
        self.kernel_satp = satp;
        self.kernel_hartid = cpuid;
        self.kernel_sp = kstack_top;
        self.kernel_trapvector = trap_vector;
    }
    pub fn set_epc(&mut self, epc: usize) {
        self.kernel_epc = epc;
    }
    pub fn get_epc(&self) -> usize {
        self.kernel_epc
    }
    pub fn update_context(&mut self, ctx: &TrapContext) {
        self.ra = ctx.ra;
        self.sp = ctx.sp;
        self.gp = ctx.gp;
        self.tp = ctx.tp;
        self.t0 = ctx.t0;
        self.t1 = ctx.t1;
        self.t2 = ctx.t2;
        self.s0 = ctx.s0;
        self.s1 = ctx.s1;
        self.a0 = ctx.a0;
        self.a1 = ctx.a1;
        self.a2 = ctx.a2;
        self.a3 = ctx.a3;
        self.a4 = ctx.a4;
        self.a5 = ctx.a5;
        self.a6 = ctx.a6;
        self.a7 = ctx.a7;
        self.s2 = ctx.s2;
        self.s3 = ctx.s3;
        self.s4 = ctx.s4;
        self.s5 = ctx.s5;
        self.s6 = ctx.s6;
        self.s7 = ctx.s7;
        self.s8 = ctx.s8;
        self.s9 = ctx.s9;
        self.s10 = ctx.s10;
        self.s11 = ctx.s11;
        self.t3 = ctx.t3;
        self.t4 = ctx.t4;
        self.t5 = ctx.t5;
        self.t6 = ctx.t6;
    }
    pub fn set_tf(&mut self, tf_addr: usize) {
        self.t6 = tf_addr;
    }
}
