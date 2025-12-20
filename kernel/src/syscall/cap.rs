use crate::cap::{CNode, CapType, Capability, rights};
use crate::ipc;
use crate::ipc::endpoint::Endpoint;
use crate::irq::{TrapContext, TrapFrame};
use crate::mem::PGSIZE;
use crate::mem::addr;
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
            let pt_ptr = addr::phys_to_virt(paddr);
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
            scheduler::reschedule();
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
            scheduler::add_thread(tcb);
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
            match pt.map(vaddr, paddr, mem::PGSIZE, flags) {
                Ok(()) => 0,
                Err(_) => 5, // Error: Mapping Failed
            }
        }
        // Unmap: (vaddr)
        2 => {
            let vaddr = VirtAddr::from(args[0]);
            match pt.unmap(vaddr, PGSIZE) {
                Ok(()) => 0,
                Err(_) => 6, // Error: Unmapping Failed
            }
        }
        _ => 4,
    }
}

// --- Other Methods (Stubs) ---

fn invoke_cnode(paddr: PhysAddr, bits: u8, method: usize, args: &[usize]) -> usize {
    let mut cnode = CNode::from_addr(paddr, bits);
    match method {
        // Mint: (src_cptr, dest_slot, badge, rights)
        1 => {
            let src_cptr = args[0];
            let dest_slot = args[1];
            let badge = if args[2] != 0 { Some(args[2]) } else { None };
            let rights = args[3] as u8;

            let current_tcb = proc::current();
            if let Some((src_cap, src_slot_addr)) = current_tcb.cap_lookup_slot(src_cptr) {
                let mut new_cap = src_cap.mint(badge);
                new_cap.rights &= rights;
                if cnode.insert_child(dest_slot, new_cap, src_slot_addr) { 0 } else { 7 }
            } else {
                1
            }
        }
        // Copy: (src_cptr, dest_slot, rights)
        2 => {
            let src_cptr = args[0];
            let dest_slot = args[1];
            let rights = args[2] as u8;

            let current_tcb = proc::current();
            if let Some((src_cap, src_slot_addr)) = current_tcb.cap_lookup_slot(src_cptr) {
                let mut new_cap = src_cap.clone();
                new_cap.rights &= rights;
                if cnode.insert_child(dest_slot, new_cap, src_slot_addr) { 0 } else { 7 }
            } else {
                1
            }
        }
        // Delete: (slot)
        3 => {
            let slot = args[0];
            let slot_addr = cnode.get_slot_addr(slot);
            if slot_addr != 0 {
                delete_recursive(slot_addr);
                0
            } else {
                7
            }
        }
        // Revoke: (slot)
        4 => {
            let slot = args[0];
            let slot_addr = cnode.get_slot_addr(slot);
            if slot_addr != 0 {
                revoke_recursive(slot_addr);
                0
            } else {
                7
            }
        }
        _ => 4,
    }
}

fn revoke_recursive(slot_addr: PhysAddr) {
    use crate::cap::cnode::Slot;
    let slot = unsafe { &mut *(slot_addr as *mut Slot) };
    let mut child_addr = slot.cdt.first_child;
    while child_addr != 0 {
        let next_sibling = unsafe { (*(child_addr as *mut Slot)).cdt.next_sibling };
        delete_recursive(child_addr);
        child_addr = next_sibling;
    }
    slot.cdt.first_child = 0;
}

fn delete_recursive(slot_addr: PhysAddr) {
    use crate::cap::cnode::Slot;
    // 1. 递归撤销所有子能力
    revoke_recursive(slot_addr);

    // 2. 从 CDT 兄弟链表中移除
    unsafe {
        let slot = &mut *(slot_addr as *mut Slot);
        let prev = slot.cdt.prev_sibling;
        let next = slot.cdt.next_sibling;
        let parent = slot.cdt.parent;

        if prev != 0 {
            (*(prev as *mut Slot)).cdt.next_sibling = next;
        } else if parent != 0 {
            (*(parent as *mut Slot)).cdt.first_child = next;
        }

        if next != 0 {
            (*(next as *mut Slot)).cdt.prev_sibling = prev;
        }

        // 3. 清空槽位 (触发 Capability::drop)
        slot.cap = crate::cap::Capability::empty();
        slot.cdt = crate::cap::cnode::CDTNode::new();
    }
}

