mod spinlock;

pub fn run_spinlock_tests(hartid: usize) {
    spinlock::run(hartid);
}
