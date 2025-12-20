use crate::cap::{invoke, rights};
use crate::proc;
use crate::trap::TrapContext;

/// 系统调用入口：sys_invoke
///
/// ABI:
/// a0: cptr (Capability Pointer)
/// a1: msg_info (Message Tag)
/// a2-a7: args (Method ID + Params)
pub fn sys_invoke(ctx: &mut TrapContext) -> usize {
    let cptr = ctx.a0;
    // let msg_info = ctx.a1;

    // 获取当前线程
    let current_tcb = proc::current();
    // 1. 查找 Capability
    // 注意：这里需要从当前线程的 CSpace 中查找
    let cap = match current_tcb.cap_lookup(cptr) {
        Some(c) => c,
        None => return 1, // Error: Invalid Capability
    };

    // 2. 检查基本调用权限
    if !cap.has_rights(rights::CALL) && !cap.has_rights(rights::SEND) {
        return 2; // Error: Permission Denied
    }

    // 3. 提取参数 (Method ID 通常在 a2)
    // 注意：根据 syscall.md，参数可能在寄存器也可能在 UTCB
    let method = ctx.a2;
    let args = [ctx.a3, ctx.a4, ctx.a5, ctx.a6, ctx.a7];
    // 4. 分发调用
    invoke::dispatch(&cap, method, &args)
}
