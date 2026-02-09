use super::super::method::*;
use crate::cap::{CapType, Capability, Rights};
use crate::ipc;
use crate::proc::TCB;
use crate::proc::scheduler;
use crate::trap::syscall::errcode;

pub fn invoke_ipc(cap: &mut Capability, method: usize) -> usize {
    let ep_ptr = if cap.cap_type() == CapType::Endpoint {
        cap.obj_ptr()
    } else {
        log!("IPC::invoke failed: invalid obj type {:?}", cap.cap_type());
        return errcode::INVALID_OBJ_TYPE;
    };

    let ep = unsafe { ep_ptr.as_mut::<ipc::Endpoint>() };
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let badge = cap.get_badge();

    // 获取 UTCB 以读取参数 (msg_info)
    if tcb.get_utcb().is_none() {
        log!("IPC::invoke failed: no UTCB");
        return errcode::MAPPING_FAILED;
    }

    match method {
        ipcmethod::SEND => {
            if !cap.has_rights(Rights::SEND) {
                log!("IPC::Send failed: permission denied");
                return errcode::PERMISSION_DENIED;
            }
            let cap_to_send = ipc::transfer_cap(tcb);
            ipc::send(tcb, ep, badge, cap_to_send);
            errcode::SUCCESS
        }
        ipcmethod::RECV => {
            if !cap.has_rights(Rights::RECV) {
                log!("IPC::Recv failed: permission denied");
                return errcode::PERMISSION_DENIED;
            }
            ipc::recv(tcb, ep);
            errcode::SUCCESS
        }
        ipcmethod::CALL => {
            if !cap.has_rights(Rights::CALL) {
                log!("IPC::Call failed: permission denied");
                return errcode::PERMISSION_DENIED;
            }
            let cap_to_send = ipc::transfer_cap(tcb);
            ipc::call(tcb, ep, badge, cap_to_send);
            errcode::SUCCESS
        }
        ipcmethod::NOTIFY => {
            if !cap.has_rights(Rights::SEND) {
                log!("IPC::Notify failed: permission denied");
                return errcode::PERMISSION_DENIED;
            }
            ipc::notify(ep, badge);
            errcode::SUCCESS
        }
        _ => {
            log!("IPC::invoke failed: invalid method {}", method);
            errcode::INVALID_METHOD
        }
    }
}

pub fn invoke_reply(cap: &mut Capability, method: usize) -> usize {
    let tcb_ptr = if cap.cap_type() == CapType::Reply {
        cap.obj_ptr()
    } else {
        log!("Reply::invoke failed: invalid obj type {:?}", cap.cap_type());
        return errcode::INVALID_OBJ_TYPE;
    };

    let target_tcb = unsafe { tcb_ptr.as_mut::<TCB>() };
    let current_tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    match method {
        replymethod::REPLY => {
            let cap_to_send = ipc::transfer_cap(current_tcb);
            ipc::reply(current_tcb, target_tcb, cap_to_send);
            errcode::SUCCESS
        }
        _ => {
            log!("Reply::invoke failed: invalid method {}", method);
            errcode::INVALID_METHOD
        }
    }
}
