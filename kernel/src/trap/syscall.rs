use crate::cap::CapPtr;
use crate::cap::invoke;

use crate::proc::scheduler;

pub mod errcode {
    pub const SUCCESS: usize = 0;
    pub const INVALID_CAP: usize = 1;
    pub const PERMISSION_DENIED: usize = 2;
    pub const INVALID_ENDPOINT: usize = 3;
    pub const INVALID_OBJ_TYPE: usize = 4;
    pub const INVALID_METHOD: usize = 5;
    pub const MAPPING_FAILED: usize = 6;
    pub const INVALID_SLOT: usize = 7;
    pub const UNTYPE_OOM: usize = 8;
}

pub fn dispatch(cptr: usize, method: usize) -> usize {
    let cptr = CapPtr::from(cptr);
    // 获取当前线程
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let cspace = tcb.get_cspace();
    match cspace.lookup_slot_ptr(cptr) {
        None => {
            log!("syscall: Invalid slot at cptr {:#x}", cptr.bits());
            errcode::INVALID_SLOT
        }
        // 1. 获取 Slot 指针（指向 CSpace 中的真实位置）
        Some(slot_ptr) => {
            // 2. 读取 Capability 副本进行操作
            //    必须使用副本，因为 Rust 不允许同时持有 &mut Slot 和其它引用
            let mut cap = unsafe { (*slot_ptr).cap.clone() };

            if cap.is_null() {
                log!("syscall: Invalid capability at cptr {:#x}, method: {}", cptr.bits(), method);
                return errcode::INVALID_CAP;
            }

            // 3. 执行分发 (invoke_untyped 会修改 cap 的 watermark)
            let result = invoke::dispatch(&mut cap, method);

            // 4. 【关键】写回逻辑
            //    仅当操作成功，且 Capability 类型为 Untyped 时需要写回
            if result == errcode::SUCCESS {
                unsafe {
                    (*slot_ptr).cap = cap;
                }
            }

            result
        }
    }
}
