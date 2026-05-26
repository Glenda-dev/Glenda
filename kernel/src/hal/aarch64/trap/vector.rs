use crate::trap::handler::trap_kernel_handler;
use core::arch::naked_asm;

unsafe extern "C" {
    pub static __user_vector_table_base: u8;
}

const KERNEL_TRAP_FRAME_SIZE: usize = 320;

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_vector() {
    naked_asm!(
        ".align 11",
        // Current EL with SP0 (Synchronous)
        "b kernel_vector_sync",
        ".align 7",
        // Current EL with SP0 (IRQ)
        "b kernel_vector_irq",
        ".align 7",
        "b kernel_vector_fiq",
        ".align 7",
        "b kernel_vector_serror",
        ".align 7",
        // Current EL with SPx
        "b kernel_vector_sync",
        ".align 7",
        "b kernel_vector_irq",
        ".align 7",
        "b kernel_vector_fiq",
        ".align 7",
        "b kernel_vector_serror",
        ".align 7",
        // Lower EL (AArch64)
        "b user_vector_sync",
        ".align 7",
        "b user_vector_irq",
        ".align 7",
        "b user_vector_fiq",
        ".align 7",
        "b user_vector_serror",
        ".align 7",
        // Lower EL (AArch32)
        "b user_vector_sync",
        ".align 7",
        "b user_vector_irq",
        ".align 7",
        "b user_vector_fiq",
        ".align 7",
        "b user_vector_serror",
        ".align 7"
    );
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_vector_sync() {
    naked_asm!("msr contextidr_el1, xzr", "b kernel_vector_common",);
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_vector_irq() {
    naked_asm!(
        "sub sp, sp, #16",
        "str x9, [sp]",
        "mov x9, #1",
        "msr contextidr_el1, x9",
        "ldr x9, [sp]",
        "add sp, sp, #16",
        "b kernel_vector_common",
    );
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_vector_fiq() {
    naked_asm!(
        "sub sp, sp, #16",
        "str x9, [sp]",
        "mov x9, #2",
        "msr contextidr_el1, x9",
        "ldr x9, [sp]",
        "add sp, sp, #16",
        "b kernel_vector_common",
    );
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_vector_serror() {
    naked_asm!(
        "sub sp, sp, #16",
        "str x9, [sp]",
        "mov x9, #3",
        "msr contextidr_el1, x9",
        "ldr x9, [sp]",
        "add sp, sp, #16",
        "b kernel_vector_common",
    );
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kernel_vector_common() {
    naked_asm!(
        "sub sp, sp, #{frame_size}",
        "stp x0, x1, [sp, #16 * 0]",
        "stp x2, x3, [sp, #16 * 1]",
        "stp x4, x5, [sp, #16 * 2]",
        "stp x6, x7, [sp, #16 * 3]",
        "stp x8, x9, [sp, #16 * 4]",
        "stp x10, x11, [sp, #16 * 5]",
        "stp x12, x13, [sp, #16 * 6]",
        "stp x14, x15, [sp, #16 * 7]",
        "stp x16, x17, [sp, #16 * 8]",
        "stp x18, x19, [sp, #16 * 9]",
        "stp x20, x21, [sp, #16 * 10]",
        "stp x22, x23, [sp, #16 * 11]",
        "stp x24, x25, [sp, #16 * 12]",
        "stp x26, x27, [sp, #16 * 13]",
        "stp x28, x29, [sp, #16 * 14]",
        "str x30, [sp, #240]",
        "add x16, sp, #{frame_size}",
        "str x16, [sp, #248]",
        "mrs x16, elr_el1",
        "str x16, [sp, #256]",
        "mrs x16, spsr_el1",
        "str x16, [sp, #264]",
        "mrs x16, tpidr_el0",
        "str x16, [sp, #272]",
        "str xzr, [sp, #280]",
        "str xzr, [sp, #288]",
        "str xzr, [sp, #296]",
        "str xzr, [sp, #304]",
        "mov x0, sp",
        "bl {handler}",
        "ldr x16, [sp, #256]",
        "msr elr_el1, x16",
        "ldr x16, [sp, #264]",
        "msr spsr_el1, x16",
        "ldr x16, [sp, #272]",
        "msr tpidr_el0, x16",
        "ldp x0, x1, [sp, #16 * 0]",
        "ldp x2, x3, [sp, #16 * 1]",
        "ldp x4, x5, [sp, #16 * 2]",
        "ldp x6, x7, [sp, #16 * 3]",
        "ldp x8, x9, [sp, #16 * 4]",
        "ldp x10, x11, [sp, #16 * 5]",
        "ldp x12, x13, [sp, #16 * 6]",
        "ldp x14, x15, [sp, #16 * 7]",
        "ldp x16, x17, [sp, #16 * 8]",
        "ldp x18, x19, [sp, #16 * 9]",
        "ldp x20, x21, [sp, #16 * 10]",
        "ldp x22, x23, [sp, #16 * 11]",
        "ldp x24, x25, [sp, #16 * 12]",
        "ldp x26, x27, [sp, #16 * 13]",
        "ldp x28, x29, [sp, #16 * 14]",
        "ldr x30, [sp, #240]",
        "add sp, sp, #{frame_size}",
        "eret",
        handler = sym trap_kernel_handler,
        frame_size = const KERNEL_TRAP_FRAME_SIZE,
    );
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = "trampsec")]
pub unsafe extern "C" fn user_vector_table() {
    naked_asm!(
        ".align 11",
        ".globl __user_vector_table_base",
        "__user_vector_table_base:",
        // Current EL with SP0 (Synchronous)
        "b user_vector_sync",
        ".align 7",
        // Current EL with SP0 (IRQ)
        "b user_vector_irq",
        ".align 7",
        "b user_vector_fiq",
        ".align 7",
        "b user_vector_serror",
        ".align 7",
        // Current EL with SPx
        "b user_vector_sync",
        ".align 7",
        "b user_vector_irq",
        ".align 7",
        "b user_vector_fiq",
        ".align 7",
        "b user_vector_serror",
        ".align 7",
        // Lower EL (AArch64)
        "b user_vector_sync",
        ".align 7",
        "b user_vector_irq",
        ".align 7",
        "b user_vector_fiq",
        ".align 7",
        "b user_vector_serror",
        ".align 7",
        // Lower EL (AArch32)
        "b user_vector_sync",
        ".align 7",
        "b user_vector_irq",
        ".align 7",
        "b user_vector_fiq",
        ".align 7",
        "b user_vector_serror",
        ".align 7"
    );
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = "trampsec")]
pub unsafe extern "C" fn user_vector_sync() {
    naked_asm!("msr contextidr_el1, xzr", "b user_vector",);
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = "trampsec")]
pub unsafe extern "C" fn user_vector_irq() {
    naked_asm!(
        "sub sp, sp, #16",
        "str x9, [sp]",
        "mov x9, #1",
        "msr contextidr_el1, x9",
        "ldr x9, [sp]",
        "add sp, sp, #16",
        "b user_vector",
    );
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = "trampsec")]
pub unsafe extern "C" fn user_vector_fiq() {
    naked_asm!(
        "sub sp, sp, #16",
        "str x9, [sp]",
        "mov x9, #2",
        "msr contextidr_el1, x9",
        "ldr x9, [sp]",
        "add sp, sp, #16",
        "b user_vector",
    );
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = "trampsec")]
pub unsafe extern "C" fn user_vector_serror() {
    naked_asm!(
        "sub sp, sp, #16",
        "str x9, [sp]",
        "mov x9, #3",
        "msr contextidr_el1, x9",
        "ldr x9, [sp]",
        "add sp, sp, #16",
        "b user_vector",
    );
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = "trampsec")]
pub unsafe extern "C" fn user_vector() {
    naked_asm!(
        "msr tpidrro_el0, x0",
        "mrs x0, tpidr_el1",    // x0 = TrapFrame pointer
        "str x30, [x0, #240]",  // regs[30]
        "mrs x30, tpidrro_el0", // x30 = original x0
        "str x30, [x0, #0]",    // regs[0]
        "stp x1, x2, [x0, #8]",
        "stp x3, x4, [x0, #24]",
        "stp x5, x6, [x0, #40]",
        "stp x7, x8, [x0, #56]",
        "stp x9, x10, [x0, #72]",
        "stp x11, x12, [x0, #88]",
        "stp x13, x14, [x0, #104]",
        "stp x15, x16, [x0, #120]",
        "stp x17, x18, [x0, #136]",
        "stp x19, x20, [x0, #152]",
        "stp x21, x22, [x0, #168]",
        "stp x23, x24, [x0, #184]",
        "stp x25, x26, [x0, #200]",
        "stp x27, x28, [x0, #216]",
        "str x29, [x0, #232]",
        "mrs x1, sp_el0",
        "str x1, [x0, #248]",
        "mrs x1, elr_el1",
        "str x1, [x0, #256]",
        "mrs x1, spsr_el1",
        "str x1, [x0, #264]",
        "mrs x1, tpidr_el0",
        "str x1, [x0, #272]",
        "ldr x1, [x0, #280]", // kernel_sp
        "mov sp, x1",
        "ldr x1, [x0, #296]", // kernel_mmu
        "msr ttbr0_el1, x1",
        "isb",
        "ldr x1, [x0, #288]", // kernel_vector (trap_user_handler)
        "br x1"
    );
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
#[unsafe(link_section = "trampsec")]
pub unsafe extern "C" fn user_return(_trapframe: usize, _user_ttbr0: usize) {
    naked_asm!(
        "msr ttbr0_el1, x1",
        "isb",
        "ldr x2, [x0, #248]", // sp
        "msr sp_el0, x2",
        "ldr x2, [x0, #256]", // elr
        "msr elr_el1, x2",
        "ldr x2, [x0, #264]", // spsr
        "msr spsr_el1, x2",
        "ldr x2, [x0, #272]", // tpidr
        "msr tpidr_el0, x2",
        "ldp x1, x2, [x0, #8]",
        "ldp x3, x4, [x0, #24]",
        "ldp x5, x6, [x0, #40]",
        "ldp x7, x8, [x0, #56]",
        "ldp x9, x10, [x0, #72]",
        "ldp x11, x12, [x0, #88]",
        "ldp x13, x14, [x0, #104]",
        "ldp x15, x16, [x0, #120]",
        "ldp x17, x18, [x0, #136]",
        "ldp x19, x20, [x0, #152]",
        "ldp x21, x22, [x0, #168]",
        "ldp x23, x24, [x0, #184]",
        "ldp x25, x26, [x0, #200]",
        "ldp x27, x28, [x0, #216]",
        "ldr x29, [x0, #232]",
        "ldr x30, [x0, #240]",
        "ldr x0, [x0, #0]",
        "eret"
    );
}
