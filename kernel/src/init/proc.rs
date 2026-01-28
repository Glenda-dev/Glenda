use crate::hal::platform::PlatformInfo;
use crate::proc;
use spin::Once;

static PROC_INIT: Once<()> = Once::new();

pub fn init(cpuid: usize, info: PlatformInfo) {
    PROC_INIT.call_once(|| {
        proc::init(cpuid, info);
    });
}
