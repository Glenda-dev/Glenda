mod dtb;
mod hart;
mod irq;
mod pmem;
mod proc;
mod trap;
mod uart;
mod vm;

pub fn init(hartid: usize, dtb: *const u8) {
    dtb::init(hartid, dtb);
    uart::init(hartid, dtb);
    pmem::init(hartid, dtb);
    trap::init(hartid, dtb);
    irq::init(hartid, dtb);
    vm::init(hartid, dtb);
    hart::init(hartid, dtb);
    proc::init(hartid, dtb);
}
