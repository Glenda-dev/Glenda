use super::method::{
    cnodemethod, consolemethod, ipcmethod, irqmethod, pagetablemethod, replymethod, tcbmethod,
    untypedmethod,
};
use crate::cap::captype::types;
use crate::cap::{CNode, CapType, Capability, Slot, rights};
use crate::hart;
use crate::ipc;
use crate::irq;
use crate::mem::{PGSIZE, PageTable, PhysAddr, PteFlags, VirtAddr};
use crate::proc::{TCB, scheduler};
use crate::trap::syscall::errcode;

pub fn dispatch(cap: &Capability, cptr: usize, method: usize) -> usize {
    // 4. 根据对象类型分发
    match cap.object {
        CapType::Endpoint { .. } => invoke_ipc(cap, cptr, method),
        CapType::Thread { .. } => invoke_tcb(cap, cptr, method),
        CapType::PageTable { .. } => invoke_pagetable(cap, cptr, method),
        CapType::CNode { .. } => invoke_cnode(cap, cptr, method),
        CapType::Untyped { .. } => invoke_untyped(cap, cptr, method),
        CapType::IrqHandler { .. } => invoke_irq_handler(cap, cptr, method),
        CapType::Reply { .. } => invoke_reply(cap, cptr, method),
        CapType::Console => invoke_console(cap, cptr, method),
        _ => errcode::INVALID_OBJ_TYPE, // Error: Invalid Object Type for Invocation
    }
}

// --- IPC ipc::Endpoint Methods ---

