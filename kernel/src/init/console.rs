use crate::hal;
use spin::Once;

static CONSOLE_INIT: Once<()> = Once::new();

pub fn init() {
    CONSOLE_INIT.call_once(|| {
        hal::console::init();
    });
}
