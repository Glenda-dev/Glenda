use super::super::method::*;
use crate::cap::{CapPtr, CapType, Capability, Rights};
use crate::hal;
use crate::proc::TCB;
use crate::proc::scheduler;
use crate::trap::syscall::errcode;

pub fn invoke_tcb(cap: &mut Capability, method: usize) -> usize {
    let tcb_ptr = if cap.cap_type() == CapType::TCB {
        cap.obj_ptr()
    } else {
        log!("TCB::invoke failed: invalid obj type {:?}", cap.cap_type());
        return errcode::INVALID_OBJ_TYPE;
    };

    let tcb = unsafe { tcb_ptr.as_mut::<TCB>() };
    let current_tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match current_tcb.get_utcb() {
        Some(u) => u,
        None => {
            log!("TCB::invoke failed: no UTCB");
            return errcode::MAPPING_FAILED;
        }
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
            if cspace_cap.is_none()
                && vspace_cap.is_none()
                && utcb_cap.is_none()
                && tf_cap.is_none()
                && kstack_cap.is_none()
            {
                log!(
                    "TCB::Configure failed: missing caps {:?} {:?} {:?} {:?} {:?}",
                    cspace_cptr,
                    vspace_cptr,
                    utcb_cptr,
                    tf_cptr,
                    kstack_cptr
                );
                return errcode::INVALID_CAP;
            }

            // 简化的配置逻辑
            tcb.configure(
                &cspace_cap.unwrap(),
                &vspace_cap.unwrap(),
                &utcb_cap.unwrap(),
                &tf_cap.unwrap(),
                &kstack_cap.unwrap(),
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
        tcbmethod::SET_ENTRYPOINT => {
            // SetRegisters: (entry, sp)
            let entry = utcb.mrs_regs[0];
            let sp = utcb.mrs_regs[1];
            let tp = utcb.mrs_regs[2];
            tcb.set_entrypoint(entry, sp, tp);
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
                    log!("TCB::SetFaultHandler failed: invalid obj type {:?}", ep_cap.cap_type());
                    errcode::INVALID_OBJ_TYPE
                }
            } else {
                log!("TCB::SetFaultHandler failed: cap not found {:?}", ep_cptr);
                errcode::INVALID_CAP
            }
        }
        tcbmethod::SET_AFFINITY => {
            // SetAffinity: (cpu_id)
            let cpu_id = utcb.mrs_regs[0];
            tcb.set_affinity(cpu_id);
            errcode::SUCCESS
        }
        tcbmethod::SET_REGISTERS => {
            // SetRegisters: (a0, a1, a2, a3, a4, a5, a6)
            tcb.set_registers(&utcb.mrs_regs);
            errcode::SUCCESS
        }
        tcbmethod::RESUME => {
            if !cap.has_rights(Rights::EXECUTE) {
                log!("TCB::Resume failed: permission denied");
                return errcode::PERMISSION_DENIED;
            }
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
        _ => {
            log!("TCB::invoke failed: invalid method {}", method);
            errcode::INVALID_METHOD
        }
    }
}
