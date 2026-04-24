use crate::cap::invoke;
use crate::cap::{CapPtr, CapType};
use crate::error::Error;

use crate::proc::scheduler;

pub fn dispatch(cptr: usize, method: usize) -> usize {
    let cptr = CapPtr::from(cptr);
    // 获取当前线程
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let cspace = tcb.get_cspace();
    match unsafe { cspace.lookup_slot_ptr(cptr) } {
        None => {
            let tcb_ptr = tcb as *mut _;
            if let Some(utcb) = tcb.get_utcb() {
                error!(
                    "syscall: Invalid slot: thread={:p}, cptr={:#x}, method={}, msg_tag={:#x}, badge={:#x}, recv_window={}, reply_window={}",
                    tcb_ptr,
                    cptr.bits(),
                    method,
                    utcb.msg_tag.as_usize(),
                    utcb.badge.bits(),
                    utcb.recv_window,
                    utcb.reply_window,
                );
            } else {
                error!(
                    "syscall: Invalid slot: thread={:p}, cptr={:#x}, method={}, utcb=none",
                    tcb_ptr,
                    cptr.bits(),
                    method,
                );
            }
            Error::InvalidSlot as usize
        }
        // 1. 获取 Slot 指针（指向 CSpace 中的真实位置）
        Some(slot_ptr) => {
            let mut cap = {
                let slot = unsafe { &*slot_ptr };
                let _guard = unsafe { slot.lock_cnode() };
                slot.cap.clone()
            };

            if cap.is_null() {
                error!(
                    "syscall: Null capability at {:p}, cptr: {:#x}, method: {}",
                    slot_ptr,
                    cptr.bits(),
                    method
                );
                return Error::InvalidCapability as usize;
            }

            let original_type = cap.cap_type();
            let original_obj = cap.obj_ptr();

            // 3. 执行分发 (invoke_untyped 会修改 cap 的 watermark)
            match invoke::dispatch(&mut cap, method, cptr.bits()) {
                Ok(_) => {
                    if matches!(original_type, CapType::Untyped | CapType::Reply) {
                        let slot = unsafe { &mut *slot_ptr };
                        let _guard = unsafe { slot.lock_cnode() };
                        if slot.cap.cap_type() == original_type
                            && slot.cap.obj_ptr() == original_obj
                        {
                            slot.cap = cap;
                        } else {
                            warn!(
                                "syscall: capability changed during invoke cptr={:#x}, skip writeback",
                                cptr.bits()
                            );
                        }
                    }
                    Error::Success as usize
                }
                Err(e) => e as usize,
            }
        }
    }
}
