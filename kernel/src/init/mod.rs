mod harts;
mod pmem;
mod trap;
mod vm;

pub fn init(hartid: usize, dtb: *const u8) {
    init_pmem(hartid, dtb);
    init_trap(hartid, dtb);
    init_vm(hartid, dtb);
    init_harts(hartid, dtb);
}

fn init_pmem(hartid: usize, _dtb: *const u8) {
    pmem::pmem_init(hartid);
}

fn init_harts(hartid: usize, dtb: *const u8) {
    harts::bootstrap_secondary_harts(hartid, dtb);
}

fn init_vm(hartid: usize, _dtb: *const u8) {
    vm::vm_init(hartid);
}

fn init_trap(hartid: usize, _dtb: *const u8) {
    trap::trap_init(hartid);
}
