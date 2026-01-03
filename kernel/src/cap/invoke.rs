use super::method::{
    cnodemethod, consolemethod, ipcmethod, irqmethod, pagetablemethod, replymethod, tcbmethod,
    untypedmethod,
};
use crate::cap::captype::types;
use crate::cap::{CNode, CapType, Capability, rights};
use crate::hart;
use crate::ipc;
use crate::irq;
use crate::mem::{PGSIZE, PageTable, PhysAddr, PteFlags, VirtAddr};
use crate::proc::{TCB, scheduler};
use crate::trap::syscall::errcode;
use core::mem::size_of;

pub fn dispatch(cap: &Capability, method: usize) -> usize {
    // 4. 根据对象类型分发
    match cap.object {
        CapType::Endpoint { ep_ptr } => invoke_ipc(ep_ptr, &cap, method),
        CapType::Thread { tcb_ptr } => invoke_tcb(tcb_ptr, method),
        CapType::PageTable { paddr, .. } => invoke_pagetable(paddr, method),
        CapType::CNode { paddr, bits, .. } => invoke_cnode(paddr, bits, method),
        CapType::Untyped { start_paddr, size } => invoke_untyped(start_paddr, size, method),
        CapType::IrqHandler { irq } => invoke_irq_handler(irq, method),
        CapType::Reply { tcb_ptr } => invoke_reply(tcb_ptr, method),
        CapType::Console => invoke_console(method),
        _ => errcode::INVALID_OBJ_TYPE, // Error: Invalid Object Type for Invocation
    }
}

// --- IPC ipc::Endpoint Methods ---

fn invoke_ipc(ep_ptr: VirtAddr, cap: &Capability, method: usize) -> usize {
    let ep = ep_ptr.as_mut::<ipc::Endpoint>();
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let badge = cap.get_badge();

    // 获取 UTCB 以读取参数 (msg_info)
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => return errcode::MAPPING_FAILED,
    };

    match method {
        ipcmethod::SEND => {
            if !cap.has_rights(rights::SEND) {
                return errcode::PERMISSION_DENIED;
            }
            let msg_info = utcb.mrs_regs[0];
            // 通过 invoke 发送时，暂时不支持传递能力，或者从 UTCB 中提取
            let mut cap_to_send = None;
            let tag = ipc::MsgTag(msg_info);
            if tag.has_cap() {
                if let Some(cap) = tcb.cap_lookup(utcb.cap_transfer) {
                    if (cap.rights & rights::GRANT) != 0 {
                        cap_to_send = Some(cap);
                    }
                }
            }
            ipc::send(tcb, ep, badge, cap_to_send);
            errcode::SUCCESS
        }
        ipcmethod::RECV => {
            if !cap.has_rights(rights::RECV) {
                return errcode::PERMISSION_DENIED;
            }
            ipc::recv(tcb, ep);
            errcode::SUCCESS
        }
        ipcmethod::CALL => {
            if !cap.has_rights(rights::CALL) {
                return errcode::PERMISSION_DENIED;
            }
            let msg_info = utcb.mrs_regs[0];
            let mut cap_to_send = None;
            let tag = ipc::MsgTag(msg_info);
            if tag.has_cap() {
                if let Some(cap) = tcb.cap_lookup(utcb.cap_transfer) {
                    if (cap.rights & rights::GRANT) != 0 {
                        cap_to_send = Some(cap);
                    }
                }
            }
            ipc::call(tcb, ep, badge, cap_to_send);
            errcode::SUCCESS
        }
        ipcmethod::NOTIFY => {
            if !cap.has_rights(rights::SEND) {
                return errcode::PERMISSION_DENIED;
            }
            ipc::notify(ep, badge);
            errcode::SUCCESS
        }
        _ => errcode::INVALID_METHOD,
    }
}

fn invoke_reply(tcb_ptr: VirtAddr, method: usize) -> usize {
    let target_tcb = tcb_ptr.as_mut::<TCB>();
    let current_tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    match method {
        replymethod::REPLY => {
            ipc::reply(current_tcb, target_tcb);
            errcode::SUCCESS
        }
        _ => errcode::INVALID_METHOD,
    }
}

