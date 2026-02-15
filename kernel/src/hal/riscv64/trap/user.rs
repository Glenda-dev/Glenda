use super::super::mem::PGSIZE;
use super::super::{asm, cpu};
use super::kernel_vector;
use super::vector::user_return;
use super::vector::user_vector;
use crate::mem::{TRAMPOLINE_VA, TRAPFRAME_VA};
use crate::proc::scheduler;
use crate::trap::handler::trap_kernel_handler;
use core::mem::transmute;

/// U-mode 陷阱处理函数
/// 在 kernel_vector 汇编代码中被调用
#[unsafe(no_mangle)]
pub extern "C" fn trap_user_handler() {
    let kernel_vec_addr = kernel_vector as *const () as usize;
    unsafe {
        asm::write_stvec(kernel_vec_addr);
    }
    let tcb = unsafe { &mut *scheduler::current().expect("No current process in scheduler") };
    let ctx = tcb.get_tf();
    ctx.set_epc(asm::read_sepc());
    trap_kernel_handler(ctx);
    trap_user_return();
}

/// 从用户态返回的函数
pub fn trap_user_return() {
    let tcb = unsafe { &mut *scheduler::current().expect("No current process in scheduler") };

    let kstack_top = tcb.get_kstack_top().as_usize();
    let user_satp = tcb.mmu_register() as u64;

    // 从 TCB 获取正确的 TrapFrame
    let ctx = tcb.get_tf();

    // 将 stvec 切换到用户态向量入口
    let tramp_base_va = TRAMPOLINE_VA;
    let user_vec_off = (user_vector as *const () as usize) & (PGSIZE - 1);
    let user_vec_addr = tramp_base_va + user_vec_off;
    unsafe {
        // 关闭 SIE (Bit 1)
        asm::sstatus_clear(1);
        asm::write_stvec(user_vec_addr);
        asm::write_sepc(ctx.get_epc());
        // 清除 SPP (Bit 8) 以返回 User Mode
        asm::sstatus_clear(8);
        // 开启 SPIE (Bit 5) 以便在 sret 后开启中断
        asm::sstatus_set(5);
    }

    // 跳回 S 态的处理入口：trap_user_handler
    // S 态页表
    // S 态 hartid
    // KSTACK(0) 顶部
    ctx.configure_kernel(
        asm::read_satp(),
        cpu::cpu_id(),
        kstack_top,
        trap_user_handler as *const () as usize,
    );

    // sscratch 指向 TrapFrame 的虚拟地址
    let user_tf_va = TRAPFRAME_VA;
    unsafe {
        asm::write_sscratch(user_tf_va);
    }

    // 通过 TRAMPOLINE 的高地址映射调用 user_return
    let user_ret_off = (user_return as *const () as usize) & (PGSIZE - 1);
    let user_ret_addr = tramp_base_va + user_ret_off;
    let user_return_fn: extern "C" fn(u64, u64) -> ! = unsafe { transmute(user_ret_addr) };
    user_return_fn(user_tf_va as u64, user_satp)
}
