
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
