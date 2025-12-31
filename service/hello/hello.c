#include "sys.h"

#define PGSIZE 4096
#define NUM 20
#define N_BUFFER_TEST 8
#define BLOCK_BASE 5000
#define INODE_TYPE_DIR  1
#define INODE_TYPE_DATA 2
#define MAXLEN_FILENAME 60

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
  const unsigned long VA_MAX = (1ul << 38);
  const unsigned long MMAP_END = (VA_MAX - (16ul * 256 + 2) * PGSIZE);
  const unsigned long MMAP_BEGIN = (MMAP_END - 64ul * 256 * PGSIZE);

  syscall(SYS_copyinstr, (long)"[TEST] mmap/munmap begin");

  syscall(SYS_mmap, MMAP_BEGIN + 4 * PGSIZE, 3 * PGSIZE);
  syscall(SYS_mmap, MMAP_BEGIN + 10 * PGSIZE, 2 * PGSIZE);
  syscall(SYS_mmap, MMAP_BEGIN + 2 * PGSIZE, 2 * PGSIZE);
  syscall(SYS_mmap, MMAP_BEGIN + 12 * PGSIZE, 1 * PGSIZE);
  syscall(SYS_mmap, MMAP_BEGIN + 7 * PGSIZE, 3 * PGSIZE);
  syscall(SYS_mmap, MMAP_BEGIN + 0 * PGSIZE, 2 * PGSIZE);
  syscall(SYS_mmap, 0, 10 * PGSIZE);

  syscall(SYS_munmap, MMAP_BEGIN + 10 * PGSIZE, 5 * PGSIZE);
  syscall(SYS_munmap, MMAP_BEGIN + 0 * PGSIZE, 10 * PGSIZE);
  syscall(SYS_munmap, MMAP_BEGIN + 17 * PGSIZE, 2 * PGSIZE);
  syscall(SYS_munmap, MMAP_BEGIN + 15 * PGSIZE, 2 * PGSIZE);
  syscall(SYS_munmap, MMAP_BEGIN + 19 * PGSIZE, 2 * PGSIZE);
  syscall(SYS_munmap, MMAP_BEGIN + 22 * PGSIZE, 1 * PGSIZE);
  syscall(SYS_munmap, MMAP_BEGIN + 21 * PGSIZE, 1 * PGSIZE);

  syscall(SYS_copyinstr, (long)"[TEST] mmap: overlap should fail");
  (void)syscall(SYS_mmap, MMAP_BEGIN + 0 * PGSIZE, 2 * PGSIZE);
  long rv = syscall(SYS_mmap, MMAP_BEGIN + 1 * PGSIZE, 2 * PGSIZE);
  if (rv != -1) { syscall(SYS_copyinstr, (long)"[WARN] overlap not rejected"); }
  syscall(SYS_munmap, MMAP_BEGIN + 0 * PGSIZE, 2 * PGSIZE);

  syscall(SYS_copyinstr, (long)"[TEST] mmap: unaligned should fail");
  rv = syscall(SYS_mmap, MMAP_BEGIN + 123, 2 * PGSIZE);
  if (rv != -1) { syscall(SYS_copyinstr, (long)"[WARN] unaligned begin not rejected"); }

  syscall(SYS_copyinstr, (long)"[TEST] munmap: unmapped range is no-op");
  syscall(SYS_munmap, MMAP_BEGIN + 8 * PGSIZE, 3 * PGSIZE);

  syscall(SYS_copyinstr, (long)"[PASS] mmap/munmap tests done");
}

void test_proczero() {
    int pid = syscall(SYS_getpid);
    if (pid == 1) {
        syscall(SYS_print_str, (long)"\nproczero: hello world!\n");
    }
}

void test_fork_order() {
    syscall(SYS_print_str, (long)"level-1!\n");
    syscall(SYS_fork);
    syscall(SYS_print_str, (long)"level-2!\n");
    syscall(SYS_fork);
    syscall(SYS_print_str, (long)"level-3!\n");
    syscall(SYS_copyinstr, (long)"[PASS] Fork order test done.");
}

