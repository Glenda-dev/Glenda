use crate::proc;
use spin::Once;

static PROC_INIT: Once<()> = Once::new();

pub fn init(_hartid: usize, _dtb: *const u8) {
    PROC_INIT.call_once(|| {
        proc::init();
    });
}
