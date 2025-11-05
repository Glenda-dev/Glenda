#ifndef GLENDA_SYS_H
#define GLENDA_SYS_H

#include "syscall/num.h"
#include "syscall/arch.h"

// Macro Mapping
#define _glenda_syscall1(n)                          __glenda_syscall0((long)(n))
#define _glenda_syscall2(n,a)                        __glenda_syscall1((long)(n), (long)(a))
#define _glenda_syscall3(n,a,b)                      __glenda_syscall2((long)(n), (long)(a), (long)(b))
#define _glenda_syscall4(n,a,b,c)                    __glenda_syscall3((long)(n), (long)(a), (long)(b), (long)(c))
#define _glenda_syscall5(n,a,b,c,d)                  __glenda_syscall4((long)(n), (long)(a), (long)(b), (long)(c), (long)(d))
#define _glenda_syscall6(n,a,b,c,d,e)                __glenda_syscall5((long)(n), (long)(a), (long)(b), (long)(c), (long)(d), (long)(e))
#define _glenda_syscall7(n,a,b,c,d,e,f)              __glenda_syscall6((long)(n), (long)(a), (long)(b), (long)(c), (long)(d), (long)(e), (long)(f))

#define _GLENDA_GET_MACRO(_1,_2,_3,_4,_5,_6,_7, NAME, ...) NAME
#define syscall(...) _GLENDA_GET_MACRO(__VA_ARGS__, \
    _glenda_syscall7, _glenda_syscall6, _glenda_syscall5, _glenda_syscall4, _glenda_syscall3, _glenda_syscall2, _glenda_syscall1)(__VA_ARGS__)

#endif // GLENDA_SYS_H
