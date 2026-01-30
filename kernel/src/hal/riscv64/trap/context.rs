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
}
