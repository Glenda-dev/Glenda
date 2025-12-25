use super::syscall;
use super::vector;
use super::{TrapContext, TrapFrame};
use crate::mem::{PGSIZE, VA_MAX};
use crate::proc::scheduler;
use core::mem;
use riscv::register::{
    satp, sepc, sscratch, sstatus,
    stvec::{self, Stvec},
};

/// U-mode 陷阱处理函数
/// 在 kernel_vector 汇编代码中被调用
#[unsafe(no_mangle)]
pub extern "C" fn trap_user_handler(ctx: &mut TrapFrame) {
    let kernel_vec_addr = vector::kernel_vector as usize;
    unsafe {
        stvec::write(Stvec::new(kernel_vec_addr, stvec::TrapMode::Direct));
    }
    let epc = sepc::read();
    ctx.kernel_epc = epc;

    let mut kctx = TrapContext {
        ra: ctx.ra,
        sp: ctx.sp,
        gp: ctx.gp,
        tp: ctx.tp,
        t0: ctx.t0,
        t1: ctx.t1,
        t2: ctx.t2,
        s0: ctx.s0,
        s1: ctx.s1,
        a0: ctx.a0,
        a1: ctx.a1,
        a2: ctx.a2,
        a3: ctx.a3,
        a4: ctx.a4,
        a5: ctx.a5,
        a6: ctx.a6,
        a7: ctx.a7,
        s2: ctx.s2,
        s3: ctx.s3,
        s4: ctx.s4,
        s5: ctx.s5,
        s6: ctx.s6,
        s7: ctx.s7,
        s8: ctx.s8,
        s9: ctx.s9,
        s10: ctx.s10,
        s11: ctx.s11,
        t3: ctx.t3,
        t4: ctx.t4,
        t5: ctx.t5,
        t6: ctx.t6,
    };
    super::kernel::trap_kernel_handler(&mut kctx);

    ctx.ra = kctx.ra;
    ctx.sp = kctx.sp;
    ctx.gp = kctx.gp;
    ctx.tp = kctx.tp;
    ctx.t0 = kctx.t0;
    ctx.t1 = kctx.t1;
    ctx.t2 = kctx.t2;
    ctx.s0 = kctx.s0;
    ctx.s1 = kctx.s1;
    ctx.a0 = kctx.a0;
    ctx.a1 = kctx.a1;
    ctx.a2 = kctx.a2;
    ctx.a3 = kctx.a3;
    ctx.a4 = kctx.a4;
    ctx.a5 = kctx.a5;
    ctx.a6 = kctx.a6;
    ctx.a7 = kctx.a7;
    ctx.s2 = kctx.s2;
    ctx.s3 = kctx.s3;
    ctx.s4 = kctx.s4;
    ctx.s5 = kctx.s5;
    ctx.s6 = kctx.s6;
    ctx.s7 = kctx.s7;
    ctx.s8 = kctx.s8;
    ctx.s9 = kctx.s9;
    ctx.s10 = kctx.s10;
    ctx.s11 = kctx.s11;
    ctx.t3 = kctx.t3;
    ctx.t4 = kctx.t4;
    ctx.t5 = kctx.t5;
    ctx.t6 = kctx.t6;

    ctx.kernel_epc = sepc::read();
    trap_user_return(ctx);
}

#[unsafe(no_mangle)]
pub extern "C" fn trap_user_return(_ctx: &mut TrapFrame) {
    // TODO: Refactor this
    // 直接通过当前 hart 的进程状态获取 TrapFrame 的指针
    let tcb = unsafe { &*scheduler::current().expect("No current process in trap_user_return") };
    let ctx: &mut TrapFrame = &mut *tcb.get_trapframe().expect("No TrapFrame found");
    unsafe {
        sstatus::clear_sie();
    }
    // 将 stvec 切换到用户态向量入口
    let tramp_base_va = VA_MAX - PGSIZE;
    let user_vec_off = (vector::user_vector as usize) - (vector::trampoline as usize);
    let user_vec_addr = tramp_base_va + user_vec_off;
    unsafe {
        stvec::write(Stvec::new(user_vec_addr, stvec::TrapMode::Direct));
    }

    unsafe {
        sepc::write(ctx.kernel_epc);
    }

    unsafe {
        sstatus::set_spp(sstatus::SPP::User);
    }

    ctx.t6 = ctx as *mut TrapFrame as usize;

    // 跳回 S 态的处理入口：trap_user_handler
    ctx.kernel_trapvector = trap_user_handler as usize;
    // S 态页表
    ctx.kernel_satp = satp::read().bits();
    // S 态 hartid
    ctx.kernel_hartid = crate::hart::get().id;
    // KSTACK(0) 顶部
    // vm::map_kstack0();
    ctx.kernel_sp = tcb.kstack.as_ref().unwrap().top().as_usize();

    // sscratch 指向 TrapFrame 的虚拟地址
    let user_tf_va = tcb.get_trapframe_va().expect("No TrapFrame VA found");
    unsafe {
        sscratch::write(user_tf_va.as_usize());
    }

    let user_satp = tcb.get_satp().expect("Failed to get satp").as_usize() as u64;

    // 通过 TRAMPOLINE 的高地址映射调用 user_return
    let user_ret_off = (vector::user_return as usize) - (vector::trampoline as usize);
    let user_ret_addr = tramp_base_va + user_ret_off;
    let user_return_fn: extern "C" fn(u64, u64) -> ! = unsafe { mem::transmute(user_ret_addr) };
    user_return_fn(user_tf_va.as_usize() as u64, user_satp)
}

pub fn syscall_handler(ctx: &mut TrapContext) {
    let ret = syscall::dispatch(ctx);
    ctx.a0 = ret;
}
