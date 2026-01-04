use crate::cap::CapPtr;
use crate::cap::{invoke, rights};
use crate::proc::scheduler;
use crate::trap::TrapContext;

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

pub fn dispatch(ctx: &mut TrapContext) -> usize {
    let cptr = ctx.a0;

    // 获取当前线程
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    // 1. 查找 Capability
    // 注意：这里需要从当前线程的 CSpace 中查找
    let cap = match tcb.cap_lookup(cptr) {
        Some(c) => c,
        None => return errcode::INVALID_CAP, // Error: Invalid Capability
    };

    // 2. 检查基本调用权限
    if !cap.has_rights(rights::CALL) && !cap.has_rights(rights::SEND) {
        return errcode::PERMISSION_DENIED; // Error: Permission Denied
    }

    // 3. 提取参数 (Method ID 通常在 a7)
    let method = ctx.a7;
    // 4. 分发调用
    invoke::dispatch(&cap, CapPtr::from(cptr), method)
}
