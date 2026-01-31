use crate::platform;
use spin::Once;

static PLATFORM_INIT: Once<()> = Once::new();
pub fn init() {
    PLATFORM_INIT.call_once(|| platform::init());
}
