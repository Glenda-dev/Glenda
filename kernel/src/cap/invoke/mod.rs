mod cnode;
mod ipc;
mod irq;
mod kernel;
mod tcb;
mod untyped;
mod vspace;

use crate::cap::{CapType, Capability};
use crate::log;
use crate::trap::syscall::errcode;

pub fn dispatch(cap: &mut Capability, method: usize, cptr: usize) -> usize {
    // 4. 根据对象类型分发
    match cap.cap_type() {
        CapType::Endpoint => ipc::invoke_ipc(cap, method),
        CapType::TCB => tcb::invoke_tcb(cap, method),
        CapType::PageTable => vspace::invoke_pagetable(cap, method),
        CapType::CNode => cnode::invoke_cnode(cap, method),
        CapType::Untyped => untyped::invoke_untyped(cap, method, cptr),
        CapType::IrqHandler => irq::invoke_irq_handler(cap, method),
        CapType::VSpace => vspace::invoke_vspace(cap, method),
        CapType::Reply => ipc::invoke_reply(cap, method),
        CapType::Kernel => kernel::invoke_kernel(cap, method),
        _ => {
            log!("Invoke: Invalid capability: {:?}\n", cap);
            errcode::INVALID_OBJ_TYPE
        }
    }
}
