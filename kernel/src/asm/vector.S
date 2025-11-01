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

# 这部分代码在用户内核切换时发挥作用
# 不论是内核页表还是用户页表, 这部分代码被映射在同一个虚拟地址空间

# kernel.ld 里面添加这个section
.section trampsec

# trampoline会作为一个物理页的起点地址
.globl trampoline
trampoline:

# 触发用户态trap后的处理过程
.align 4
.globl user_vector
user_vector:

#------------------sd 过程 (begin)-----------------------

        # sscratch寄存器存放了p->trapframe
        csrrw a0, sscratch, a0

        # 把 TrapFrame 中由内核预先写入的指针给 a3
        # See: trap_user_handler
        ld a3, 280(a0)

        # 保存通用寄存器到trapframe
        sd ra, 40(a0)
        sd sp, 48(a0)
        sd gp, 56(a0)
        sd tp, 64(a0)
        sd t0, 72(a0)
        sd t1, 80(a0)
        sd t2, 88(a0)
        sd s0, 96(a0)
        sd s1, 104(a0)
        sd a1, 120(a0)
        sd a2, 128(a0)
        sd a3, 136(a0)
        sd a4, 144(a0)
        sd a5, 152(a0)
        sd a6, 160(a0)
        sd a7, 168(a0)
        sd s2, 176(a0)
        sd s3, 184(a0)
        sd s4, 192(a0)
        sd s5, 200(a0)
        sd s6, 208(a0)
        sd s7, 216(a0)
        sd s8, 224(a0)
        sd s9, 232(a0)
        sd s10, 240(a0)
        sd s11, 248(a0)
        sd t3, 256(a0)
        sd t4, 264(a0)
        sd t5, 272(a0)
        sd t6, 280(a0)

        # 保存 a0 到 p->trapframe
        csrr t0, sscratch
        sd t0, 112(a0)

#------------------sd 过程 (end)-------------------------

        # 恢复之前保存的内核执行环境
        # 1. 内核栈指针
        # 2. hartid信息
        # 3. 内核页表
        # 之后跳转到 trap_user_handler

        # sp = tf->user_to_kern_sp
        ld sp, 8(a0)
        # tp = tf->user_to_kern_hartid
        ld tp, 32(a0)
        sd tp, 64(a0)
        # t0 = tf->user_to_kern_trapvector
        ld t0, 16(a0)

        # t1 = tf->user_to_kern_satp
        # 将内核页表写入satp寄存器
        # 切换页表后a0指向的地址实效, 所以所有ld操作都要在这之前完成
        ld t1, 0(a0)
        csrw satp, t1
        sfence.vma zero, zero

        # 进入trap处理逻辑
        # 切换到内核页表后，用之前保存的 a3 作为 TrapFrame 指针
        mv a0, a3
        jr t0


# 用户态trap处理完成后返回
# user_return(trapframe, pagetable)
.globl user_return
user_return:

        # 切换到用户页表
        csrw satp, a1
        sfence.vma zero, zero

#---------------------ld 过程 (begin)----------------------

        csrw sscratch, a0

        ld ra, 40(a0)
        ld sp, 48(a0)
        ld gp, 56(a0)
        ld tp, 64(a0)
        ld t0, 72(a0)
        ld t1, 80(a0)
        ld t2, 88(a0)
        ld s0, 96(a0)
        ld s1, 104(a0)
        ld a1, 120(a0)
        ld a2, 128(a0)
        ld a3, 136(a0)
        ld a4, 144(a0)
        ld a5, 152(a0)
        ld a6, 160(a0)
        ld a7, 168(a0)
        ld s2, 176(a0)
        ld s3, 184(a0)
        ld s4, 192(a0)
        ld s5, 200(a0)
        ld s6, 208(a0)
        ld s7, 216(a0)
        ld s8, 224(a0)
        ld s9, 232(a0)
        ld s10, 240(a0)
        ld s11, 248(a0)
        ld t3, 256(a0)
        ld t4, 264(a0)
        ld t5, 272(a0)
        ld t6, 280(a0)

        ld a0, 112(a0)

#---------------------ld 过程 (end)----------------------

        # S-mode to U-mode
        sret
