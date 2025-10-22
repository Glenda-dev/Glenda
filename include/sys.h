#ifndef GLENDA_SYS_H
#define GLENDA_SYS_H

#include "syscall_num.h"
#include "syscall_arch.h"

static inline long syscall(long num) {
    return __glenda_syscall0(num);
}

#endif // GLENDA_SYS_H
