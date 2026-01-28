use crate::hal;
use crate::hal::platform::PlatformInfo;
use spin::Once;

static CONSOLE_INIT: Once<()> = Once::new();

pub fn init(_cpuid: usize, _info: PlatformInfo) {
    CONSOLE_INIT.call_once(|| {
        hal::console::init();
    });
}
