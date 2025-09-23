pub mod spinlock;

pub fn run_spinlock_tests(hartid: usize, dtb: *const u8) {
    spinlock::run(hartid, dtb);
}

pub fn init_harts(hartid: usize, dtb: *const u8) {
    spinlock::init(hartid, dtb);
}
