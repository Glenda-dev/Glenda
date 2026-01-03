use crate::boot::initrd;
use spin::Once;

static INITRD_INIT: Once<()> = Once::new();
pub fn init(_hartid: usize, _dtb: *const u8) {
    INITRD_INIT.call_once(|| {
        initrd::init();
    });
}
