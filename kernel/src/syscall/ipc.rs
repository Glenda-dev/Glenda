use crate::ipc;
use crate::irq::TrapContext;

pub fn sys_send(ctx: &mut TrapContext) -> usize {
    let cptr = ctx.a0;
    let msg_info = ctx.a1;
    // Extract from registers a2-a7
    let args: ipc::Args = [ctx.a2, ctx.a3, ctx.a4, ctx.a5, ctx.a6, ctx.a7, 0, 0];
    // TODO: Extract more args from uctb if needed
    // TODO: 确认是 Endpoint
    ipc::send(cptr, msg_info, args);
    0
}

pub fn sys_recv(ctx: &mut TrapContext) -> usize {
    let cptr = ctx.a0;
    // 纯接收
    ipc::recv(cptr);
    0
}

pub fn sys_reply_recv(ctx: &mut TrapContext) -> usize {
    let cptr = ctx.a0;
    let msg_info = ctx.a1;
    let args: ipc::Args = [ctx.a2, ctx.a3, ctx.a4, ctx.a5, ctx.a6, ctx.a7, 0, 0];
    // 服务端常用：回复上一个请求，并等待下一个
    ipc::reply_recv(cptr, msg_info, args);
    0
}