// --- TCB Methods ---

fn invoke_tcb(tcb_ptr: VirtAddr, method: usize) -> usize {
    let tcb = tcb_ptr.as_mut::<TCB>();
    let current_tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match current_tcb.get_utcb() {
        Some(u) => u,
        None => return errcode::MAPPING_FAILED,
    };

    match method {
        tcbmethod::CONFIGURE => {
            // args: [cspace_cptr, vspace_cptr, utcb_cptr, tf_cptr, kstack_cptr]
            let cspace_cptr = utcb.mrs_regs[0];
            let vspace_cptr = utcb.mrs_regs[1];
            let utcb_cptr = utcb.mrs_regs[2];
            let tf_cptr = utcb.mrs_regs[3];
            let kstack_cptr = utcb.mrs_regs[4];

            // 查找并验证能力
            let cspace_cap = current_tcb.cap_lookup(cspace_cptr);
            let vspace_cap = current_tcb.cap_lookup(vspace_cptr);
            let utcb_cap = current_tcb.cap_lookup(utcb_cptr);
            let tf_cap = current_tcb.cap_lookup(tf_cptr);
            let kstack_cap = current_tcb.cap_lookup(kstack_cptr);

            // 简化的配置逻辑
            tcb.configure(
                cspace_cap.as_ref(),
                vspace_cap.as_ref(),
                utcb_cap.as_ref(),
                tf_cap.as_ref(),
                kstack_cap.as_ref(),
            );
            errcode::SUCCESS
        }
        tcbmethod::SET_PRIORITY => {
            // SetPriority: (prio)
            let prio = utcb.mrs_regs[0] as u8;
            tcb.set_priority(prio);
            // 如果修改了优先级，可能需要触发重新调度
            scheduler::reschedule();
            errcode::SUCCESS
        }
        tcbmethod::SET_REGISTERS => {
            // SetRegisters: (entry, sp)
            // 简化版：只设置入口点和栈指针
            let entry = utcb.mrs_regs[0];
            let sp = utcb.mrs_regs[1];
            tcb.set_registers(entry, sp);
            errcode::SUCCESS
        }
        tcbmethod::SET_FAULT_HANDLER => {
            // SetFaultHandler: (ep_cptr)
            let ep_cptr = utcb.mrs_regs[0];
            if let Some(ep_cap) = current_tcb.cap_lookup(ep_cptr) {
                // Only accept ipc::Endpoint caps
                if let CapType::Endpoint { .. } = ep_cap.object {
                    tcb.set_fault_handler(ep_cap);
                    errcode::SUCCESS
                } else {
                    errcode::INVALID_OBJ_TYPE
                }
            } else {
                errcode::INVALID_CAP
            }
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

fn invoke_pagetable(paddr: PhysAddr, method: usize) -> usize {
    // PageTable 需要物理地址转虚拟地址才能操作
    let pt_ptr = paddr.to_va();
    let pt = pt_ptr.as_mut::<PageTable>();
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => return errcode::MAPPING_FAILED,
    };

    match method {
        pagetablemethod::MAP => {
            // Map: (frame_cap, vaddr, flags)
            let frame_cptr = utcb.mrs_regs[0];
            let vaddr = VirtAddr::from(utcb.mrs_regs[1]);
            let flags = PteFlags::from(utcb.mrs_regs[2]);

            let frame_cap = match tcb.cap_lookup(frame_cptr) {
                Some(c) => c,
                None => return errcode::INVALID_CAP,
            };

            let (frame_paddr, frame_size) = match frame_cap.object {
                CapType::Frame { paddr, page_count } => (paddr, page_count * PGSIZE),
                _ => return errcode::INVALID_OBJ_TYPE,
            };

            // 执行映射
            match pt.map(vaddr, frame_paddr, frame_size, flags) {
                Ok(()) => errcode::SUCCESS,
                Err(_) => errcode::MAPPING_FAILED,
            }
        }
        pagetablemethod::UNMAP => {
            // Unmap: (vaddr, size)
            let vaddr = VirtAddr::from(utcb.mrs_regs[0]);
            let size = utcb.mrs_regs[1];
            match pt.unmap(vaddr, size) {
                Ok(()) => errcode::SUCCESS,
                Err(_) => errcode::MAPPING_FAILED,
            }
        }
        _ => errcode::INVALID_METHOD,
    }
}

// --- CNode methods ---

fn invoke_cnode(paddr: PhysAddr, bits: u8, method: usize) -> usize {
    let mut cnode = CNode::from_addr(paddr, bits);
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => return errcode::MAPPING_FAILED,
    };

    match method {
        cnodemethod::MINT => {
            // Mint: (src_cptr, dest_slot, badge, rights)
            let src_cptr = utcb.mrs_regs[0];
            let dest_slot = utcb.mrs_regs[1];
            let badge_val = utcb.mrs_regs[2];
            let rights = utcb.mrs_regs[3];
            let badge = if badge_val == 0 { None } else { Some(badge_val) };

            if let Some((src_cap, src_slot_addr)) = tcb.cap_lookup_slot(src_cptr) {
                let new_cap = src_cap.mint(badge, rights as u8);
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
            let src_cptr = utcb.mrs_regs[0];
            let dest_slot = utcb.mrs_regs[1];
            let rights = utcb.mrs_regs[2] as u8;

            if let Some((src_cap, src_slot_addr)) = tcb.cap_lookup_slot(src_cptr) {
                let new_cap = src_cap.mint(None, rights);
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
            let slot = utcb.mrs_regs[0];
            let slot_addr = cnode.get_slot_addr(slot);
            if slot_addr != PhysAddr::null() {
                cnode.delete(slot);
                errcode::SUCCESS
            } else {
                errcode::INVALID_SLOT
            }
        }
        cnodemethod::REVOKE => {
            // Revoke: (slot)
            let slot = utcb.mrs_regs[0];
            let slot_addr = cnode.get_slot_addr(slot);
            if slot_addr != PhysAddr::null() {
                cnode.revoke(slot);
                errcode::SUCCESS
            } else {
                errcode::INVALID_SLOT
            }
        }
        _ => errcode::INVALID_METHOD,
    }
}

fn invoke_untyped(start: PhysAddr, size: usize, method: usize) -> usize {
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => return errcode::MAPPING_FAILED,
    };

    match method {
        untypedmethod::RETYPE => {
            // Retype: (type, obj_size_bits, n_objects, dest_cnode_cptr, dest_slot_offset)
            let obj_type = utcb.mrs_regs[0];
            let obj_size_bits = utcb.mrs_regs[1];
            let n_objects = utcb.mrs_regs[2];
            let dest_cnode_cptr = utcb.mrs_regs[3];
            let dest_slot_offset = utcb.mrs_regs[4];

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
                        types::FRAME => {
                            Capability::create_frame(obj_paddr, obj_size / PGSIZE, rights::ALL)
                        }
                        // PageTable
                        types::PAGETABLE => {
                            // 初始化页表 (清零已在上面完成)
                            Capability::create_pagetable(obj_paddr, 0, rights::ALL)
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

fn invoke_irq_handler(irq: usize, method: usize) -> usize {
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => return errcode::MAPPING_FAILED,
    };

    match method {
        irqmethod::SET_NOTIFICATION => {
            // SetNotification: args[0] = ep_cptr
            let ep_cptr = utcb.mrs_regs[0];

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
            let priority = utcb.mrs_regs[0];
            irq::plic::set_priority(irq, priority);
            errcode::SUCCESS
        }
        _ => errcode::INVALID_METHOD,
    }
}

fn invoke_console(method: usize) -> usize {
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => return errcode::MAPPING_FAILED,
    };

    match method {
        consolemethod::PUT_CHAR => {
            let c = utcb.mrs_regs[0] as u8 as char;
            crate::printk!("{}", c);
            errcode::SUCCESS
        }
        consolemethod::PUT_STR => {
            let offset = utcb.mrs_regs[0];
            let len = utcb.mrs_regs[1];
            if let Some(s) = utcb.get_str(offset, len) {
                crate::printk!("{}", s);
                errcode::SUCCESS
            } else {
                errcode::INVALID_SLOT
            }
        }
        _ => errcode::INVALID_METHOD,
    }
}
