mod hart;
mod irq;
mod pmem;
mod vm;

pub fn init(hartid: usize, dtb: *const u8) {
    pmem::init(hartid, dtb);
    irq::init(hartid, dtb);
    vm::init(hartid, dtb);
    hart::init(hartid, dtb);
}
