use crate::cap::cnode;
use crate::cap::{CNode, CapType, Capability, rights};
use crate::hart;
use crate::ipc;
use crate::ipc::Endpoint;
use crate::irq;
use crate::mem::PGSIZE;
use crate::mem::addr;
use crate::mem::{self, PageTable, PhysAddr, VirtAddr};
use crate::proc::{self, TCB, scheduler};

pub fn dispatch(cap: &Capability, method: usize, args: &[usize]) -> usize {
    // 4. 根据对象类型分发
    match cap.object {
        CapType::Endpoint { ep_ptr } => invoke_ipc(ep_ptr, &cap, method, &args),
        CapType::Thread { tcb_ptr } => invoke_tcb(tcb_ptr, method, &args),
        CapType::PageTable { paddr, .. } => invoke_pagetable(paddr, method, &args),
        CapType::CNode { paddr, bits, .. } => invoke_cnode(paddr, bits, method, &args),
        CapType::Untyped { start_paddr, size } => invoke_untyped(start_paddr, size, method, &args),
        CapType::IrqHandler { irq } => invoke_irq_handler(irq, method, &args),
        _ => 3, // Error: Invalid Object Type for Invocation
    }
}

// --- IPC Endpoint Methods ---

fn invoke_ipc(ep_ptr: usize, _cap: &Capability, method: usize, args: &[usize]) -> usize {
    let ep = unsafe { &mut *(ep_ptr as *mut Endpoint) };
    let tcb = proc::current();
    match method {
        // Send
        1 => {
            let msg_info = args[0];
            ipc::send(tcb, ep, msg_info);
            0
        }
        // Receive
        2 => {
            ipc::recv(tcb, ep);
            0
        }
        _ => 4, // Error: Invalid Method
    }
}

// --- TCB Methods ---

fn invoke_tcb(tcb_ptr: usize, method: usize, args: &[usize]) -> usize {
    let tcb = unsafe { &mut *(tcb_ptr as *mut TCB) };
    match method {
        // Configure: (cspace, vspace, utcb, fault_ep)
        // args: [cspace_cptr, vspace_cptr, utcb_addr, fault_ep_cptr]
        1 => {
            // 注意：这里传递的是 CPTR，内核需要再次查找这些 Cap 对应的内核对象
            // 这是一个简化的实现，实际需要验证这些 Cap 的有效性
            unimplemented!()
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

fn invoke_pagetable(paddr: PhysAddr, method: usize, args: &[usize]) -> usize {
    // PageTable 需要物理地址转虚拟地址才能操作
    let pt_ptr = addr::phys_to_virt(paddr);
    let pt = unsafe { &mut *(pt_ptr as *mut PageTable) };
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

// --- CNode methods ---

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
                cnode::delete_recursive(slot_addr);
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
                cnode::revoke_recursive(slot_addr);
                0
            } else {
                7
            }
        }
        _ => 4,
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
                            let _ = CNode::new(obj_paddr, obj_size_bits as u8);
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

fn invoke_irq_handler(irq: usize, method: usize, args: &[usize]) -> usize {
    match method {
        // SetNotification: args[0] = ep_cptr
        1 => {
            let ep_cptr = args[0];
            let current = proc::current();
            if let Some(ep_cap) = current.cap_lookup(ep_cptr) {
                // Only accept Endpoint caps
                if let CapType::Endpoint { .. } = ep_cap.object {
                    irq::bind_notification(irq, ep_cap.clone());
                    0
                } else {
                    2
                }
            } else {
                1
            }
        }
        // Ack: acknowledge handled IRQ and unmask
        2 => {
            let hartid = hart::getid();
            irq::ack_irq(hartid, irq);
            0
        }
        // Clear binding
        3 => {
            irq::clear_notification(irq);
            0
        }
        // SetPriority: args[0] = priority
        4 => {
            let priority = args[0];
            irq::plic::set_priority(irq, priority);
            0
        }
        _ => 4,
    }
}
