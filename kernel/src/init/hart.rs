use crate::cpu;
use crate::hal;
use crate::hal::platform::PlatformInfo;

pub fn init(cpuid: usize, info: PlatformInfo) {
    cpu::init(cpuid);
    hal::platform::bootstrap_cpus(cpuid, info);
}
