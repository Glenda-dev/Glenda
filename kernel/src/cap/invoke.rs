use super::method::*;
use crate::cap::{Badge, CNode, CapPtr, CapType, Capability, Rights};
use crate::hal;
use crate::hal::mem::PGSIZE;
use crate::ipc;
use crate::irq;
use crate::mem::PageTable;
use crate::mem::Perms;
use crate::mem::{UntypedRegion, VirtAddr};
use crate::proc::{TCB, scheduler};
use crate::trap::syscall::errcode;

pub fn dispatch(cap: &mut Capability, method: usize) -> usize {
    // 4. 根据对象类型分发
    match cap.cap_type() {
        CapType::Endpoint => invoke_ipc(cap, method),
        CapType::TCB => invoke_tcb(cap, method),
        CapType::PageTable => invoke_pagetable(cap, method),
        CapType::CNode => invoke_cnode(cap, method),
        CapType::Untyped => invoke_untyped(cap, method),
        CapType::IrqHandler => invoke_irq_handler(cap, method),
        CapType::VSpace => invoke_vspace(cap, method),
        CapType::Reply => invoke_reply(cap, method),
        CapType::Console => invoke_console(cap, method),
        _ => errcode::INVALID_OBJ_TYPE,
    }
}

// --- IPC ipc::Endpoint Methods ---

fn invoke_ipc(cap: &mut Capability, method: usize) -> usize {
    let ep_ptr = if cap.cap_type() == CapType::Endpoint {
        cap.obj_ptr()
    } else {
        return errcode::INVALID_OBJ_TYPE;
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
            if !cap.has_rights(Rights::SEND) {
                return errcode::PERMISSION_DENIED;
            }
            let tag = utcb.msg_tag;
            // 通过 invoke 发送时，暂时不支持传递能力，或者从 UTCB 中提取
            let mut cap_to_send = None;
            if tag.has_cap() {
                if let Some(cap) = tcb.cap_lookup(utcb.cap_transfer) {
                    if cap.has_rights(Rights::GRANT) {
                        cap_to_send = Some(cap);
                    }
                }
            }
            ipc::send(tcb, ep, badge, cap_to_send);
            errcode::SUCCESS
        }
        ipcmethod::RECV => {
            if !cap.has_rights(Rights::RECV) {
                return errcode::PERMISSION_DENIED;
            }
            ipc::recv(tcb, ep);
            errcode::SUCCESS
        }
        ipcmethod::CALL => {
            if !cap.has_rights(Rights::CALL) {
                return errcode::PERMISSION_DENIED;
            }
            let tag = utcb.msg_tag;
            let mut cap_to_send = None;
            if tag.has_cap() {
                if let Some(cap) = tcb.cap_lookup(utcb.cap_transfer) {
                    if cap.has_rights(Rights::GRANT) {
                        cap_to_send = Some(cap);
                    }
                }
            }
            ipc::call(tcb, ep, badge, cap_to_send);
            errcode::SUCCESS
        }
        ipcmethod::NOTIFY => {
            if !cap.has_rights(Rights::SEND) {
                return errcode::PERMISSION_DENIED;
            }
            ipc::notify(ep, badge);
            errcode::SUCCESS
        }
        _ => errcode::INVALID_METHOD,
    }
}

