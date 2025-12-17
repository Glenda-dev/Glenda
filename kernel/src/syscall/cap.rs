use crate::cap::CapType;
use crate::ipc;
use crate::irq::TrapContext;
use crate::mem::{PageTable, PhysAddr};
use crate::proc;
use crate::proc::process::ProcState;
use crate::proc::table::{NPROC, PROC_TABLE};
use alloc::vec::Vec;

pub fn sys_invoke(ctx: &mut TrapContext) -> usize {
    let cptr = ctx.a0;
    let msg_info = ctx.a1;
    // Extract from registers a2-a7
    let args: Vec<usize> = Vec::from([ctx.a2, ctx.a3, ctx.a4, ctx.a5, ctx.a6, ctx.a7]);
    // TODO: Extract more args from uctb if needed
    handle_invocation(cptr, msg_info, &args)
}

fn handle_invocation(cptr: usize, msg_info: usize, args: &[usize]) -> usize {
    let proc = proc::current();

    // 1. 查找 Capability
    let cap = match proc.cspace.get(cptr) {
        Some(c) => c,
        None => return 1, // Invalid Capability
    };

    // 2. 检查权限 (例如是否允许 Call)
    if !cap.can_invoke() {
        return 2; // Permission Denied
    }

    // 3. 根据对象类型分发
    match cap.object {
        CapType::Endpoint { id } => {
            // IPC 调用：发送消息并等待回复
            // 这是一个原子操作：Send + Recv
            ipc_method(id, cap.badge.unwrap_or(0), msg_info, args)
        }
        CapType::Process { pid } => {
            // Process 操作：如 Suspend, Resume, SetPriority
            // args[0] 通常是方法 ID
            tcb_method(pid, args)
        }
        CapType::PageTable { addr } => {
            // 页表操作：如 Map, Unmap
            page_table_method(addr, args)
        }
        // ... 处理其他类型 ...
        _ => usize::MAX, // 不支持的操作
    }
}

fn ipc_method(endpoint_id: usize, badge: usize, msg_info: usize, args: &[usize]) -> usize {
    // 简化实现：将 msg_info 和前 7 个参数打包为 8 个寄存器字
    let mut data = [0usize; 8];
    data[0] = msg_info; // MR0: Tag
    for (i, v) in args.iter().take(7).enumerate() {
        data[i + 1] = *v;
    }

    // 调用同步调用：Send + Recv（Call 语义）
    ipc::send(endpoint_id, badge, data);
    ipc::recv(endpoint_id);
    0
}

fn tcb_method(pid: usize, args: &[usize]) -> usize {
    // args[0]: method id
    if args.is_empty() {
        return usize::MAX;
    }
    let method = args[0];

    match method {
        // Resume: 使目标线程可运行
        0 => {
            let mut table = PROC_TABLE.lock();
            for i in 0..NPROC {
                if table[i].pid == pid {
                    // 只有非 Dying/Unused 的情况下设置 Ready
                    if table[i].state != ProcState::Unused {
                        table[i].state = ProcState::Ready;
                        return 0;
                    } else {
                        return 2; // invalid state
                    }
                }
            }
            1 // not found
        }
        // Suspend: 将目标线程置为 Sleeping（简化）
        1 => {
            let mut table = PROC_TABLE.lock();
            for i in 0..NPROC {
                if table[i].pid == pid {
                    table[i].state = ProcState::Sleeping;
                    return 0;
                }
            }
            1
        }
        // SetSP: 设置栈顶（args[1] = sp）
        2 => {
            if args.len() < 2 {
                return usize::MAX;
            }
            let sp = args[1];
            let mut table = PROC_TABLE.lock();
            for i in 0..NPROC {
                if table[i].pid == pid {
                    table[i].context.sp = sp;
                    return 0;
                }
            }
            1
        }
        // SetEntry: 设置入口（args[1] = pc），这里只修改 TrapFrame 的 kernel_epc 作为简化
        3 => {
            if args.len() < 2 {
                return usize::MAX;
            }
            let pc = args[1];
            let mut table = PROC_TABLE.lock();
            for i in 0..NPROC {
                if table[i].pid == pid {
                    unsafe {
                        if !table[i].trapframe.is_null() {
                            (*table[i].trapframe).kernel_epc = pc;
                        }
                    }
                    return 0;
                }
            }
            1
        }
        _ => usize::MAX,
    }
}

fn page_table_method(pt_addr: PhysAddr, args: &[usize]) -> usize {
    // 简化方法：
    // args[0] = method id (0=Map, 1=Unmap)
    // Map: args = [0, vaddr, paddr, size, flags]
    // Unmap: args = [1, vaddr, size]
    if args.is_empty() {
        return usize::MAX;
    }
    let method = args[0];

    let pt = unsafe { &mut *(pt_addr as *mut PageTable) };

    match method {
        0 => {
            if args.len() < 5 {
                return usize::MAX;
            }
            let vaddr = args[1];
            let paddr = args[2];
            let size = args[3];
            let flags = args[4];
            if pt.map(vaddr, paddr, size, flags) { 0 } else { 1 }
        }
        1 => {
            if args.len() < 3 {
                return usize::MAX;
            }
            let vaddr = args[1];
            let size = args[2];
            if pt.unmap(vaddr, size, true) { 0 } else { 1 }
        }
        _ => usize::MAX,
    }
}
