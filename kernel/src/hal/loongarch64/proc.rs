#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcContext {
    pub ra: usize,
    pub sp: usize,
    pub s: [usize; 9], // s0-s8
}

impl ProcContext {
    pub const fn new() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 9],
        }
    }

    pub fn configure(&mut self, entry: usize, sp: usize) {
        self.ra = entry;
        self.sp = sp;
    }

    pub fn set_fp(&mut self, fp: usize) {
        self.s[0] = fp;
    }
}

pub fn switch_context(_old: *mut ProcContext, _new: *const ProcContext) {
}
