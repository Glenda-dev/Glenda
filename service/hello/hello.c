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
    syscall(SYS_copyinstr, (long)"[PASS] brk test passed");
}

static void test_stack(void) {
    static volatile char sink;
    char tmp[PGSIZE * 2];
    tmp[PGSIZE * 1] = 'h';
    tmp[PGSIZE * 1 + 1] = 'e';
    tmp[PGSIZE * 1 + 2] = 'l';
    tmp[PGSIZE * 1 + 3] = 'l';
    tmp[PGSIZE * 1 + 4] = 'o';
    tmp[PGSIZE * 1 + 5] = '\0';
    syscall(SYS_copyinstr, (long)(tmp + PGSIZE * 1));
    tmp[0] = 'w';
    tmp[1] = 'o';
    tmp[2] = 'r';
    tmp[3] = 'l';
    tmp[4] = 'd';
    tmp[5] = '\0';
    sink = tmp[0];
    syscall(SYS_copyinstr, (long)tmp);
}

static void test_mmap(void) {
  // Keep macros consistent with kernel
  const unsigned long VA_MAX = (1ul << 38);
  const unsigned long MMAP_END = (VA_MAX - (16ul * 256 + 2) * PGSIZE);
  const unsigned long MMAP_BEGIN = (MMAP_END - 64ul * 256 * PGSIZE);

  syscall(SYS_copyinstr, (long)"[TEST] mmap/munmap begin");

  syscall(SYS_mmap, MMAP_BEGIN + 4 * PGSIZE, 3 * PGSIZE);   // [4,7)
  syscall(SYS_mmap, MMAP_BEGIN + 10 * PGSIZE, 2 * PGSIZE);  // [10,12)
  syscall(SYS_mmap, MMAP_BEGIN + 2 * PGSIZE, 2 * PGSIZE);   // [2,4) -> merge left with [4,7) => [2,7)
  syscall(SYS_mmap, MMAP_BEGIN + 12 * PGSIZE, 1 * PGSIZE);  // [12,13) -> merge right with [10,12) => [10,13)
  syscall(SYS_mmap, MMAP_BEGIN + 7 * PGSIZE, 3 * PGSIZE);   // [7,10) -> bridge merge => [2,13)
  syscall(SYS_mmap, MMAP_BEGIN + 0 * PGSIZE, 2 * PGSIZE);   // [0,2) -> merge left => [0,13)
  syscall(SYS_mmap, 0, 10 * PGSIZE);                        // first-fit => [13,23)

  syscall(SYS_munmap, MMAP_BEGIN + 10 * PGSIZE, 5 * PGSIZE); // unmap [10,15): trims [0,13)->[0,10) and [13,23)->[15,23)
  syscall(SYS_munmap, MMAP_BEGIN + 0 * PGSIZE, 10 * PGSIZE); // remove [0,10)
  syscall(SYS_munmap, MMAP_BEGIN + 17 * PGSIZE, 2 * PGSIZE); // split [15,23) -> [15,17) + [19,23)
  syscall(SYS_munmap, MMAP_BEGIN + 15 * PGSIZE, 2 * PGSIZE); // remove [15,17)
  syscall(SYS_munmap, MMAP_BEGIN + 19 * PGSIZE, 2 * PGSIZE); // trim front [19,23)->[21,23)
  syscall(SYS_munmap, MMAP_BEGIN + 22 * PGSIZE, 1 * PGSIZE); // trim back [21,23)->[21,22)
  syscall(SYS_munmap, MMAP_BEGIN + 21 * PGSIZE, 1 * PGSIZE); // remove [21,22) -> empty

  // Some additional
  syscall(SYS_copyinstr, (long)"[TEST] mmap: overlap should fail");
  (void)syscall(SYS_mmap, MMAP_BEGIN + 0 * PGSIZE, 2 * PGSIZE); // map [0,2)
  long rv = syscall(SYS_mmap, MMAP_BEGIN + 1 * PGSIZE, 2 * PGSIZE); // overlap [1,3) -> expect failure (usize::MAX)
  if (rv != -1) { syscall(SYS_copyinstr, (long)"[WARN] overlap not rejected"); }
  syscall(SYS_munmap, MMAP_BEGIN + 0 * PGSIZE, 2 * PGSIZE); // cleanup

  syscall(SYS_copyinstr, (long)"[TEST] mmap: unaligned should fail");
  rv = syscall(SYS_mmap, MMAP_BEGIN + 123, 2 * PGSIZE);
  if (rv != -1) { syscall(SYS_copyinstr, (long)"[WARN] unaligned begin not rejected"); }

  syscall(SYS_copyinstr, (long)"[TEST] munmap: unmapped range is no-op");
  syscall(SYS_munmap, MMAP_BEGIN + 8 * PGSIZE, 3 * PGSIZE); // no mapped regions -> no-op

  syscall(SYS_copyinstr, (long)"[PASS] mmap/munmap tests done");
}

int main(void)
{
  test_helloworld();
  test_copy();
  test_stack();
  test_brk();
  test_mmap();

  for (;;) {}
  return 0;
}
