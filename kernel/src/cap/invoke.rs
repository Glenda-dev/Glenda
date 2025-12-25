use super::method::{cnodemethod, ipcmethod, irqmethod, pagetablemethod, tcbmethod, untypedmethod};
use crate::cap::captype::types;
use crate::cap::cnode;
use crate::cap::{CNode, CapType, Capability, rights};
use crate::hart;
use crate::ipc;
use crate::irq;
use crate::mem;
use crate::mem::{PGSIZE, PageTable, PhysAddr, PteFlags, VirtAddr};
use crate::proc;
use crate::proc::{TCB, scheduler};
use crate::trap::syscall::{Args, errcode};
use core::mem::size_of;

pub fn dispatch(cap: &Capability, method: usize, args: &Args) -> usize {
    // 4. 根据对象类型分发
    match cap.object {
        CapType::Endpoint { ep_ptr } => invoke_ipc(ep_ptr, &cap, method, &args),
        CapType::Thread { tcb_ptr } => invoke_tcb(tcb_ptr, method, &args),
        CapType::PageTable { paddr, .. } => invoke_pagetable(paddr, method, &args),
        CapType::CNode { paddr, bits, .. } => invoke_cnode(paddr, bits, method, &args),
        CapType::Untyped { start_paddr, size } => invoke_untyped(start_paddr, size, method, &args),
        CapType::IrqHandler { irq } => invoke_irq_handler(irq, method, &args),
        _ => errcode::INVALID_OBJ_TYPE, // Error: Invalid Object Type for Invocation
    }
}

// --- IPC ipc::Endpoint Methods ---

fn invoke_ipc(ep_ptr: VirtAddr, _cap: &Capability, method: usize, args: &Args) -> usize {
    let ep = ep_ptr.as_mut::<ipc::Endpoint>();
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    match method {
        ipcmethod::SEND => {
            let msg_info = args[0];
            // 通过 invoke 发送时，暂时不支持传递能力，或者从 UTCB 中提取
            let mut cap_to_send = None;
            let tag = ipc::MsgTag(msg_info);
            if tag.has_cap() {
                if let Some(utcb) = tcb.get_utcb() {
                    if let Some(cap) = tcb.cap_lookup(utcb.cap_transfer) {
                        if (cap.rights & rights::GRANT) != 0 {
                            cap_to_send = Some(cap);
                        }
                    }
                }
            }
            ipc::send(tcb, ep, msg_info, cap_to_send);
            errcode::SUCCESS
        }
        ipcmethod::RECV => {
            ipc::recv(tcb, ep);
            errcode::SUCCESS
        }
        _ => errcode::INVALID_METHOD,
    }
}

// --- TCB Methods ---

fn invoke_tcb(tcb_ptr: VirtAddr, method: usize, args: &Args) -> usize {
    let tcb = tcb_ptr.as_mut::<TCB>();
    match method {
        tcbmethod::CONFIGURE => {
            // Configure: (cspace, vspace, utcb, fault_ep)
            // args: [cspace_cptr, vspace_cptr, utcb_addr, fault_ep_cptr]
            let cspace_cptr = args[0];
            let vspace_cptr = args[1];
            let utcb_addr = args[2];
            let fault_ep_cptr = args[3];

            let tcb =
                unsafe { &mut *scheduler::current().expect("No current TCB in exception handler") };

            // 查找并验证能力
            let cspace_cap = tcb.cap_lookup(cspace_cptr);
            let vspace_cap = tcb.cap_lookup(vspace_cptr);
            let fault_cap = if fault_ep_cptr != 0 { tcb.cap_lookup(fault_ep_cptr) } else { None };

            // 简化的配置逻辑
            if let (Some(cs), Some(vs)) = (cspace_cap, vspace_cap) {
                // 实际实现中需要更严格的类型检查
                tcb.cspace_root = cs;
                tcb.vspace_root = vs;
                tcb.utcb_base = VirtAddr::from(utcb_addr);
                tcb.fault_handler = fault_cap;
                errcode::SUCCESS
            } else {
                errcode::INVALID_CAP
            }
        }
        tcbmethod::SET_PRIORITY => {
            // SetPriority: (prio)
            let prio = args[0] as u8;
            tcb.set_priority(prio);
            // 如果修改了优先级，可能需要触发重新调度
            scheduler::reschedule();
            errcode::SUCCESS
        }
        tcbmethod::SET_REGISTERS => {
            // SetRegisters: (flags, arch_flags, ...)
            // 参数通常从 UTCB 读取，因为寄存器太多放不下
            // 读取 UTCB 中的寄存器状态并写入 tcb.context
            unimplemented!();
        }
        tcbmethod::RESUME => {
            // Resume
            tcb.resume();
            // 将线程加入调度队列
            scheduler::add_thread(tcb);
            errcode::SUCCESS
        }
        tcbmethod::SUSPEND => {
            // Suspend
            tcb.suspend();
            // 如果目标是当前线程，需要触发 yield
            scheduler::yield_proc();
            errcode::SUCCESS
        }
        _ => errcode::INVALID_METHOD,
    }
}

