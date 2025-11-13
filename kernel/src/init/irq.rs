use crate::irq;
use spin::Once;

static IRQ_INIT: Once<()> = Once::new();
pub fn init(hartid: usize, _dtb: *const u8) {
    IRQ_INIT.call_once(|| {
        irq::init();
    });
    irq::init_hart(hartid);
}
