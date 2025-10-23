.section .text
.align 2

.globl enter_proc
enter_proc:
    // a0 = entry PC (VA)
    // a1 = user SP (VA)
    // Set user stack pointer
    mv   sp, a1
    // Set sepc to user entry point
    csrw sepc, a0
    // Clear SPP to 0 (User)
    li   t0, (1 << 8)
    csrc sstatus, t0
    // Enable SUM so S-mode can touch U pages during trap save/restore
    li   t0, (1 << 18)
    csrs sstatus, t0
    // Return to user mode at sepc with user SP
    sret

