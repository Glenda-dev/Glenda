use crate::cap::CapType;
use crate::ipc::{self, Endpoint, MsgTag};
use crate::proc::{self, thread::TCB};
use crate::trap::TrapContext;

pub fn sys_send(ctx: &mut TrapContext) -> usize {
    let cptr = ctx.a0;
    let msg_info = ctx.a1;
    let current = proc::current();

    let ep = match get_ep(current, cptr) {
        Some(e) => e,
        None => return 3, // Error: Not an Endpoint
    };

    let mut cap_to_send = None;
    let tag = MsgTag(msg_info);
    if tag.has_cap() {
        if let Some(utcb) = current.get_utcb() {
            let cap_ptr = utcb.cap_transfer;
            if let Some(cap) = current.cap_lookup(cap_ptr) {
                // 检查是否有 Grant 权限
                if (cap.rights & crate::cap::rights::GRANT) != 0 {
                    cap_to_send = Some(cap);
                }
            }
        }
    }

    ipc::send(current, ep, msg_info, cap_to_send);
    0
}

pub fn sys_recv(ctx: &mut TrapContext) -> usize {
    let cptr = ctx.a0;
    let current = proc::current();

    let ep = match get_ep(current, cptr) {
        Some(e) => e,
        None => return 3, // Error: Not an Endpoint
    };

    ipc::recv(current, ep);
    0
}

fn get_ep(tcb: &TCB, cptr: usize) -> Option<&'static mut Endpoint> {
    if let Some(cap) = tcb.cap_lookup(cptr) {
        if let CapType::Endpoint { ep_ptr } = cap.object {
            return Some(unsafe { &mut *(ep_ptr as *mut Endpoint) });
        }
    }
    None
}
