use crate::ipc;
use crate::ipc::Endpoint;
use crate::proc;
use crate::trap::TrapContext;

pub fn sys_send(ctx: &mut TrapContext) -> usize {
    let cptr = ctx.a0;
    let msg_info = ctx.a1;
    let proc = proc::current();
    // Extract from registers a2-a7
    let ep = match get_ep(cptr) {
        Some(e) => e,
        None => return 3, // Error: Not an Endpoint
    };
    // TODO: Extract more args from uctb if needed
    ipc::send(proc, ep, msg_info);
    0
}

pub fn sys_recv(ctx: &mut TrapContext) -> usize {
    let cptr = ctx.a0;
    let ep = match get_ep(cptr) {
        Some(e) => e,
        None => return 3, // Error: Not an Endpoint
    };
    let proc = proc::current();
    // 纯接收
    ipc::recv(proc, ep);
    0
}

// TODO: 实现 Capability 查找逻辑
fn get_ep(cptr: usize) -> Option<&'static mut Endpoint> {
    unimplemented!()
}
