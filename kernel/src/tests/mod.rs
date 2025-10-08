mod barrier;
mod pmem;
mod printk;
mod run;
mod spinlock;
mod vm;

pub fn test(hartid: usize) {
    run::run_tests(hartid);
}