fn invoke_reply(cap: &mut Capability, method: usize) -> usize {
    let tcb_ptr = if cap.cap_type() == CapType::Reply {
        cap.obj_ptr()
    } else {
        return errcode::INVALID_OBJ_TYPE;
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

fn invoke_tcb(cap: &mut Capability, method: usize) -> usize {
    let tcb_ptr = if cap.cap_type() == CapType::TCB {
        cap.obj_ptr()
    } else {
        return errcode::INVALID_OBJ_TYPE;
    };

    let tcb = tcb_ptr.as_mut::<TCB>();
    let current_tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match current_tcb.get_utcb() {
        Some(u) => u,
        None => return errcode::MAPPING_FAILED,
    };

    match method {
        tcbmethod::CONFIGURE => {
            // args: [cspace_ vspace_ utcb_ tf_ kstack_cptr]
            let cspace_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let vspace_cptr = CapPtr::from(utcb.mrs_regs[1]);
            let utcb_cptr = CapPtr::from(utcb.mrs_regs[2]);
            let tf_cptr = CapPtr::from(utcb.mrs_regs[3]);
            let kstack_cptr = CapPtr::from(utcb.mrs_regs[4]);
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
            let entry = utcb.mrs_regs[1];
            let sp = utcb.mrs_regs[2];
            tcb.set_registers(entry, sp);
            errcode::SUCCESS
        }
        tcbmethod::SET_FAULT_HANDLER => {
            // SetFaultHandler: (ep_cptr)
            let ep_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let native = utcb.mrs_regs[1] != 0;
            if let Some(ep_cap) = current_tcb.cap_lookup(ep_cptr) {
                // Only accept ipc::Endpoint caps
                if ep_cap.cap_type() == CapType::Endpoint {
                    tcb.set_fault_handler(ep_cap, native);
                    errcode::SUCCESS
                } else {
                    errcode::INVALID_OBJ_TYPE
                }
            } else {
                errcode::INVALID_CAP
            }
        }
        tcbmethod::SET_AFFINITY => {
            // SetAffinity: (cpu_id)
            let cpu_id = utcb.mrs_regs[0];
            tcb.set_affinity(cpu_id);
            errcode::SUCCESS
        }
        tcbmethod::RESUME => {
            // Resume
            tcb.resume();
            // 将线程加入调度队列
            scheduler::add_thread(tcb);
            // 2. 抢占检查
            // 如果目标核心是当前核心，且优先级高于当前线程，则触发重新调度
            let current_cpu = hal::cpu::cpu_id();
            let target_cpu =
                if tcb.affinity < hal::cpu::MAX_CPUS { tcb.affinity } else { current_cpu };

            if target_cpu == current_cpu {
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

fn invoke_pagetable(cap: &mut Capability, method: usize) -> usize {
    let paddr = if cap.cap_type() == CapType::PageTable {
        cap.paddr()
    } else {
        return errcode::INVALID_OBJ_TYPE;
    };

    // PageTable 需要物理地址转虚拟地址才能操作
    let pt_ptr = hal::mem::phys_to_virt(paddr);
    let pt = pt_ptr.as_mut::<PageTable>();
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => return errcode::MAPPING_FAILED,
    };

    match method {
        pagetablemethod::MAP_TABLE => {
            // MapTable: (table_cap, vaddr, level)
            let table_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let vaddr = VirtAddr::from(utcb.mrs_regs[1]);
            let level = utcb.mrs_regs[2];

            let table_cap = match tcb.cap_lookup(table_cptr) {
                Some(c) => c,
                None => return errcode::INVALID_CAP,
            };

            let table_paddr = if table_cap.cap_type() == CapType::PageTable {
                table_cap.paddr()
            } else {
                return errcode::INVALID_OBJ_TYPE;
            };

            match pt.map_table(vaddr, table_paddr, level) {
                Ok(()) => errcode::SUCCESS,
                Err(_) => errcode::MAPPING_FAILED,
            }
        }
        _ => errcode::INVALID_METHOD,
    }
}

// --- CNode methods ---

fn invoke_cnode(cap: &mut Capability, method: usize) -> usize {
    let vaddr = if cap.cap_type() == CapType::CNode {
        cap.obj_ptr()
    } else {
        return errcode::INVALID_OBJ_TYPE;
    };

    let cnode = vaddr.as_mut::<CNode>();
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => return errcode::MAPPING_FAILED,
    };

    match method {
        cnodemethod::MINT => {
            // Mint: (src_cptr, dest_slot, badge, rights)
            let src_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let dest_slot = utcb.mrs_regs[1];
            let new_badge = Badge::from(utcb.mrs_regs[2]);
            let req_rights = utcb.mrs_regs[3] as u8;

            if let Some((src_cap, src_slot_addr)) = tcb.cap_lookup_slot(src_cptr) {
                // 1. 权限收缩：新权限必须是源权限的子集
                let final_rights =
                    Rights::from_bits_truncate(req_rights).intersection(src_cap.rights());

                // 2. Badge 检查：
                // - 如果源 Cap 已有 Badge，则不能再次设置新 Badge (seL4 语义)
                // - 新 Cap 必须继承源 Cap 的 Badge (如果存在)
                let src_badge = src_cap.get_badge();
                if !src_badge.is_null() && !new_badge.is_null() {
                    return errcode::INVALID_CAP; // Cannot re-badge an already badged cap
                }
                let final_badge = if !src_badge.is_null() { src_badge } else { new_badge };

                let new_cap = src_cap.mint(final_badge, final_rights);
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
            // Copy: (src_ dest_slot, rights)
            let src_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let dest_slot = utcb.mrs_regs[1];
            let rights = utcb.mrs_regs[2] as u8;

            if let Some((src_cap, src_slot_addr)) = tcb.cap_lookup_slot(src_cptr) {
                let new_cap = src_cap.mint(Badge::null(), Rights::from_bits_truncate(rights));
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
            if slot_addr != VirtAddr::null() {
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
            if slot_addr != VirtAddr::null() {
                cnode.revoke(slot);
                errcode::SUCCESS
            } else {
                errcode::INVALID_SLOT
            }
        }
        cnodemethod::DEBUG_PRINT => {
            cnode.debug_print();
            errcode::SUCCESS
        }
        _ => errcode::INVALID_METHOD,
    }
}

fn invoke_untyped(cap: &mut Capability, method: usize) -> usize {
    let mut untyped = match UntypedRegion::from_cap(cap) {
        Some(u) => u,
        None => return errcode::INVALID_OBJ_TYPE,
    };

    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => return errcode::MAPPING_FAILED,
    };

    match method {
        untypedmethod::RETYPE => {
            // Retype: (type, flags, n_objects, dest_cnode, dest_slot_offset, dirty)
            let obj_type = utcb.mrs_regs[0];

            let flags = utcb.mrs_regs[1];
            let n_objects = utcb.mrs_regs[2];
            let dest_cnode_cptr = CapPtr::from(utcb.mrs_regs[3]);
            let dest_slot_offset = utcb.mrs_regs[4];
            let dirty = utcb.mrs_regs[5];

            let dest_cnode_cap = match tcb.cap_lookup(dest_cnode_cptr) {
                Some(c) => c,
                None => return errcode::INVALID_CAP,
            };

            if dest_cnode_cap.cap_type() == CapType::CNode {
                let cn_vaddr = dest_cnode_cap.obj_ptr();
                let dest_cnode = cn_vaddr.as_mut::<CNode>();
                untyped.retype(
                    CapType::from(obj_type),
                    flags,
                    n_objects,
                    dest_cnode,
                    dest_slot_offset,
                    dirty,
                )
            } else {
                errcode::INVALID_OBJ_TYPE
            }
        }
        _ => errcode::INVALID_METHOD,
    }
}

fn invoke_irq_handler(cap: &mut Capability, method: usize) -> usize {
    let irq = if cap.cap_type() == CapType::IrqHandler {
        cap.value()
    } else {
        return errcode::INVALID_OBJ_TYPE;
    };

    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => return errcode::MAPPING_FAILED,
    };

    match method {
        irqmethod::SET_NOTIFICATION => {
            // SetNotification: args[0] = ep_cptr
            let ep_cptr = CapPtr::from(utcb.mrs_regs[0]);

            if let Some(ep_cap) = tcb.cap_lookup(ep_cptr) {
                // Only accept ipc::Endpoint caps
                if ep_cap.cap_type() == CapType::Endpoint {
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
            let cpuid = hal::cpu::cpu_id();
            irq::ack_irq(cpuid, irq);
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
            hal::irq::set_priority(irq as u32, priority as u8);
            errcode::SUCCESS
        }
        _ => errcode::INVALID_METHOD,
    }
}

fn invoke_console(_cap: &mut Capability, method: usize) -> usize {
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
        consolemethod::GET_CHAR => {
            let c = hal::console::read() as usize;
            utcb.mrs_regs[0] = c;
            errcode::SUCCESS
        }
        _ => errcode::INVALID_METHOD,
    }
}

fn invoke_vspace(cap: &mut Capability, method: usize) -> usize {
    let paddr = if cap.cap_type() == CapType::VSpace {
        cap.paddr()
    } else {
        return errcode::INVALID_OBJ_TYPE;
    };

    // PageTable 需要物理地址转虚拟地址才能操作
    let pt_ptr = hal::mem::phys_to_virt(paddr);
    let pt = pt_ptr.as_mut::<PageTable>();
    let tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match tcb.get_utcb() {
        Some(u) => u,
        None => return errcode::MAPPING_FAILED,
    };

    match method {
        vspacemethod::MAP => {
            // Map: (frame_cap, vaddr, flags)
            let frame_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let vaddr = VirtAddr::from(utcb.mrs_regs[1]);
            let flags = Perms::from_bits_truncate(utcb.mrs_regs[2]) | Perms::USER;

            let frame_cap = match tcb.cap_lookup(frame_cptr) {
                Some(c) => c,
                None => return errcode::INVALID_CAP,
            };

            let frame_paddr = if frame_cap.cap_type() == CapType::Frame {
                frame_cap.paddr()
            } else {
                return errcode::INVALID_OBJ_TYPE;
            };

            // 执行映射
            match pt.map(vaddr, frame_paddr, PGSIZE, flags) {
                Ok(()) => errcode::SUCCESS,
                Err(_) => errcode::MAPPING_FAILED,
            }
        }
        vspacemethod::MAP_TABLE => {
            // MapTable: (table_cap, vaddr, level)
            let table_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let vaddr = VirtAddr::from(utcb.mrs_regs[1]);
            let level = utcb.mrs_regs[2];

            let table_cap = match tcb.cap_lookup(table_cptr) {
                Some(c) => c,
                None => return errcode::INVALID_CAP,
            };

            let table_paddr = if table_cap.cap_type() == CapType::VSpace {
                table_cap.paddr()
            } else {
                return errcode::INVALID_OBJ_TYPE;
            };

            match pt.map_table(vaddr, table_paddr, level) {
                Ok(()) => errcode::SUCCESS,
                Err(_) => errcode::MAPPING_FAILED,
            }
        }
        vspacemethod::UNMAP => {
            // Unmap: (vaddr, size)
            let vaddr = VirtAddr::from(utcb.mrs_regs[0]);
            let size = utcb.mrs_regs[1];
            match pt.unmap(vaddr, size) {
                Ok(()) => errcode::SUCCESS,
                Err(_) => errcode::MAPPING_FAILED,
            }
        }
        vspacemethod::SETUP => match hal::mem::pt_setup(pt) {
            Ok(()) => errcode::SUCCESS,
            Err(_) => errcode::MAPPING_FAILED,
        },
        vspacemethod::DEBUG_PRINT => {
            pt.debug_print();
            errcode::SUCCESS
        }
        _ => errcode::INVALID_METHOD,
    }
}