// --- PageTable Methods ---

fn invoke_pagetable(paddr: PhysAddr, method: usize, args: &Args) -> usize {
    // PageTable 需要物理地址转虚拟地址才能操作
    let pt_ptr = paddr.to_va();
    let pt = pt_ptr.as_mut::<PageTable>();
    match method {
        pagetablemethod::MAP => {
            // Map: (frame_cap, vaddr, flags)
            let paddr = PhysAddr::from(args[0]);
            let vaddr = VirtAddr::from(args[1]);
            let flags = PteFlags::from(args[2]);

            // 执行映射
            // pt.map(vaddr, paddr, flags)
            match pt.map(vaddr, paddr, mem::PGSIZE, flags) {
                Ok(()) => errcode::SUCCESS,
                Err(_) => errcode::MAPPING_FAILED,
            }
        }
        pagetablemethod::UNMAP => {
            // Unmap: (vaddr)
            let vaddr = VirtAddr::from(args[0]);
            match pt.unmap(vaddr, PGSIZE) {
                Ok(()) => errcode::SUCCESS,
                Err(_) => errcode::MAPPING_FAILED,
            }
        }
        _ => errcode::INVALID_METHOD,
    }
}

// --- CNode methods ---

fn invoke_cnode(paddr: PhysAddr, bits: u8, method: usize, args: &Args) -> usize {
    let mut cnode = CNode::from_addr(paddr, bits);
    match method {
        cnodemethod::MINT => {
            // Mint: (src_cptr, dest_slot, badge, rights)
            let src_cptr = args[0];
            let dest_slot = args[1];
            let badge = if args[2] != 0 { Some(args[2]) } else { None };
            let rights = args[3] as u8;

            let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
            if let Some((src_cap, src_slot_addr)) = tcb.cap_lookup_slot(src_cptr) {
                let new_cap = src_cap.mint(rights, badge);
                if cnode.insert_child(dest_slot, &new_cap, src_slot_addr) {
                    errcode::SUCCESS
                } else {
                    errcode::INVALID_SLOT
                }
            } else {
                errcode::INVALID_CAP
            }
        }
        cnodemethod::COPY => {
            // Copy: (src_cptr, dest_slot, rights)
            let src_cptr = args[0];
            let dest_slot = args[1];
            let rights = args[2] as u8;

            let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
            if let Some((src_cap, src_slot_addr)) = tcb.cap_lookup_slot(src_cptr) {
                let new_cap = src_cap.mint(rights, None);
                if cnode.insert_child(dest_slot, &new_cap, src_slot_addr) {
                    errcode::SUCCESS
                } else {
                    errcode::INVALID_SLOT
                }
            } else {
                errcode::INVALID_CAP
            }
        }
        cnodemethod::DELETE => {
            // Delete: (slot)
            let slot = args[0];
            let slot_addr = cnode.get_slot_addr(slot);
            if slot_addr != PhysAddr::null() {
                cnode::delete_recursive(slot_addr);
                errcode::SUCCESS
            } else {
                errcode::INVALID_SLOT
            }
        }
        cnodemethod::REVOKE => {
            // Revoke: (slot)
            let slot = args[0];
            let slot_addr = cnode.get_slot_addr(slot);
            if slot_addr != PhysAddr::null() {
                cnode::revoke_recursive(slot_addr);
                errcode::SUCCESS
            } else {
                errcode::INVALID_SLOT
            }
        }
        _ => errcode::INVALID_METHOD,
    }
}

