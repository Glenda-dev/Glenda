use crate::trap::handler::trap_kernel_handler;
use core::arch::naked_asm;

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
        // 交换 a0 和 sscratch。
        // 现在 a0 = TrapFrame (用户态虚拟地址), sscratch = 用户 a0 数据
        "csrrw a0, sscratch, a0",
        // 保存通用寄存器 (除 a0 外)
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
        "sd t6, 280(a0)", // 正常保存 t6，不再从此处读取指针
        // 保存 a0 (用户数据)
        "csrr t0, sscratch",
        "sd t0, 112(a0)", // 把原来的 a0 存入 TF
        //------------------sd 过程 (end)-------------------------

        // 恢复内核上下文
        "ld sp, 8(a0)",  // sp = tf->kernel_sp (内核栈)
        "ld tp, 32(a0)", // tp = tf->kernel_hartid
        "ld t0, 16(a0)", // t0 = tf->kernel_trapvector (C函数入口)
        "ld t1, 0(a0)",  // t1 = tf->kernel_satp (内核页表)
        // 只有目标页表不同才更新 SATP，避免同地址空间线程切换时的冗余写入。
        "csrr t2, satp",
        "beq t1, t2, 2f",
        "csrw satp, t1",
        "2:",
        // 跳转处理函数
        // 注意：此时 a0 仍持有 TrapFrame 的 *用户态虚拟地址*。
        // 但由于页表已切换到内核，此地址在内核空间通过 a0 访问是无效的。
        // Rust 层的 handler 必须忽略此参数，转而从 TCB 获取内核映射的 TF 地址。
        "jr t0",
    );
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = "trampsec")]
pub unsafe extern "C" fn user_return(trapframe: u64, satp: u64) {
    naked_asm!(
        // a0 = TrapFrame Ptr (用户态 VA), a1 = 用户 SATP

        // 1. 只有目标页表不同才切回用户页表。
        "csrr t0, satp",
        "beq a1, t0, 2f",
        "csrw satp, a1",
        "2:",
        // 2. 将 TrapFrame 指针存入 sscratch，供下次 trap 使用
        // 此时我们使用用户页表，访问 a0 (TrapFrame VA) 是合法的
        "csrw sscratch, a0",
        // 3. 恢复寄存器
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
        "ld t6, 280(a0)", // 恢复正确的 t6 值
        // 4. 恢复 a0
        "ld a0, 112(a0)",
        // 5. 返回用户态
        "sret",
    );
}
