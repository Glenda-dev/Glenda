pub mod syscall;

use super::super::{TrapContext, TrapFrame};
use super::vector;
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
pub extern "C" fn trap_user_return(ctx: &mut TrapFrame) {
    // 将 stvec 切换到用户态向量入口
    let user_vec_addr = vector::user_vector as usize;
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
    ctx.kernel_hartid = crate::hart::getid();
    // KSTACK(0) 顶部
    crate::mem::vm::vm_map_kstack0();
    ctx.kernel_sp = crate::mem::vm::kstack_top(0);

    // sscratch 指向 TrapFrame 的虚拟地址
    let user_tf_va = ctx.a0;
    unsafe {
        sscratch::write(user_tf_va);
    }

    let user_satp = crate::proc::current_user_satp().unwrap_or_else(|| satp::read().bits()) as u64;
    unsafe { vector::user_return(user_tf_va as u64, user_satp) }
}
