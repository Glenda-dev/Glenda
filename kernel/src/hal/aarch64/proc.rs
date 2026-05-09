use core::arch::naked_asm;

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn switch_context(old: *mut ProcContext, new: *const ProcContext) {
    naked_asm!(
        "stp x19, x20, [x0, #0]",
        "stp x21, x22, [x0, #16]",
        "stp x23, x24, [x0, #32]",
        "stp x25, x26, [x0, #48]",
        "stp x27, x28, [x0, #64]",
        "stp x29, x30, [x0, #80]",
        "mov x2, sp",
        "str x2, [x0, #96]",
        "ldr x2, [x1, #96]",
        "mov sp, x2",
        "ldp x19, x20, [x1, #0]",
        "ldp x21, x22, [x1, #16]",
        "ldp x23, x24, [x1, #32]",
        "ldp x25, x26, [x1, #48]",
        "ldp x27, x28, [x1, #64]",
        "ldp x29, x30, [x1, #80]",
        "ret",
    );
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ProcContext {
    pub x19: usize,
    pub x20: usize,
    pub x21: usize,
    pub x22: usize,
    pub x23: usize,
    pub x24: usize,
    pub x25: usize,
    pub x26: usize,
    pub x27: usize,
    pub x28: usize,
    pub fp: usize,
    pub lr: usize,
    pub sp: usize,
}

impl ProcContext {
    pub const fn new() -> Self {
        Self {
            x19: 0,
            x20: 0,
            x21: 0,
            x22: 0,
            x23: 0,
            x24: 0,
            x25: 0,
            x26: 0,
            x27: 0,
            x28: 0,
            fp: 0,
            lr: 0,
            sp: 0,
        }
    }

    pub fn configure(&mut self, entry_point: usize, stack_pointer: usize) {
        self.lr = entry_point;
        self.sp = stack_pointer;
    }

    pub fn set_fp(&mut self, fp: usize) {
        self.fp = fp;
    }
}
