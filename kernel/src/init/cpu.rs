use crate::cpu;

pub fn init() {
    cpu::init();
    crate::hal::virt::init_cpu();
}
