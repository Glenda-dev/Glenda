mod dtb;
mod hart;
mod irq;
mod pmem;
mod uart;
mod vm;

pub fn init(hartid: usize, dtb: *const u8) {
    dtb::init(hartid, dtb);
    uart::init(hartid, dtb);
    pmem::init(hartid, dtb);
    irq::init(hartid, dtb);
    vm::init(hartid, dtb);
    hart::init(hartid, dtb);
}
