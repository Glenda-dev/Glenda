use crate::cap::capability;
use crate::cap::{CapType, Capability, rights};
use crate::ipc;
use crate::ipc::endpoint::Endpoint;
use crate::irq::{TrapContext, TrapFrame};
use crate::mem::{self, PageTable, PhysAddr, VirtAddr, vm};
use crate::proc::{self, TCB, ThreadState, scheduler};
use alloc::vec::Vec;

/// 系统调用入口：sys_invoke
///
/// ABI:
/// a0: cptr (Capability Pointer)
/// a1: msg_info (Message Tag)
/// a2-a7: args (Method ID + Params)
pub fn sys_invoke(ctx: &mut TrapContext) -> usize {
    let cptr = ctx.a0;
    let msg_info = ctx.a1;

    // 获取当前线程
    let current_tcb = proc::current();
    // 1. 查找 Capability
    // 注意：这里需要从当前线程的 CSpace 中查找
    // 假设 TCB 中有 helper 方法 get_cnode() 或直接访问 cspace_root
    // 这里简化为假设 current_tcb 有个方法 lookup_cap
    let cap = match current_tcb.cap_lookup(cptr) {
        Some(c) => c,
        None => return 1, // Error: Invalid Capability
    };

    // 2. 检查基本调用权限
    if !cap.has_rights(rights::CALL) && !cap.has_rights(rights::SEND) {
        return 2; // Error: Permission Denied
    }

    // 3. 提取参数 (Method ID 通常在 a2)
    // 注意：根据 syscall.md，参数可能在寄存器也可能在 UTCB
    let method = ctx.a2;
    let args = [ctx.a3, ctx.a4, ctx.a5, ctx.a6, ctx.a7];

    // 4. 根据对象类型分发
    match cap.object {
        CapType::Endpoint { ep_ptr } => {
            // IPC 调用：Send / Call
            // 注意：sys_invoke 对 Endpoint 的语义通常等同于 sys_send 或 sys_call
            // 这里我们将其映射为 sys_send (带 Badge)
            let ep = unsafe { &mut *(ep_ptr as *mut Endpoint) };
            ipc::send(current_tcb, ep, cap.badge.unwrap_or(0));
            0 // Send 成功 (如果阻塞，调度器会处理)
        }
        CapType::Thread { tcb_ptr } => {
            let target_tcb = unsafe { &mut *(tcb_ptr as *mut TCB) };
            invoke_tcb(target_tcb, method, &args)
        }
        CapType::PageTable { paddr, .. } => {
            // PageTable 需要物理地址转虚拟地址才能操作
            let pt_ptr = vm::phys_to_virt(paddr);
            let pt = unsafe { &mut *(pt_ptr as *mut PageTable) };
            invoke_pagetable(pt, method, &args)
        }
        CapType::CNode { paddr, bits, .. } => {
            // CNode 操作 (Mint, Copy, etc.) 比较复杂，通常涉及两个 CNode
            // 这里仅作为占位符
            invoke_cnode(paddr, bits, method, &args)
        }
        CapType::Untyped { start_paddr, size } => invoke_untyped(start_paddr, size, method, &args),
        _ => 3, // Error: Invalid Object Type for Invocation
    }
}

// --- TCB Methods ---

fn invoke_tcb(tcb: &mut TCB, method: usize, args: &[usize]) -> usize {
    match method {
        // Configure: (cspace, vspace, utcb, fault_ep)
        // args: [cspace_cptr, vspace_cptr, utcb_addr, fault_ep_cptr]
        1 => {
            // 注意：这里传递的是 CPTR，内核需要再次查找这些 Cap 对应的内核对象
            // 这是一个简化的实现，实际需要验证这些 Cap 的有效性
            // tcb.configure(...)
            unimplemented!();
        }
        // SetPriority: (prio)
        2 => {
            let prio = args[0] as u8;
            tcb.set_priority(prio);
            // 如果修改了优先级，可能需要触发重新调度
            0
        }
        // SetRegisters: (flags, arch_flags, ...)
        // 参数通常从 UTCB 读取，因为寄存器太多放不下
        3 => {
            // 读取 UTCB 中的寄存器状态并写入 tcb.context
            unimplemented!();
        }
        // Resume
        5 => {
            tcb.resume();
            // 将线程加入调度队列
            proc::scheduler::add_thread(tcb);
            0
        }
        // Suspend
        6 => {
            tcb.suspend();
            // 如果目标是当前线程，需要触发 yield
            scheduler::yield_proc();
            0
        }
        _ => 4, // Error: Invalid Method
    }
}

// --- PageTable Methods ---

fn invoke_pagetable(pt: &mut PageTable, method: usize, args: &[usize]) -> usize {
    match method {
        // Map: (frame_cap, vaddr, flags)
        1 => {
            // args[0] 是 Frame Cap 的 CPTR，需要查找获取物理地址
            // 假设 args[0] 已经是 paddr (简化)
            let paddr = PhysAddr::from(args[0]);
            let vaddr = VirtAddr::from(args[1]);
            let flags = args[2];

            // 转换 flags 为 PTE flags
            // ...

            // 执行映射
            // pt.map(vaddr, paddr, flags)
            unimplemented!();
        }
        // Unmap: (vaddr)
        2 => {
            let vaddr = VirtAddr::from(args[0]);
            // pt.unmap(vaddr)
            unimplemented!();
        }
        _ => 4,
    }
}

// --- Other Methods (Stubs) ---

fn invoke_cnode(paddr: PhysAddr, bits: u8, method: usize, args: &[usize]) -> usize {
    // CNode 操作：Copy, Mint, Move, Revoke, Delete
    unimplemented!();
}

fn invoke_untyped(start: PhysAddr, size: usize, method: usize, args: &[usize]) -> usize {
    // Untyped 操作：Retype, etc.
    unimplemented!();
}
