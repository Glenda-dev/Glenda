use crate::hal::platform::PlatformInfo;
use crate::initrd;
use spin::Once;

static INITRD_INIT: Once<()> = Once::new();
pub fn init(_cpuid: usize, _info: PlatformInfo) {
    INITRD_INIT.call_once(|| {
        initrd::init();
    });
}
