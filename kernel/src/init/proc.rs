use crate::proc;
use spin::Once;

static PROC_INIT: Once<()> = Once::new();

pub fn init(hartid: usize, dtb: *const u8) {
    PROC_INIT.call_once(|| {
        proc::init(hartid, dtb);
    });
}
