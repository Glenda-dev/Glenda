use crate::platform;
use crate::sync::Once;

static PLATFORM_INIT: Once<()> = Once::new();
pub fn init() {
    PLATFORM_INIT.call_once(|| platform::init());
}
