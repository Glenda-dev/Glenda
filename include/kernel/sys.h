#ifndef GLENDA_SYS_H
#define GLENDA_SYS_H

#include "syscall/num.h"
#include "syscall/arch.h"

static inline long syscall(long num) {
    return __glenda_syscall0(num);
}

#endif // GLENDA_SYS_H
