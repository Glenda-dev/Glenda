use crate::hal;
use crate::hal::platform::PlatformInfo;
use crate::irq;
use spin::Once;

static IRQ_INIT: Once<()> = Once::new();
pub fn init(hartid: usize, _info: PlatformInfo) {
    IRQ_INIT.call_once(|| {
        hal::irq::init();
    });
    hal::irq::init_cpu(hartid);
    irq::timer::start(hartid);
}
