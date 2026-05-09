use super::super::asm;
use super::super::mem::PGSIZE;
use super::super::{cpu, trap};
use super::context::TrapFrame;
use super::vector::{kernel_vector, user_return, user_vector_table};
use crate::mem::TRAMPOLINE_VA;
use crate::proc::scheduler;
use crate::trap::handler::trap_kernel_handler;
use core::mem::transmute;

#[unsafe(no_mangle)]
pub extern "C" fn trap_user_handler() {
    unsafe { asm::write_vbar(kernel_vector as *const () as usize) };
    let tcb = unsafe { &mut *scheduler::current().expect("No current process in scheduler") };
    let ctx = tcb.get_tf();
    trap_kernel_handler(ctx);
    trap_user_return();
}

pub fn trap_user_return() {
    let tcb = unsafe { &mut *scheduler::current().expect("No current process in scheduler") };
    let kstack_top = tcb.get_kstack_top().as_usize();
    let user_mmu = tcb.mmu_register();
    let user_tf_va = tcb.trapframe_va;
    let ctx = tcb.get_tf();

    let tramp_base_va = TRAMPOLINE_VA;
    let user_vec_table_off = (user_vector_table as *const () as usize) & (PGSIZE - 1);
    let user_vec_table_addr = tramp_base_va + user_vec_table_off;

    unsafe { asm::write_vbar(user_vec_table_addr) };

    let ttbr1_el1 = trap::read_ttbr1();

    ctx.configure_kernel(
        ttbr1_el1,
        cpu::cpu_id(),
        kstack_top,
        trap_user_handler as *const () as usize,
    );

    let tf_kvaddr = ctx as *mut TrapFrame as usize;
    unsafe { asm::write_tpidr_el1(tf_kvaddr) };

    let user_ret_off = (user_return as *const () as usize) & (PGSIZE - 1);
    let user_ret_addr = tramp_base_va + user_ret_off;
    let user_return_fn: extern "C" fn(usize, usize) -> ! = unsafe { transmute(user_ret_addr) };

    user_return_fn(user_tf_va, user_mmu)
}
