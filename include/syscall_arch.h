#ifndef GLENDA_SYSCALL_ARCH_H
#define GLENDA_SYSCALL_ARCH_H

static inline long __glenda_syscall0(long num) {
    register long a7 asm("a7") = num;
    asm volatile ("ecall" : : "r"(a7) : "memory");
    return 0;
}

#endif // GLENDA_SYSCALL_ARCH_H
