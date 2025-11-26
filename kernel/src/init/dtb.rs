use crate::dtb;
use spin::Once;

static DTB_INIT: Once<()> = Once::new();

pub fn init(_hartid: usize, dtb: *const u8) {
    DTB_INIT.call_once(|| {
        dtb::init(dtb);
    });
}
