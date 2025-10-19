use core::arch::asm;
use riscv::register::stvec::{self, Stvec};
/// 使用内联汇编实现的 S-mode 陷阱向量处理器
/// 这个函数会保存所有通用寄存器，调用 trap_kernel_handler，然后恢复寄存器
#[unsafe(no_mangle)]
#[allow(named_asm_labels)]
pub extern "C" fn kernel_vector() -> ! {
    unsafe {
        asm!(
            // Align entry to 4-byte boundary
            ".p2align 2",
            // 为栈指针减少 256 字节以容纳 32 个 64 位寄存器
            "addi sp, sp, -256",
            // 保存所有通用寄存器到栈上
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
            // 调用 Rust 陷阱处理函数，传递栈指针(陷阱上下文)
            "mv a0, sp",
            "call trap_kernel_handler",
            // 从栈上恢复所有通用寄存器
            "ld ra, 0(sp)",
            "ld gp, 16(sp)",
            // tp 寄存器的值可能不可靠，跳过恢复
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
            // 恢复栈指针并返回
            "addi sp, sp, 256",
            "sret",
            options(noreturn),
        );
    }
}

/// 使用内联汇编实现的 M-mode 定时器中断处理器
/// 用于处理机器模式下的时钟中断，并触发 S-mode 软件中断
#[unsafe(no_mangle)]
#[allow(named_asm_labels)]
pub extern "C" fn timer_vector() -> ! {
    unsafe {
        asm!(
            // Align entry to 4-byte boundary
            ".p2align 2",
            // 交换 a0 和 mscratch，使用 a0 作为临时寄存器指针
            "csrrw a0, mscratch, a0",
            // 保存 a1, a2, a3 到 mscratch 指向的内存
            "sd a1, 0(a0)",  // 偏移 0: a1
            "sd a2, 8(a0)",  // 偏移 8: a2
            "sd a3, 16(a0)", // 偏移 16: a3
            // 更新下一个定时器中断的比较时间
            // cmp_time += INTERVAL
            "ld a1, 24(a0)",  // 偏移 24: CLINT_MTIMECMP 地址
            "ld a2, 32(a0)",  // 偏移 32: INTERVAL
            "ld a3, 0(a1)",   // 读取当前的比较时间
            "add a3, a3, a2", // 加上间隔
            "sd a3, 0(a1)",   // 写回新的比较时间
            // 触发 S-mode 软件中断
            // 设置 sip[SSIP] 位以告知 S-mode 有软件中断
            "li a1, 2", // SSIP = 1 << 1 = 0x2
            "csrw sip, a1",
            // 恢复 a1, a2, a3
            "ld a3, 16(a0)",
            "ld a2, 8(a0)",
            "ld a1, 0(a0)",
            // 恢复 a0 并返回
            "csrrw a0, mscratch, a0",
            "mret",
            options(noreturn),
        );
    }
}

pub fn set_vector() {
    let kernel_vec_addr = kernel_vector as usize;
    let vec = Stvec::new(kernel_vec_addr, stvec::TrapMode::Direct);
    unsafe {
        // set supervisor trap vector address
        stvec::write(vec);
    }
}