fn invoke_untyped(start: PhysAddr, size: usize, method: usize, args: &[usize]) -> usize {
    match method {
        // Retype: (type, obj_size_bits, n_objects, dest_cnode_cptr, dest_slot_offset)
        1 => {
            let obj_type = args[0];
            let obj_size_bits = args[1];
            let n_objects = args[2];
            let dest_cnode_cptr = args[3];
            let dest_slot_offset = args[4];

            let current_tcb = proc::current();
            let dest_cnode_cap = match current_tcb.cap_lookup(dest_cnode_cptr) {
                Some(c) => c,
                None => return 1,
            };

            if let CapType::CNode { paddr: cn_paddr, bits: cn_bits } = dest_cnode_cap.object {
                let mut dest_cnode = crate::cap::CNode::from_addr(cn_paddr, cn_bits);

                let obj_size = 1 << obj_size_bits;
                // 检查总大小
                if n_objects * obj_size > size {
                    return 8; // Error: Untyped Out of Memory
                }

                for i in 0..n_objects {
                    let obj_paddr = start + i * obj_size;
                    let obj_vaddr = addr::phys_to_virt(obj_paddr);

                    // 必须清零内存，防止旧数据残留
                    unsafe { core::ptr::write_bytes(obj_vaddr as *mut u8, 0, obj_size) };

                    let new_cap = match obj_type {
                        // CNode
                        1 => {
                            // CNode 需要初始化 Header
                            // obj_size_bits 是 CNode 的 slot 数量 log2
                            // 实际上我们需要分配的空间 = Header + slots * sizeof(Cap)
                            // 这里假设用户已经计算好了足够的 obj_size_bits 来容纳这一切

                            // 采用 seL4 方式：obj_size_bits 指定 CNode 的 slot log2。
                            // 对象实际大小 = 2^obj_size_bits * 16 bytes (slot size).
                            // 我们忽略 Header 的开销 (假设它很小或者我们偷用第一个 slot?)
                            // 为了正确性，我们使用 CNode::new 初始化 Header
                            let _ = crate::cap::CNode::new(obj_paddr, obj_size_bits as u8);
                            Capability::create_cnode(obj_paddr, obj_size_bits as u8, rights::ALL)
                        }
                        // TCB
                        2 => {
                            if obj_size < core::mem::size_of::<TCB>() {
                                return 9; // Error: Object Size Too Small
                            }
                            let tcb_ptr = obj_vaddr as *mut TCB;
                            unsafe { tcb_ptr.write(TCB::new()) };
                            Capability::create_thread(obj_vaddr, rights::ALL)
                        }
                        // Endpoint
                        3 => {
                            if obj_size < core::mem::size_of::<Endpoint>() {
                                return 9;
                            }
                            let ep_ptr = obj_vaddr as *mut Endpoint;
                            unsafe { ep_ptr.write(Endpoint::new()) };
                            Capability::create_endpoint(obj_vaddr, rights::ALL)
                        }
                        // Frame
                        4 => Capability::create_frame(obj_paddr, rights::ALL),
                        // PageTable
                        5 => {
                            // 初始化页表 (清零已在上面完成)
                            Capability::create_pagetable(obj_paddr, 0, 0, rights::ALL)
                        }
                        _ => return 3,
                    };

                    if !dest_cnode.insert(dest_slot_offset + i, new_cap) {
                        return 7;
                    }
                }
                0
            } else {
                3
            }
        }
        _ => 4,
    }
}