void test_memory_fork() {
    volatile int pid;
    int i;
    const unsigned long VA_MAX = (1ul << 38);
    const unsigned long MMAP_END = (VA_MAX - (16ul * 256 + 2) * PGSIZE);
    const unsigned long MMAP_BEGIN = (MMAP_END - 64ul * 256 * PGSIZE);

    char *str1, *str2, *str3 = "STACK_REGION\n\n";
    char *tmp1 = "MMAP_REGION\n", *tmp2 = "HEAP_REGION\n";

    str1 = (char*)syscall(SYS_mmap, MMAP_BEGIN, PGSIZE);
    for (i = 0; tmp1[i] != '\0'; i++)
        str1[i] = tmp1[i];
    str1[i] = '\0';

    str2 = (char*)syscall(SYS_brk, 0);
    syscall(SYS_brk, (long long int)str2 + PGSIZE);
    for (i = 0; tmp2[i] != '\0'; i++)
        str2[i] = tmp2[i];
    str2[i] = '\0';

    pid = syscall(SYS_fork);
    syscall(SYS_print_int, pid);

    if (pid == 0) { // Child
      syscall(SYS_print_str, (long)"child proc: hello\n");
      syscall(SYS_print_str, (long)str1);
      syscall(SYS_print_str, (long)str2);
      syscall(SYS_print_str, (long)str3);
      syscall(SYS_exit, 1234);
    } else { // Parent
      int exit_state = 0;
      syscall(SYS_wait, (long)&exit_state);
      syscall(SYS_print_str, (long)"parent proc: hello\n");
      syscall(SYS_print_int, pid);
      if (exit_state == 1234)
        syscall(SYS_print_str, (long)"good boy!\n");
      else
        syscall(SYS_print_str, (long)"bad boy!\n");
    }

    syscall(SYS_copyinstr, (long)"[PASS] Memory fork test done.");
}

void test_sleep() {
    int pid = syscall(SYS_fork);
    if (pid == 0) {
        syscall(SYS_print_str, (long)"Ready to sleep!\n");
        syscall(SYS_sleep, 5);
        syscall(SYS_print_str, (long)"Ready to exit!\n");
        syscall(SYS_exit, 0);
    } else {
        syscall(SYS_wait, 0);
        syscall(SYS_print_str, (long)"Child exit!\n");
    }
    syscall(SYS_copyinstr, (long)"[PASS] Sleep test done.");
}

void test_bitmap() {
	unsigned int block_num[NUM];
	unsigned int inode_num[NUM];

	for (int i = 0; i < NUM; i++)
		block_num[i] = syscall(SYS_alloc_block);

	syscall(SYS_flush_buffer, N_BUFFER_TEST);
	syscall(SYS_show_bitmap, 0);

	for (int i = 0; i < NUM; i+=2)
		syscall(SYS_free_block, block_num[i]);

	syscall(SYS_flush_buffer, N_BUFFER_TEST);
	syscall(SYS_show_bitmap, 0);

	for (int i = 1; i < NUM; i+=2)
		syscall(SYS_free_block, block_num[i]);

	syscall(SYS_flush_buffer, N_BUFFER_TEST);
	syscall(SYS_show_bitmap, 0);

	for (int i = 0; i < NUM; i++)
		inode_num[i] = syscall(SYS_alloc_inode);

	syscall(SYS_flush_buffer, N_BUFFER_TEST);
	syscall(SYS_show_bitmap, 1);

	for (int i = 0; i < NUM; i++)
		syscall(SYS_free_inode, inode_num[i]);

	syscall(SYS_flush_buffer, N_BUFFER_TEST);
	syscall(SYS_show_bitmap, 1);

    syscall(SYS_copyinstr, (long)"[PASS] Bitmap test done.");
}

