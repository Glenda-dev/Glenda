#include "sys.h"

#define PGSIZE 4096

static void test_helloworld(void) {
    syscall(SYS_helloworld);
}

static void test_copy(void) {
    int L[5] = {0};
    char *s = "hello, world";
    syscall(SYS_copyout, (long)L);
    syscall(SYS_copyin, (long)L, 5);
    syscall(SYS_copyinstr, (long)s);
}

static void test_brk(void) {
    long heap_top = 0;
    heap_top = syscall(SYS_brk, 0);
    heap_top = syscall(SYS_brk, heap_top + PGSIZE * 9);
    heap_top = syscall(SYS_brk, heap_top);
    heap_top = syscall(SYS_brk, heap_top - PGSIZE * 5);
    (void)heap_top;
}

static void test_stack(void) {
    static volatile char sink;
    char tmp[PGSIZE * 4];
    tmp[PGSIZE * 3] = 'h';
    tmp[PGSIZE * 3 + 1] = 'e';
    tmp[PGSIZE * 3 + 2] = 'l';
    tmp[PGSIZE * 3 + 3] = 'l';
    tmp[PGSIZE * 3 + 4] = 'o';
    tmp[PGSIZE * 3 + 5] = '\0';
    syscall(SYS_copyinstr, (long)(tmp + PGSIZE * 3));
    tmp[0] = 'w';
    tmp[1] = 'o';
    tmp[2] = 'r';
    tmp[3] = 'l';
    tmp[4] = 'd';
    tmp[5] = '\0';
    sink = tmp[0];
    syscall(SYS_copyinstr, (long)tmp);
}

int main(void)
{
    test_helloworld();
    test_copy();
    for (;;) {}
    return 0;
}
