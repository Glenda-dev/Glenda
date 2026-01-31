use crate::cpu;
use crate::hal;

pub fn init() {
    cpu::init();
    hal::platform::bootstrap_cpus();
}