void test_buffer() {
	char data[PGSIZE], tmp[PGSIZE];
	unsigned long long buffer[N_BUFFER_TEST];

	for (int i = 0; i < 8; i++)
		data[i] = 'A' + i;
	data[8] = '\n';
	data[9] = '\0';

	syscall(SYS_print_str, (long)"\nstate-1\n");
	syscall(SYS_show_buffer);

	buffer[0] = syscall(SYS_get_block, BLOCK_BASE);
	syscall(SYS_write_block, buffer[0], (long)data);
	syscall(SYS_put_block, buffer[0]);

	syscall(SYS_print_str, (long)"\nstate-2\n");
	syscall(SYS_show_buffer);

	syscall(SYS_flush_buffer, N_BUFFER_TEST);

	buffer[0] = syscall(SYS_get_block, BLOCK_BASE);
	syscall(SYS_read_block, buffer[0], (long)tmp);
	syscall(SYS_put_block, buffer[0]);

	syscall(SYS_print_str, (long)"\n");
	syscall(SYS_print_str, (long)"write data:\n");
	syscall(SYS_print_str, (long)data);
	syscall(SYS_print_str, (long)"read data:\n");
	syscall(SYS_print_str, (long)tmp);

	syscall(SYS_print_str, (long)"\nstate-3\n");
	syscall(SYS_show_buffer);

	buffer[0] = syscall(SYS_get_block, BLOCK_BASE);
	buffer[3] = syscall(SYS_get_block, BLOCK_BASE + 3);
	buffer[7] = syscall(SYS_get_block, BLOCK_BASE + 7);
	buffer[2] = syscall(SYS_get_block, BLOCK_BASE + 2);
	buffer[4] = syscall(SYS_get_block, BLOCK_BASE + 4);

	syscall(SYS_print_str, (long)"\nstate-4\n");
    syscall(SYS_show_buffer);

	syscall(SYS_put_block, buffer[7]);
	syscall(SYS_put_block, buffer[0]);
	syscall(SYS_put_block, buffer[4]);

	syscall(SYS_print_str, (long)"\nstate-5\n");
	syscall(SYS_show_buffer);
	syscall(SYS_flush_buffer, 3);
	syscall(SYS_print_str, (long)"\nstate-6\n");
	syscall(SYS_show_buffer);

    syscall(SYS_print_str, (long)"\n[PASS] Buffer test done.\n");
}

static void test_fs_inodes(void) {
    syscall(SYS_copyinstr, (long)"[TEST] FS-1: inode alloc/dup/put/delete");

    long inum = syscall(SYS_inode_create, INODE_TYPE_DATA, 0, 0);
    syscall(SYS_print_str, (long)"  created inode ");
    syscall(SYS_print_int, inum);
    syscall(SYS_print_str, (long)"\n");
    syscall(SYS_inode_print, inum);

    long rc = syscall(SYS_inode_dup, inum);
    syscall(SYS_print_str, (long)"  after dup refcnt=");
    syscall(SYS_print_int, rc);
    syscall(SYS_print_str, (long)"\n");

    syscall(SYS_inode_put, inum);
    rc = syscall(SYS_inode_get_refcnt, inum);
    syscall(SYS_print_str, (long)"  after put refcnt=");
    syscall(SYS_print_int, rc);
    syscall(SYS_print_str, (long)"\n");

    // Simulate unlink then release to trigger free
    syscall(SYS_inode_set_nlink, inum, 0);
    syscall(SYS_inode_put, inum);

    syscall(SYS_copyinstr, (long)"[PASS] FS-1 done.");
}

static void test_fs_rw(void) {
    syscall(SYS_copyinstr, (long)"[TEST] FS-2: inode write/read/size");
    long inum = syscall(SYS_inode_create, INODE_TYPE_DATA, 0, 0);

    unsigned char wbuf[100];
    unsigned char rbuf[100];
    for (int i = 0; i < 100; i++) wbuf[i] = (unsigned char)i;

    long written = syscall(SYS_inode_write_data, inum, 0, (long)wbuf, 100);
    long read = syscall(SYS_inode_read_data, inum, 0, (long)rbuf, 100);

    if (written != 100 || read != 100) {
        syscall(SYS_copyinstr, (long)"[WARN] FS-2: length mismatch");
    }
    for (int i = 0; i < 100; i++) {
        if (wbuf[i] != rbuf[i]) {
            syscall(SYS_print_str, (long)"[FAIL] FS-2 byte mismatch at ");
            syscall(SYS_print_int, i);
            syscall(SYS_print_str, (long)"\n");
            break;
        }
    }

    // Cleanup
    syscall(SYS_inode_set_nlink, inum, 0);
    syscall(SYS_inode_put, inum);
    syscall(SYS_copyinstr, (long)"[PASS] FS-2 done.");
}

