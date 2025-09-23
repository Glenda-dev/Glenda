mod harts;

pub fn init_harts(hartid: usize, dtb: *const u8) {
    harts::bootstrap_secondary_harts(hartid, dtb);
}
