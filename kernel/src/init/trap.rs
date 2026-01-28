use crate::hal::platform::PlatformInfo;
use crate::trap;

pub fn init(cpuid: usize, _info: PlatformInfo) {
    trap::init_hart(cpuid);
}
