use crate::irq;
use crate::sync::Once;

static IRQ_INIT: Once<()> = Once::new();
pub fn init() {
    IRQ_INIT.call_once(|| {
        irq::init();
    });
    irq::init_cpu();
}
