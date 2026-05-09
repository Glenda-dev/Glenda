#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcContext {
    pub rip: usize,
    pub rsp: usize,
    pub rbp: usize,
    pub rbx: usize,
    pub r12: usize,
    pub r13: usize,
    pub r14: usize,
    pub r15: usize,
}

pub unsafe fn switch_context(_old: *mut ProcContext, _new: *const ProcContext) {
    core::arch::naked_asm!(
        "mov [rdi + 0x00], rax",
        "mov [rdi + 0x08], rsp",
        "mov [rdi + 0x10], rbp",
        "mov [rdi + 0x18], rbx",
        "mov [rdi + 0x20], r12",
        "mov [rdi + 0x28], r13",
        "mov [rdi + 0x30], r14",
        "mov [rdi + 0x38], r15",
        "lea rax, [rip + 2f]",
        "mov [rdi + 0x00], rax",
        "mov rax, [rsi + 0x00]",
        "mov rsp, [rsi + 0x08]",
        "mov rbp, [rsi + 0x10]",
        "mov rbx, [rsi + 0x18]",
        "mov r12, [rsi + 0x20]",
        "mov r13, [rsi + 0x28]",
        "mov r14, [rsi + 0x30]",
        "mov r15, [rsi + 0x38]",
        "jmp rax",
        "2:",
        "ret",
    )
}

impl ProcContext {
    pub const fn new() -> Self {
        Self { rip: 0, rsp: 0, rbp: 0, rbx: 0, r12: 0, r13: 0, r14: 0, r15: 0 }
    }

    pub fn configure(&mut self, entry: usize, stack_top: usize) -> Self {
        self.rip = entry;
        self.rsp = stack_top;
        *self
    }

    pub fn set_fp(&mut self, fp: usize) {
        self.rbp = fp;
    }
}
