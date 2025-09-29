mod pmem;
mod printk;
mod spinlock;

pub fn run_spinlock_tests(hartid: usize) {
    spinlock::run(hartid);
}
pub fn run_printk_tests(hartid: usize) {
    if hartid != 0 {
        return;
    }
    printk::run();
}
pub fn run_pmem_tests(hartid: usize) {
    pmem::run(hartid);
}
