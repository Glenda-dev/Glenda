use crate::hal;
use crate::hal::platform::PlatformInfo;
use spin::Once;

static PLATFORM_INIT: Once<()> = Once::new();

pub fn init(_cpuid: usize, info: PlatformInfo) {
    PLATFORM_INIT.call_once(|| {
        hal::platform::init(info);
    });
}
