use super::super::method::*;
use crate::cap::{CapType, Capability, Rights};
use crate::ipc;
use crate::ipc::MsgFlags;
use crate::proc::TCB;
use crate::proc::scheduler;
use crate::trap::syscall::errcode;

pub fn invoke_ipc(cap: &mut Capability, method: usize) -> usize {
    let ep_ptr = if cap.cap_type() == CapType::Endpoint {
        cap.obj_ptr()
    } else {
        return errcode::INVALID_OBJ_TYPE;
    };

    let ep = ep_ptr.as_mut::<ipc::Endpoint>();
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let badge = cap.get_badge();

    // 获取 UTCB 以读取参数 (msg_info)
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => return errcode::MAPPING_FAILED,
    };

    match method {
        ipcmethod::SEND => {
            if !cap.has_rights(Rights::SEND) {
                return errcode::PERMISSION_DENIED;
            }
            let tag = utcb.msg_tag;
            // 通过 invoke 发送时，暂时不支持传递能力，或者从 UTCB 中提取
            let mut cap_to_send = None;
            if tag.flags().contains(MsgFlags::HAS_CAP) {
                if let Some(cap) = tcb.cap_lookup(utcb.cap_transfer) {
                    if cap.has_rights(Rights::GRANT) {
                        cap_to_send = Some(cap);
                    }
                }
            }
            ipc::send(tcb, ep, badge, cap_to_send);
            errcode::SUCCESS
        }
        ipcmethod::RECV => {
            if !cap.has_rights(Rights::RECV) {
                return errcode::PERMISSION_DENIED;
            }
            ipc::recv(tcb, ep);
            errcode::SUCCESS
        }
        ipcmethod::CALL => {
            if !cap.has_rights(Rights::CALL) {
                return errcode::PERMISSION_DENIED;
            }
            let tag = utcb.msg_tag;
            let mut cap_to_send = None;
            if tag.flags().contains(MsgFlags::HAS_CAP) {
                if let Some(cap) = tcb.cap_lookup(utcb.cap_transfer) {
                    if cap.has_rights(Rights::GRANT) {
                        cap_to_send = Some(cap);
                    }
                }
            }
            ipc::call(tcb, ep, badge, cap_to_send);
            errcode::SUCCESS
        }
        ipcmethod::NOTIFY => {
            if !cap.has_rights(Rights::SEND) {
                return errcode::PERMISSION_DENIED;
            }
            ipc::notify(ep, badge);
            errcode::SUCCESS
        }
        _ => errcode::INVALID_METHOD,
    }
}

pub fn invoke_reply(cap: &mut Capability, method: usize) -> usize {
    let tcb_ptr = if cap.cap_type() == CapType::Reply {
        cap.obj_ptr()
    } else {
        return errcode::INVALID_OBJ_TYPE;
    };

    let target_tcb = tcb_ptr.as_mut::<TCB>();
    let current_tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    match method {
        replymethod::REPLY => {
            ipc::reply(current_tcb, target_tcb);
            errcode::SUCCESS
        }
        _ => errcode::INVALID_METHOD,
    }
}
