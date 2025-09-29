mod harts;
pub mod pmem;

pub fn init_pmem() {
    pmem::pmem_init();
}

pub fn init_harts(hartid: usize, dtb: *const u8) {
    harts::bootstrap_secondary_harts(hartid, dtb);
}
