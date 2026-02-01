mod cnode;
mod ipc;
mod irq;
mod kernel;
mod tcb;
mod untyped;
mod vspace;

use crate::cap::{CapType, Capability};
use crate::trap::syscall::errcode;

// TODO: 分离各个对象类型的 invoke 处理函数
pub fn dispatch(cap: &mut Capability, method: usize) -> usize {
    // 4. 根据对象类型分发
    match cap.cap_type() {
        CapType::Endpoint => ipc::invoke_ipc(cap, method),
        CapType::TCB => tcb::invoke_tcb(cap, method),
        CapType::PageTable => vspace::invoke_pagetable(cap, method),
        CapType::CNode => cnode::invoke_cnode(cap, method),
        CapType::Untyped => untyped::invoke_untyped(cap, method),
        CapType::IrqHandler => irq::invoke_irq_handler(cap, method),
        CapType::VSpace => vspace::invoke_vspace(cap, method),
        CapType::Reply => ipc::invoke_reply(cap, method),
        CapType::Kernel => kernel::invoke_kernel(cap, method),
        _ => errcode::INVALID_OBJ_TYPE,
    }
}
