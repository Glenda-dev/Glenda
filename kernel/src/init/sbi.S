.section .text
.align 2

// SBI Hart State Management (HSM) [1]
// a0 = hartid, a1 = start_addr, a2 = opaque
// a6 = function id (0), a7 = extension id (0x48534d)
// returns: a0 = error code (isize), a1 = value (UNUSED!)
// [1]: https://www.scs.stanford.edu/~zyedidia/docs/riscv/riscv-sbi.pdf, Chapter Nine   
.globl sbi_hart_start_asm
sbi_hart_start_asm:
    li   a6, 0
    li   a7, 0x48534d
    ecall
    ret