fn invoke_untyped(start: PhysAddr, size: usize, method: usize, args: &Args) -> usize {
    match method {
        untypedmethod::RETYPE => {
            // Retype: (type, obj_size_bits, n_objects, dest_cnode_cptr, dest_slot_offset)
            let obj_type = args[0];
            let obj_size_bits = args[1];
            let n_objects = args[2];
            let dest_cnode_cptr = args[3];
            let dest_slot_offset = args[4];

            let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
            let dest_cnode_cap = match tcb.cap_lookup(dest_cnode_cptr) {
                Some(c) => c,
                None => return errcode::INVALID_CAP,
            };

            if let CapType::CNode { paddr: cn_paddr, bits: cn_bits } = dest_cnode_cap.object {
                let mut dest_cnode = crate::cap::CNode::from_addr(cn_paddr, cn_bits);

                let obj_size = 1 << obj_size_bits;
                // 检查总大小
                if n_objects * obj_size > size {
                    return errcode::UNTYPE_OOM;
                }

                for i in 0..n_objects {
                    let obj_paddr = PhysAddr::from(start.as_usize() + i * obj_size);
                    let obj_vaddr = obj_paddr.to_va();

                    // 必须清零内存，防止旧数据残留
                    unsafe { core::ptr::write_bytes(obj_vaddr.as_mut_ptr::<u8>(), 0, obj_size) };

                    let new_cap = match obj_type {
                        // CNode
                        types::CNODE => {
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
                        types::TCB => {
                            if obj_size < size_of::<TCB>() {
                                return errcode::INVALID_OBJ_TYPE;
                            }
                            let tcb_ptr = obj_vaddr.as_mut_ptr::<TCB>();
                            unsafe { tcb_ptr.write(TCB::new()) };
                            Capability::create_thread(obj_vaddr, rights::ALL)
                        }
                        // ipc::Endpoint
                        types::ENDPOINT => {
                            if obj_size < size_of::<ipc::Endpoint>() {
                                return errcode::INVALID_OBJ_TYPE;
                            }
                            let ep_ptr = obj_vaddr.as_mut_ptr::<ipc::Endpoint>();
                            unsafe { ep_ptr.write(ipc::Endpoint::new()) };
                            Capability::create_endpoint(obj_vaddr, rights::ALL)
                        }
                        // Frame
                        types::FRAME => Capability::create_frame(obj_paddr, rights::ALL),
                        // PageTable
                        types::PAGETABLE => {
                            // 初始化页表 (清零已在上面完成)
                            Capability::create_pagetable(
                                obj_paddr,
                                VirtAddr::null(),
                                0,
                                rights::ALL,
                            )
                        }
                        _ => return errcode::INVALID_OBJ_TYPE,
                    };

                    if !dest_cnode.insert(dest_slot_offset + i, &new_cap) {
                        return errcode::INVALID_SLOT;
                    }
                }
                errcode::SUCCESS
            } else {
                errcode::INVALID_OBJ_TYPE
            }
        }
        _ => errcode::INVALID_METHOD,
    }
}

fn invoke_irq_handler(irq: usize, method: usize, args: &Args) -> usize {
    match method {
        irqmethod::SET_NOTIFICATION => {
            // SetNotification: args[0] = ep_cptr
            let ep_cptr = args[0];

            let tcb =
                unsafe { &mut *scheduler::current().expect("No current TCB in exception handler") };
            if let Some(ep_cap) = tcb.cap_lookup(ep_cptr) {
                // Only accept ipc::Endpoint caps
                if let CapType::Endpoint { .. } = ep_cap.object {
                    irq::bind_notification(irq, ep_cap.clone());
                    errcode::SUCCESS
                } else {
                    errcode::INVALID_OBJ_TYPE
                }
            } else {
                errcode::INVALID_CAP
            }
        }
        irqmethod::ACK => {
            // Ack: acknowledge handled IRQ and unmask
            let hartid = hart::get().id;
            irq::ack_irq(hartid, irq);
            errcode::SUCCESS
        }
        irqmethod::CLEAR_NOTIFICATION => {
            // Clear binding
            irq::clear_notification(irq);
            errcode::SUCCESS
        }
        irqmethod::SET_PRIORITY => {
            // SetPriority: args[0] = priority
            let priority = args[0];
            irq::plic::set_priority(irq, priority);
            errcode::SUCCESS
        }
        _ => errcode::INVALID_METHOD,
    }
}
