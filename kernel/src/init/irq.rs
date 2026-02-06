use crate::hal;
use spin::Once;

static IRQ_INIT: Once<()> = Once::new();
pub fn init() {
    IRQ_INIT.call_once(|| {
        hal::irq::init();
    });
    hal::irq::init_cpu();
    hal::timer::init();
}
