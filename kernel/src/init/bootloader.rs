use crate::bootloader::initrd;
use spin::Once;

static BL_INIT: Once<()> = Once::new();
pub fn init(_hartid: usize, _dtb: *const u8) {
    BL_INIT.call_once(|| {
        initrd::init();
    });
}
