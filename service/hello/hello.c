#include "sys.h"

int main(void)
{
    syscall(SYS_helloworld);
    syscall(SYS_helloworld);
    //for (;;) {}
    return 0;
}

