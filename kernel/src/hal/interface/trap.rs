use crate::trap::TrapCause;

/// 上下文结构体
/// 保存寄存器等上下文信息
pub struct TrapContext;

/// 初始化异常向量表
///
/// 将 CPU 的异常入口基地址 (stvec/vbar_el1) 指向内核的 trap handler
pub unsafe fn vector_init() {
    unimplemented!()
}

/// 获取导致 Trap 的原因
/// 返回架构无关的枚举 (Syscall, Timer, ExternalIrq, PageFault...)
pub fn get_cause(ctx: &TrapContext) -> TrapCause {
    unimplemented!()
}
/// 获取 Trap 发生时的程序计数器 (PC/EPC)
pub fn get_pc(ctx: &TrapContext) -> usize {
    unimplemented!()
}

/// 修改上下文中的 PC (通常用于 syscall 返回后跳过 ecall 指令)
pub fn advance_pc(ctx: &mut TrapContext, bytes: usize) {
    unimplemented!()
}
