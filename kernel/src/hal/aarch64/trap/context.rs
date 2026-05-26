use crate::ipc::MsgArgs;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TrapFrame {
    pub regs: [usize; 31],    // 0 - 240
    pub sp: usize,            // 248
    pub elr: usize,           // 256
    pub spsr: usize,          // 264
    pub tpidr: usize,         // 272
    pub kernel_sp: usize,     // 280
    pub kernel_vector: usize, // 288
    pub mmu: usize,           // 296
    pub cpuid: usize,         // 304
}

impl TrapFrame {
    pub const fn new() -> Self {
        Self {
            regs: [0; 31],
            sp: 0,
            elr: 0,
            spsr: 0,
            tpidr: 0,
            kernel_sp: 0,
            kernel_vector: 0,
            mmu: 0,
            cpuid: 0,
        }
    }

    pub const fn get_epc(&self) -> usize {
        self.elr
    }

    pub fn set_epc(&mut self, epc: usize) {
        self.elr = epc;
    }

    pub fn set_cpuid(&mut self, id: usize) {
        self.cpuid = id;
    }

    pub fn configure(&mut self, entry_point: usize, stack_pointer: usize, thread_pointer: usize) {
        self.elr = entry_point;
        self.sp = stack_pointer;
        self.tpidr = thread_pointer;
        // SPSR_EL1: EL0t mode, interrupts enabled (all 0s)
        self.spsr = 0;
    }

    pub fn configure_kernel(
        &mut self,
        mmu: usize,
        hartid: usize,
        kstack_top: usize,
        kernel_vec: usize,
    ) {
        self.mmu = mmu;
        self.cpuid = hartid;
        self.kernel_sp = kstack_top;
        self.kernel_vector = kernel_vec;
    }

    pub fn set_return_value(&mut self, value: usize) {
        self.regs[0] = value;
    }

    pub fn set_fast_ipc_hint(&mut self, enabled: bool) {
        // Reserve x18 as a software hint register in trap context.
        self.regs[18] = enabled as usize;
    }

    pub const fn get_syscall_args(&self) -> (isize, usize) {
        (self.regs[8] as isize, self.regs[0])
    }

    pub const fn get_syscall_ipc_args(&self) -> (usize, usize, [usize; 4]) {
        (self.regs[1], self.regs[2], [self.regs[3], self.regs[4], self.regs[5], self.regs[6]])
    }

    pub fn set_syscall_ipc_ret(&mut self, msgtag: usize, badge: usize, mrs: [usize; 4]) {
        self.regs[1] = msgtag;
        self.regs[2] = badge;
        self.regs[3] = mrs[0];
        self.regs[4] = mrs[1];
        self.regs[5] = mrs[2];
        self.regs[6] = mrs[3];
    }

    pub fn advance_pc(&mut self) {
        self.elr += 4;
    }

    pub const fn get_ra(&self) -> usize {
        self.regs[30]
    }

    pub fn set_ra(&mut self, ra: usize) {
        self.regs[30] = ra;
    }

    pub const fn get_sp(&self) -> usize {
        self.sp
    }

    pub fn get_registers(&self) -> MsgArgs {
        [
            self.regs[3],
            self.regs[4],
            self.regs[5],
            self.regs[6],
            self.regs[7],
            self.regs[9],
            self.regs[10],
            self.regs[11],
        ]
    }

    pub fn get_syscall_registers(&self) -> MsgArgs {
        [
            self.regs[0],
            self.regs[1],
            self.regs[2],
            self.regs[3],
            self.regs[4],
            self.regs[5],
            self.regs[6],
            self.regs[8],
        ]
    }

    pub fn set_registers(&mut self, regs: &MsgArgs) {
        self.regs[0] = regs[0];
        self.regs[1] = regs[1];
        self.regs[2] = regs[2];
        self.regs[3] = regs[3];
        self.regs[4] = regs[4];
        self.regs[5] = regs[5];
        self.regs[6] = regs[6];
        self.regs[7] = regs[7];
    }

    pub fn for_each_register<F>(&self, mut f: F)
    where
        F: FnMut(usize),
    {
        for i in 0..31 {
            f(self.regs[i]);
        }
        f(self.sp);
        f(self.elr);
    }
}
