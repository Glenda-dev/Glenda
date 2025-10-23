mod barrier;
mod pmem;
mod printk;
mod run;
mod spinlock;
mod syscall;
mod trap;
mod vm;

pub fn test(hartid: usize) {
    run::run_tests(hartid);
}

pub fn test_user(hartid: usize) {
    run::run_tests_user(hartid);
}
