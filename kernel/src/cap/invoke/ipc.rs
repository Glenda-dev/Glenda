use super::super::method::*;
use crate::cap::{CapType, Capability, Rights};
use crate::error::Error;
use crate::ipc;
use crate::proc::TCB;
use crate::proc::scheduler;

pub fn invoke_ipc(cap: &mut Capability, method: usize) -> Result<(), Error> {
    let ep_ptr = if cap.cap_type() == CapType::Endpoint {
        cap.obj_ptr()
    } else {
        error!("IPC::invoke failed: invalid obj type {:?}", cap.cap_type());
        return Err(Error::InvalidType);
    };

    let ep = unsafe { ep_ptr.as_mut::<ipc::Endpoint>() };
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let badge = cap.get_badge();

    match method {
        ipcmethod::SEND => {
            if !cap.has_rights(Rights::SEND) {
                error!("IPC::Send failed: permission denied");
                return Err(Error::PermissionDenied);
            }
            let cap_to_send = ipc::transfer_cap(tcb);
            ipc::send(tcb, ep, badge, cap_to_send)
        }
        ipcmethod::RECV => {
            if !cap.has_rights(Rights::RECV) {
                error!("IPC::Recv failed: permission denied");
                return Err(Error::PermissionDenied);
            }
            ipc::recv(tcb, ep)
        }
        ipcmethod::CALL => {
            if !cap.has_rights(Rights::CALL) {
                error!("IPC::Call failed: permission denied");
                return Err(Error::PermissionDenied);
            }
            let cap_to_send = ipc::transfer_cap(tcb);
            ipc::call(tcb, ep, badge, cap_to_send)
        }
        ipcmethod::NOTIFY => {
            if !cap.has_rights(Rights::SEND) {
                error!("IPC::Notify failed: permission denied");
                return Err(Error::PermissionDenied);
            }
            let utcb = tcb.get_utcb().ok_or(Error::MappingFailed)?;
            let badge = utcb.badge;
            ipc::notify(ep, badge)
        }
        ipcmethod::PROXY => {
            if !cap.has_rights(Rights::CUSTOM) {
                error!("IPC::Proxy failed: permission denied");
                return Err(Error::PermissionDenied);
            }
            let cap_to_send = ipc::transfer_cap(tcb);
            ipc::proxy(tcb, ep, cap_to_send)
        }
        _ => {
            error!("IPC::invoke failed: invalid method {}", method);
            Err(Error::InvalidMethod)
        }
    }
}

pub fn invoke_reply(cap: &mut Capability, method: usize) -> Result<(), Error> {
    let tcb_ptr = if cap.cap_type() == CapType::Reply {
        cap.obj_ptr()
    } else {
        error!("Reply::invoke failed: invalid obj type {:?}", cap.cap_type());
        return Err(Error::InvalidType);
    };

    let target_tcb = unsafe { tcb_ptr.as_mut::<TCB>() };
    let current_tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    match method {
        replymethod::REPLY => {
            let cap_to_send = ipc::transfer_cap(current_tcb);
            ipc::reply(current_tcb, target_tcb, cap_to_send)?;
            // Reply 成功后，该回复能力失效
            *cap = Capability::empty();
            Ok(())
        }
        _ => {
            error!("Reply::invoke failed: invalid method {}", method);
            Err(Error::InvalidMethod)
        }
    }
}