fn invoke_ipc(cap: &Capability, _cptr: usize, method: usize) -> usize {
    let ep_ptr = match cap.object {
        CapType::Endpoint { ep_ptr } => ep_ptr,
        _ => return errcode::INVALID_OBJ_TYPE,
    };

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
            let tag = utcb.msg_tag;
            // 通过 invoke 发送时，暂时不支持传递能力，或者从 UTCB 中提取
            let mut cap_to_send = None;
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
            let tag = utcb.msg_tag;
            let mut cap_to_send = None;
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

fn invoke_reply(cap: &Capability, _cptr: usize, method: usize) -> usize {
    let tcb_ptr = match cap.object {
        CapType::Reply { tcb_ptr } => tcb_ptr,
        _ => return errcode::INVALID_OBJ_TYPE,
    };

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

fn invoke_tcb(cap: &Capability, _cptr: usize, method: usize) -> usize {
    let tcb_ptr = match cap.object {
        CapType::Thread { tcb_ptr } => tcb_ptr,
        _ => return errcode::INVALID_OBJ_TYPE,
    };

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
            // SetRegisters: (flags, entry, sp)
            // 简化版：只设置入口点和栈指针
            let _flags = utcb.mrs_regs[0];
            let entry = utcb.mrs_regs[1];
            let sp = utcb.mrs_regs[2];
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
        tcbmethod::SET_AFFINITY => {
            // SetAffinity: (hart_id)
            let hart_id = utcb.mrs_regs[0];
            tcb.set_affinity(hart_id);
            errcode::SUCCESS
        }
        tcbmethod::RESUME => {
            // Resume
            tcb.resume();
            // 将线程加入调度队列
            scheduler::add_thread(tcb);
            // 2. 抢占检查
            // 如果目标核心是当前核心，且优先级高于当前线程，则触发重新调度
            let current_hart = hart::getid();
            let target_hart =
                if tcb.affinity < hart::MAX_HARTS { tcb.affinity } else { current_hart };

            if target_hart == current_hart {
                if let Some(curr_ptr) = scheduler::current() {
                    // SAFETY: current() 返回的指针在内核运行期间有效
                    let curr = unsafe { &*curr_ptr };
                    if tcb.priority >= curr.priority {
                        scheduler::reschedule();
                    }
                } else {
                    // 当前没有运行线程（Idle），立即调度
                    scheduler::reschedule();
                }
            }
            errcode::SUCCESS
        }
        tcbmethod::SUSPEND => {
            // Suspend
            tcb.suspend();
            if tcb as *const TCB == current_tcb as *const TCB {
                scheduler::block_current_thread();
            }
            errcode::SUCCESS
        }
        _ => errcode::INVALID_METHOD,
    }
}

// --- PageTable Methods ---

fn invoke_pagetable(cap: &Capability, _cptr: usize, method: usize) -> usize {
    let paddr = match cap.object {
        CapType::PageTable { paddr, .. } => paddr,
        _ => return errcode::INVALID_OBJ_TYPE,
    };

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
        pagetablemethod::MAP_TABLE => {
            // MapTable: (table_cap, vaddr, level)
            let table_cptr = utcb.mrs_regs[0];
            let vaddr = VirtAddr::from(utcb.mrs_regs[1]);
            let level = utcb.mrs_regs[2];

            let table_cap = match tcb.cap_lookup(table_cptr) {
                Some(c) => c,
                None => return errcode::INVALID_CAP,
            };

            let table_paddr = match table_cap.object {
                CapType::PageTable { paddr, .. } => paddr,
                _ => return errcode::INVALID_OBJ_TYPE,
            };

            match pt.map_table(vaddr, table_paddr, level) {
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
        pagetablemethod::MAP_TRAMPOLINE => match pt.map_trampoline() {
            Ok(()) => errcode::SUCCESS,
            Err(_) => errcode::MAPPING_FAILED,
        },
        pagetablemethod::DEBUG_PRINT => {
            pt.debug_print();
            errcode::SUCCESS
        }

        _ => errcode::INVALID_METHOD,
    }
}

// --- CNode methods ---

fn invoke_cnode(cap: &Capability, _cptr: usize, method: usize) -> usize {
    let paddr = match cap.object {
        CapType::CNode { paddr } => paddr,
        _ => return errcode::INVALID_OBJ_TYPE,
    };

    let mut cnode = CNode::from_addr(paddr);
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

fn invoke_untyped(cap: &Capability, cptr: usize, method: usize) -> usize {
    let (start, total_pages, free_pages) = match cap.object {
        CapType::Untyped { start_paddr, total_pages, free_pages } => {
            (start_paddr, total_pages, free_pages)
        }
        _ => return errcode::INVALID_OBJ_TYPE,
    };

    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => return errcode::MAPPING_FAILED,
    };

    match method {
        untypedmethod::RETYPE => {
            // Retype: (type, obj_pages, n_objects, dest_cnode_cptr, dest_slot_offset, dirty)
            let obj_type = utcb.mrs_regs[0];

            let obj_pages = utcb.mrs_regs[1];
            let n_objects = utcb.mrs_regs[2];
            let dest_cnode_cptr = utcb.mrs_regs[3];
            let dest_slot_offset = utcb.mrs_regs[4];
            let dirty = utcb.mrs_regs[5];

            let dest_cnode_cap = match tcb.cap_lookup(dest_cnode_cptr) {
                Some(c) => c,
                None => return errcode::INVALID_CAP,
            };

            if let CapType::CNode { paddr: cn_paddr } = dest_cnode_cap.object {
                let mut dest_cnode = crate::cap::CNode::from_addr(cn_paddr);

                // 检查总大小
                let needed_pages = n_objects * obj_pages;
                if free_pages + needed_pages > total_pages {
                    return errcode::UNTYPE_OOM;
                }

                let current_page_offset = free_pages;

                for i in 0..n_objects {
                    let page_idx = current_page_offset + i * obj_pages;
                    let obj_paddr = PhysAddr::from(start.as_usize() + page_idx * PGSIZE);
                    let obj_vaddr = obj_paddr.to_va();
                    let obj_size_bytes = obj_pages * PGSIZE;

                    // 必须清零内存，防止旧数据残留 (除非是设备内存)
                    if dirty == 0 {
                        unsafe {
                            core::ptr::write_bytes(obj_vaddr.as_mut_ptr::<u8>(), 0, obj_size_bytes)
                        };
                    }

                    let new_cap = match obj_type {
                        // CNode
                        types::CNODE => {
                            if obj_pages != 1 {
                                return errcode::INVALID_OBJ_TYPE;
                            }
                            let _ = CNode::new(obj_paddr);
                            Capability::create_cnode(obj_paddr, rights::ALL)
                        }
                        // TCB
                        types::TCB => {
                            if obj_pages != 1 {
                                return errcode::INVALID_OBJ_TYPE;
                            }
                            let tcb_ptr = obj_vaddr.as_mut_ptr::<TCB>();
                            unsafe { tcb_ptr.write(TCB::new()) };
                            Capability::create_thread(obj_vaddr, rights::ALL)
                        }
                        // ipc::Endpoint
                        types::ENDPOINT => {
                            if obj_pages != 1 {
                                return errcode::INVALID_OBJ_TYPE;
                            }
                            let ep_ptr = obj_vaddr.as_mut_ptr::<ipc::Endpoint>();
                            unsafe { ep_ptr.write(ipc::Endpoint::new()) };
                            Capability::create_endpoint(obj_vaddr, rights::ALL)
                        }
                        // Frame
                        types::FRAME => Capability::create_frame(obj_paddr, obj_pages, rights::ALL),
                        // PageTable
                        types::PAGETABLE => {
                            if obj_pages != 1 {
                                return errcode::INVALID_OBJ_TYPE;
                            }
                            // 初始化页表 (清零已在上面完成)
                            Capability::create_pagetable(obj_paddr, 0, rights::ALL)
                        }
                        _ => return errcode::INVALID_OBJ_TYPE,
                    };

                    if !dest_cnode.insert(dest_slot_offset + i, &new_cap) {
                        return errcode::INVALID_SLOT;
                    }
                }

                // 更新 Untyped Cap 的 free_pages
                let new_free_pages = current_page_offset + n_objects * obj_pages;

                // 写回 CSpace
                if let Some((_, slot_paddr)) = tcb.cap_lookup_slot(cptr) {
                    let slot_ptr = slot_paddr.as_mut::<Slot>();
                    // 我们需要构造一个新的 Capability，或者直接修改现有的
                    // 由于 Capability 是 Copy，我们可以直接修改 slot_ptr.cap
                    if let CapType::Untyped { free_pages, .. } = &mut slot_ptr.cap.object {
                        *free_pages = new_free_pages;
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

fn invoke_irq_handler(cap: &Capability, _cptr: usize, method: usize) -> usize {
    let irq = match cap.object {
        CapType::IrqHandler { irq } => irq,
        _ => return errcode::INVALID_OBJ_TYPE,
    };

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
            let hartid = hart::getid();
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

fn invoke_console(_cap: &Capability, _cptr: usize, method: usize) -> usize {
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
            if let Some(_) = utcb.with_str(offset, len, |s| {
                crate::printk!("{}", s);
            }) {
                errcode::SUCCESS
            } else {
                errcode::INVALID_SLOT
            }
        }
        _ => errcode::INVALID_METHOD,
    }
}