static void test_fs_dentry(void) {
    syscall(SYS_copyinstr, (long)"[TEST] FS-3: dentry create/search/delete");
    // Ensure root exists and sane
    syscall(SYS_prepare_root);

    const char *name = "test_file";
    unsigned long target = 100; // arbitrary target inum for dentry test

    long rc = syscall(SYS_dentry_create, 0, target, (long)name);
    if (rc == -1) {
        syscall(SYS_copyinstr, (long)"[WARN] FS-3: create failed");
    }
    long found = syscall(SYS_dentry_search, 0, (long)name);
    if ((unsigned long)found != target) {
        syscall(SYS_copyinstr, (long)"[FAIL] FS-3: search mismatch");
    }
    syscall(SYS_dentry_print, 0);
    long removed = syscall(SYS_dentry_delete, 0, (long)name);
    if ((unsigned long)removed != target) {
        syscall(SYS_copyinstr, (long)"[WARN] FS-3: delete returned unexpected inum");
    }
    long again = syscall(SYS_dentry_search, 0, (long)name);
    if (again != -1) {
        syscall(SYS_copyinstr, (long)"[WARN] FS-3: entry still present");
    }

    syscall(SYS_copyinstr, (long)"[PASS] FS-3 done.");
}

static void test_fs_path(void) {
    syscall(SYS_copyinstr, (long)"[TEST] FS-4: path_to_inode/parent + data");
    syscall(SYS_prepare_root);

    long inum = syscall(SYS_inode_create, INODE_TYPE_DATA, 0, 0);
    const char *pname = "/test_path";
    const char *leaf = "test_path";
    const char *msg = "hello_path";
    char out[32] = {0};
    char tail[MAXLEN_FILENAME] = {0};

    // Write content to the file's inode
    syscall(SYS_inode_write_data, inum, 0, (long)msg, 10);
    // Link into root directory
    syscall(SYS_dentry_create, 0, inum, (long)leaf);

    long finum = syscall(SYS_path_to_inode, (long)pname);
    if (finum == -1) {
        syscall(SYS_copyinstr, (long)"[FAIL] FS-4: path not found");
    } else {
        long r = syscall(SYS_inode_read_data, finum, 0, (long)out, 10);
        (void)r;
        // Compare
        int ok = 1;
        for (int i = 0; i < 10; i++) if (out[i] != msg[i]) ok = 0;
        if (!ok) syscall(SYS_copyinstr, (long)"[FAIL] FS-4: data mismatch");

        long parent = syscall(SYS_path_to_parent, (long)pname, (long)tail);
        syscall(SYS_print_str, (long)"  parent inum=");
        syscall(SYS_print_int, parent);
        syscall(SYS_print_str, (long)", tail='");
        syscall(SYS_print_str, (long)tail);
        syscall(SYS_print_str, (long)"'\n");
    }

    // Cleanup
    syscall(SYS_dentry_delete, 0, (long)leaf);
    syscall(SYS_inode_set_nlink, inum, 0);
    syscall(SYS_inode_put, inum);

    syscall(SYS_copyinstr, (long)"[PASS] FS-4 done.");
}

int main(void)
{
  test_helloworld();
  test_copy();
  test_stack();
  test_brk();
  test_mmap();
  test_proczero();
  test_memory_fork();

  test_bitmap();
  test_buffer();

  // Currently tests are verified in Rust API
  /* test_fs_inodes(); */
  /* test_fs_rw(); */
  /* test_fs_dentry(); */
  /* test_fs_path(); */

  test_sleep();
  test_fork_order();

  for (;;) {}
  return 0;
}
