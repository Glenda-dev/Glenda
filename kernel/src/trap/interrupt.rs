use crate::hart;
use riscv::register::{sie, sscratch, sstatus};

pub fn enable_s() {
    let hartid = hart::getid();
    unsafe {
        sscratch::write(hartid);
        sstatus::set_sie();
        sie::set_sext();
        sie::set_ssoft();
        sie::set_stimer();
        super::timer::start(hartid);
    }
}

pub fn disable_s() {
    unsafe {
        sstatus::clear_sie();
    }
}

pub fn enable_m() {
    unsafe {
        riscv::register::mstatus::set_mie();
    }
}

pub fn disable_m() {
    unsafe {
        riscv::register::mstatus::clear_mie();
    }
}

pub fn enter() {
    let hart = hart::get();
    hart.nest_count += 1;
}

pub fn exit() {
    let hart = hart::get();
    if hart.nest_count > 0 {
        hart.nest_count -= 1;
    }
}
