use super::super::super::timer;
use crate::hart;
use riscv::interrupt::supervisor::Interrupt;
use riscv::register::sip;

pub fn interrupt_handler_ssip() {
    let hartid = hart::getid();
    if hartid == 0 {
        timer::update();
    }
    unsafe {
        sip::clear_pending(Interrupt::SupervisorSoft);
    }
}

pub fn interrupt_handler_stip() {
    let hartid = hart::getid();
    if hartid == 0 {
        timer::update();
    }
    timer::program_next_tick();
}
