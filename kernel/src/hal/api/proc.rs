pub struct ProcContext;
/// 切换进程上下文
pub unsafe fn switch_context(old: *mut ProcContext, new: *const ProcContext) {
    unimplemented!()
}

impl ProcContext {
    /// 创建一个空的上下文
    pub const fn new() -> Self {
        unimplemented!()
    }
    /// 创建新的进程上下文
    ///
    /// * `entry`: 进程入口地址
    /// * `stack_top`: 进程栈顶地址
    pub fn configure(&mut self, entry: usize, stack_top: usize) -> Self {
        unimplemented!()
    }
    /// 设置帧指针 (s0)
    pub fn set_fp(&mut self, fp: usize) {
        unimplemented!()
    }
}
