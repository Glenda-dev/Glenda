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
            error!("syscall: Invalid slot at cptr {:#x}", cptr.bits());
            Error::InvalidSlot as usize
        }
        // 1. 获取 Slot 指针（指向 CSpace 中的真实位置）
        Some(slot_ptr) => {
            // 2. 读取 Capability 副本进行操作
            //    必须使用副本，因为 Rust 不允许同时持有 &mut Slot 和其它引用
            let mut cap = unsafe { (*slot_ptr).cap.clone() };

            if cap.is_null() {
                log!(
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
