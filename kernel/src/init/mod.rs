mod hart;
mod pmem;
mod trap;
mod vm;

pub fn init(hartid: usize, dtb: *const u8) {
    pmem::init(hartid, dtb);
    trap::init(hartid, dtb);
    vm::init(hartid, dtb);
    hart::init(hartid, dtb);
}
