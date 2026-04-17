use crate::cap::CapPtr;
use crate::cap::invoke;
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
            // 2. 读取 Capability 副本进行操作
            //    必须使用副本，因为 Rust 不允许同时持有 &mut Slot 和其它引用
            let mut cap = unsafe { (*slot_ptr).cap.clone() };

            if cap.is_null() {
                error!(
                    "syscall: Null capability at {:p}, cptr: {:#x}, method: {}",
                    slot_ptr,
                    cptr.bits(),
                    method
                );
                return Error::InvalidCapability as usize;
            }

            // 3. 执行分发 (invoke_untyped 会修改 cap 的 watermark)
            match invoke::dispatch(&mut cap, method, cptr.bits()) {
                Ok(_) => {
                    // 4. 【关键】写回逻辑
                    //    仅当操作成功，且 Capability 类型为 Untyped 时需要写回
                    unsafe {
                        (*slot_ptr).cap = cap;
                    }
                    Error::Success as usize
                }
                Err(e) => e as usize,
            }
        }
    }
}
