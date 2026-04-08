use crate::ipc::MsgArgs;

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
    pub fn configure(&mut self, entry_point: usize, stack_pointer: usize, thread_pointer: usize) {
        self.kernel_epc = entry_point;
        self.sp = stack_pointer;
        self.tp = thread_pointer;
    }
    pub fn configure_kernel(
        &mut self,
        mmu: usize,
        cpuid: usize,
        kstack_top: usize,
        trap_vector: usize,
    ) {
        self.kernel_satp = mmu;
        self.kernel_hartid = cpuid;
        self.kernel_sp = kstack_top;
        self.kernel_trapvector = trap_vector;
    }
    pub fn set_epc(&mut self, epc: usize) {
        self.kernel_epc = epc;
    }
    pub const fn get_epc(&self) -> usize {
        self.kernel_epc
    }
    pub fn set_cpuid(&mut self, id: usize) {
        self.kernel_hartid = id;
    }
    pub fn set_return_value(&mut self, value: usize) {
        self.a0 = value;
    }
    pub const fn get_syscall_args(&self) -> (usize, usize) {
        (self.a0, self.a7)
    }
    pub fn advance_pc(&mut self) {
        self.kernel_epc += 4;
    }
    pub const fn get_ra(&self) -> usize {
        self.ra
    }
    pub const fn get_sp(&self) -> usize {
        self.sp
    }
    pub fn get_registers(&self) -> MsgArgs {
        [self.a0, self.a1, self.a2, self.a3, self.a4, self.a5, self.a6, self.a7]
    }
    pub fn get_syscall_registers(&self) -> MsgArgs {
        [self.a7, self.a0, self.a1, self.a2, self.a3, self.a4, self.a5, self.a6]
    }
    pub fn set_registers(&mut self, regs: &MsgArgs) {
        self.a0 = regs[0];
        self.a1 = regs[1];
        self.a2 = regs[2];
        self.a3 = regs[3];
        self.a4 = regs[4];
        self.a5 = regs[5];
        self.a6 = regs[6];
        self.a7 = regs[7];
    }
    pub fn for_each_register<F>(&self, mut f: F)
    where
        F: FnMut(usize),
    {
        f(0); // x0
        f(self.ra);
        f(self.sp);
        f(self.gp);
        f(self.tp);
        f(self.t0);
        f(self.t1);
        f(self.t2);
        f(self.s0);
        f(self.s1);
        f(self.a0);
        f(self.a1);
        f(self.a2);
        f(self.a3);
        f(self.a4);
        f(self.a5);
        f(self.a6);
        f(self.a7);
        f(self.s2);
        f(self.s3);
        f(self.s4);
        f(self.s5);
        f(self.s6);
        f(self.s7);
        f(self.s8);
        f(self.s9);
        f(self.s10);
        f(self.s11);
        f(self.t3);
        f(self.t4);
        f(self.t5);
        f(self.t6);
        f(self.kernel_epc);
    }
}
