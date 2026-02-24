use super::super::method::*;
use crate::cap::{CapPtr, CapType, Capability, Rights};
use crate::error::Error;
use crate::hal;
use crate::proc::TCB;
use crate::proc::scheduler;

pub fn invoke_tcb(cap: &mut Capability, method: usize) -> Result<(), Error> {
    let tcb_ptr = if cap.cap_type() == CapType::TCB {
        cap.obj_ptr()
    } else {
        error!("TCB::invoke failed: invalid obj type {:?}", cap.cap_type());
        return Err(Error::InvalidType);
    };

    let tcb = unsafe { tcb_ptr.as_mut::<TCB>() };
    let current_tcb = unsafe { &mut *scheduler::current().expect("No current TCB") };
    let utcb = match current_tcb.get_utcb() {
        Some(u) => u,
        None => {
            error!("TCB::invoke failed: no UTCB");
            return Err(Error::MappingFailed);
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
                || vspace_cap.is_none()
                || utcb_cap.is_none()
                || tf_cap.is_none()
                || kstack_cap.is_none()
            {
                error!(
                    "TCB::Configure failed: missing caps {} {} {} {} {}",
                    cspace_cptr, vspace_cptr, utcb_cptr, tf_cptr, kstack_cptr
                );
                return Err(Error::InvalidCapability);
            }

            // 简化的配置逻辑
            tcb.configure(
                &cspace_cap.unwrap(),
                &vspace_cap.unwrap(),
                &utcb_cap.unwrap(),
                &tf_cap.unwrap(),
                &kstack_cap.unwrap(),
            );
            Ok(())
        }
        tcbmethod::SET_PRIORITY => {
            // SetPriority: (prio, incr)
            let mut prio = utcb.mrs_regs[0] as u8;
            let incr = utcb.mrs_regs[1] as i8;
            if prio == 0 {
                prio = tcb.priority; // 0 表示不修改优先级
            }
            prio = prio.saturating_add_signed(incr);
            // 如果修改了优先级，可能需要触发重新调度
            if prio != tcb.priority {
                tcb.set_priority(prio);
                scheduler::reschedule();
            }
            Ok(())
        }
        tcbmethod::SET_ENTRYPOINT => {
            // SetRegisters: (entry, sp)
            let entry = utcb.mrs_regs[0];
            let sp = utcb.mrs_regs[1];
            let tp = utcb.mrs_regs[2];
            tcb.set_entrypoint(entry, sp, tp);
            Ok(())
        }
        tcbmethod::SET_ADDRESS => {
            // SetAddress: (utcb_va, trapframe_va)
            let utcb_va = utcb.mrs_regs[0];
            let trapframe_va = utcb.mrs_regs[1];
            tcb.set_address(utcb_va, trapframe_va);
            Ok(())
        }
        tcbmethod::SET_FAULT_HANDLER => {
            // SetFaultHandler: (ep_cptr)
            let ep_cptr = CapPtr::from(utcb.mrs_regs[0]);
            let native = utcb.mrs_regs[1] != 0;
            if let Some(ep_cap) = current_tcb.cap_lookup(ep_cptr) {
                // Only accept ipc::Endpoint caps
                if ep_cap.cap_type() == CapType::Endpoint {
                    tcb.set_fault_handler(ep_cap, native);
                    Ok(())
                } else {
                    error!("TCB::SetFaultHandler failed: invalid obj type {:?}", ep_cap.cap_type());
                    Err(Error::InvalidType)
                }
            } else {
                error!("TCB::SetFaultHandler failed: cap not found {:?}", ep_cptr);
                Err(Error::InvalidCapability)
            }
        }
        tcbmethod::SET_AFFINITY => {
            // SetAffinity: (cpu_id)
            let cpu_id = utcb.mrs_regs[0];
            tcb.set_affinity(cpu_id);
            Ok(())
        }
        tcbmethod::SET_REGISTERS => {
            // SetRegisters: (a0, a1, a2, a3, a4, a5, a6)
            tcb.set_registers(&utcb.mrs_regs);
            Ok(())
        }
        tcbmethod::RESUME => {
            if !cap.has_rights(Rights::EXECUTE) {
                error!("TCB::Resume failed: permission denied");
                return Err(Error::PermissionDenied);
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
            Ok(())
        }
        tcbmethod::SUSPEND => {
            // Suspend
            tcb.suspend();
            if tcb as *const TCB == current_tcb as *const TCB {
                scheduler::block_current_thread();
            }
            Ok(())
        }
        _ => {
            error!("TCB::invoke failed: invalid method {}", method);
            Err(Error::InvalidMethod)
        }
    }
}
