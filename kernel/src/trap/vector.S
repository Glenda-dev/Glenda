.section .text
.align 2

.globl kernel_vector
kernel_vector:
    // 为栈指针减少 256 字节以容纳 32 个 64 位寄存器
    addi sp, sp, -256
    // 保存所有通用寄存器到栈上
    sd ra, 0(sp)
    sd sp, 8(sp)
    sd gp, 16(sp)
    sd tp, 24(sp)
    sd t0, 32(sp)
    sd t1, 40(sp)
    sd t2, 48(sp)
    sd s0, 56(sp)
    sd s1, 64(sp)
    sd a0, 72(sp)
    sd a1, 80(sp)
    sd a2, 88(sp)
    sd a3, 96(sp)
    sd a4, 104(sp)
    sd a5, 112(sp)
    sd a6, 120(sp)
    sd a7, 128(sp)
    sd s2, 136(sp)
    sd s3, 144(sp)
    sd s4, 152(sp)
    sd s5, 160(sp)
    sd s6, 168(sp)
    sd s7, 176(sp)
    sd s8, 184(sp)
    sd s9, 192(sp)
    sd s10, 200(sp)
    sd s11, 208(sp)
    sd t3, 216(sp)
    sd t4, 224(sp)
    sd t5, 232(sp)
    sd t6, 240(sp)
    // 调用 Rust 陷阱处理函数，传递栈指针(陷阱上下文)
    mv a0, sp
    call trap_kernel_handler
    // 从栈上恢复所有通用寄存器
    ld ra, 0(sp)
    ld gp, 16(sp)
    // tp 寄存器的值可能不可靠，跳过恢复
    ld t0, 32(sp)
    ld t1, 40(sp)
    ld t2, 48(sp)
    ld s0, 56(sp)
    ld s1, 64(sp)
    ld a0, 72(sp)
    ld a1, 80(sp)
    ld a2, 88(sp)
    ld a3, 96(sp)
    ld a4, 104(sp)
    ld a5, 112(sp)
    ld a6, 120(sp)
    ld a7, 128(sp)
    ld s2, 136(sp)
    ld s3, 144(sp)
    ld s4, 152(sp)
    ld s5, 160(sp)
    ld s6, 168(sp)
    ld s7, 176(sp)
    ld s8, 184(sp)
    ld s9, 192(sp)
    ld s10, 200(sp)
    ld s11, 208(sp)
    ld t3, 216(sp)
    ld t4, 224(sp)
    ld t5, 232(sp)
    ld t6, 240(sp)
    // 恢复栈指针并返回
    addi sp, sp, 256
    sret

.align 2
.globl timer_vector_base
timer_vector_base:
    // Vectored trap table (only M-mode timer used)
    j .                // 0: U-mode software interrupt
    j .                // 1: S-mode software interrupt
    j .                // 2: reserved
    j .                // 3: M-mode software interrupt
    j .                // 4: U-mode timer interrupt
    j .                // 5: S-mode timer interrupt
    j .                // 6: reserved
    j timer_vector     // 7: M-mode timer interrupt
    j .                // 8: U-mode external interrupt
    j .                // 9: S-mode external interrupt
    j .                // 10: reserved
    j .                // 11: M-mode external interrupt
    j .                // 12: reserved
    j .                // 13: reserved
    j .                // 14: reserved
    j .                // 15: reserved

.align 2
.globl timer_vector
timer_vector:
    // 交换 a0 和 mscratch，使用 a0 作为临时寄存器指针
    csrrw a0, mscratch, a0
    // 保存 a1, a2, a3 到 mscratch 指向的内存
    sd a1, 0(a0)    // 偏移 0: a1
    sd a2, 8(a0)    // 偏移 8: a2
    sd a3, 16(a0)   // 偏移 16: a3
    // 调用 Rust 中的处理函数
    call timer_vector_body
    // 恢复 a1, a2, a3
    ld a3, 16(a0)
    ld a2, 8(a0)
    ld a1, 0(a0)
    // 恢复 a0 并返回
    csrrw a0, mscratch, a0
    mret
