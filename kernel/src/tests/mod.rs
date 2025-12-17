mod allocator;
mod barrier;
mod mmaprepo;
mod pmem;
mod printk;
mod run;
mod spinlock;
mod trap;
mod user;
mod vm;

pub fn test(hartid: usize) {
    run::run_tests(hartid);
}

pub fn test_user(hartid: usize) {
    run::run_tests_user(hartid);
}
