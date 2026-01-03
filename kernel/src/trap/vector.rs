use super::kernel::trap_kernel_handler;
use core::arch::naked_asm;
use riscv::register::stvec::{self, Stvec};

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_vector() {
    naked_asm!(
        "addi sp, sp, -256",
        "sd ra, 0(sp)",
        "sd sp, 8(sp)",
        "sd gp, 16(sp)",
        "sd tp, 24(sp)",
        "sd t0, 32(sp)",
        "sd t1, 40(sp)",
        "sd t2, 48(sp)",
        "sd s0, 56(sp)",
        "sd s1, 64(sp)",
        "sd a0, 72(sp)",
        "sd a1, 80(sp)",
        "sd a2, 88(sp)",
        "sd a3, 96(sp)",
        "sd a4, 104(sp)",
        "sd a5, 112(sp)",
        "sd a6, 120(sp)",
        "sd a7, 128(sp)",
        "sd s2, 136(sp)",
        "sd s3, 144(sp)",
        "sd s4, 152(sp)",
        "sd s5, 160(sp)",
        "sd s6, 168(sp)",
        "sd s7, 176(sp)",
        "sd s8, 184(sp)",
        "sd s9, 192(sp)",
        "sd s10, 200(sp)",
        "sd s11, 208(sp)",
        "sd t3, 216(sp)",
        "sd t4, 224(sp)",
        "sd t5, 232(sp)",
        "sd t6, 240(sp)",

        "mv a0, sp",
        "call {handler}",

        "ld ra, 0(sp)",
        "ld gp, 16(sp)",
        "ld t0, 32(sp)",
        "ld t1, 40(sp)",
        "ld t2, 48(sp)",
        "ld s0, 56(sp)",
        "ld s1, 64(sp)",
        "ld a0, 72(sp)",
        "ld a1, 80(sp)",
        "ld a2, 88(sp)",
        "ld a3, 96(sp)",
        "ld a4, 104(sp)",
        "ld a5, 112(sp)",
        "ld a6, 120(sp)",
        "ld a7, 128(sp)",
        "ld s2, 136(sp)",
        "ld s3, 144(sp)",
        "ld s4, 152(sp)",
        "ld s5, 160(sp)",
        "ld s6, 168(sp)",
        "ld s7, 176(sp)",
        "ld s8, 184(sp)",
        "ld s9, 192(sp)",
        "ld s10, 200(sp)",
        "ld s11, 208(sp)",
        "ld t3, 216(sp)",
        "ld t4, 224(sp)",
        "ld t5, 232(sp)",
        "ld t6, 240(sp)",
        "addi sp, sp, 256",
        "sret",
        handler = sym trap_kernel_handler,
    );
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = "trampsec")]
pub unsafe extern "C" fn user_vector() {
    naked_asm!(
        // ------------------sd 过程 (begin)-----------------------
        // sscratch寄存器存放了p->trapframe
        "csrrw a0, sscratch, a0",
        // 把 TrapFrame 中由内核预先写入的指针给 a3
        // See: trap_user_handler
        "ld a3, 280(a0)",
        // 保存通用寄存器到trapframe
        "sd ra, 40(a0)",
        "sd sp, 48(a0)",
        "sd gp, 56(a0)",
        "sd tp, 64(a0)",
        "sd t0, 72(a0)",
        "sd t1, 80(a0)",
        "sd t2, 88(a0)",
        "sd s0, 96(a0)",
        "sd s1, 104(a0)",
        "sd a1, 120(a0)",
        "sd a2, 128(a0)",
        "sd a3, 136(a0)",
        "sd a4, 144(a0)",
        "sd a5, 152(a0)",
        "sd a6, 160(a0)",
        "sd a7, 168(a0)",
        "sd s2, 176(a0)",
        "sd s3, 184(a0)",
        "sd s4, 192(a0)",
        "sd s5, 200(a0)",
        "sd s6, 208(a0)",
        "sd s7, 216(a0)",
        "sd s8, 224(a0)",
        "sd s9, 232(a0)",
        "sd s10, 240(a0)",
        "sd s11, 248(a0)",
        "sd t3, 256(a0)",
        "sd t4, 264(a0)",
        "sd t5, 272(a0)",
        "sd t6, 280(a0)",
        // 保存 a0 到 p->trapframe
        "csrr t0, sscratch",
        "sd t0, 112(a0)",
        //------------------sd 过程 (end)-------------------------
        /*
        恢复之前保存的内核执行环境
        1. 内核栈指针
        2. hartid信息
        3. 内核页表
        之后跳转到 trap_user_handler
        */
        "ld sp, 8(a0)",  // sp = tf->kernel_sp
        "ld tp, 32(a0)", // tp = tf->kernel_hartid
        "ld t0, 16(a0)", // t0 = tf->kernel_trapvector
        "ld t1, 0(a0)",  // t1 = tf->kernel_satp
        "csrw satp, t1",
        "sfence.vma zero, zero",
        "mv a0, a3",
        "jr t0",
    );
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = "trampsec")]
pub unsafe extern "C" fn user_return(trapframe: u64, satp: u64) {
    naked_asm!(
        "csrw satp, a1",
        "sfence.vma zero, zero",
        "csrw sscratch, a0",
        "ld ra, 40(a0)",
        "ld sp, 48(a0)",
        "ld gp, 56(a0)",
        "ld tp, 64(a0)",
        "ld t0, 72(a0)",
        "ld t1, 80(a0)",
        "ld t2, 88(a0)",
        "ld s0, 96(a0)",
        "ld s1, 104(a0)",
        "ld a1, 120(a0)",
        "ld a2, 128(a0)",
        "ld a3, 136(a0)",
        "ld a4, 144(a0)",
        "ld a5, 152(a0)",
        "ld a6, 160(a0)",
        "ld a7, 168(a0)",
        "ld s2, 176(a0)",
        "ld s3, 184(a0)",
        "ld s4, 192(a0)",
        "ld s5, 200(a0)",
        "ld s6, 208(a0)",
        "ld s7, 216(a0)",
        "ld s8, 224(a0)",
        "ld s9, 232(a0)",
        "ld s10, 240(a0)",
        "ld s11, 248(a0)",
        "ld t3, 256(a0)",
        "ld t4, 264(a0)",
        "ld t5, 272(a0)",
        "ld t6, 280(a0)",
        "ld a0, 112(a0)",
        "sret",
    );
}

pub fn init() {
    let kernel_vec_addr = kernel_vector as usize;
    let vec = Stvec::new(kernel_vec_addr, stvec::TrapMode::Direct);
    unsafe {
        // set supervisor trap vector address
        stvec::write(vec);
    }
}
