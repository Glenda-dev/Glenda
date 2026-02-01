use super::super::method::*;
use crate::cap::{Capability, Rights};
use crate::hal;
use crate::proc::scheduler;
use crate::trap::syscall::errcode;

pub fn invoke_kernel(cap: &mut Capability, method: usize) -> usize {
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => return errcode::MAPPING_FAILED,
    };

    match method {
        kernelmethod::CONSOLE_PUT_STR => {
            if cap.has_rights(Rights::WRITE) == false {
                return errcode::PERMISSION_DENIED;
            }
            let offset = utcb.mrs_regs[0];
            let len = utcb.mrs_regs[1];
            if let Some(_) = utcb.with_str(offset, len, |s| {
                crate::printk!("{}", s);
            }) {
                errcode::SUCCESS
            } else {
                errcode::INVALID_SLOT
            }
        }
        kernelmethod::CONSOLE_GET_CHAR => {
            if cap.has_rights(Rights::READ) == false {
                return errcode::PERMISSION_DENIED;
            }
            let c = hal::console::read() as usize;
            utcb.mrs_regs[0] = c;
            errcode::SUCCESS
        }
        kernelmethod::SHELL => {
            if cap.has_rights(Rights::EXECUTE) == false {
                return errcode::PERMISSION_DENIED;
            }
            crate::shell::run();
            errcode::SUCCESS
        }
        _ => errcode::INVALID_METHOD,
    }
}
