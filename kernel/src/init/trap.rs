use crate::trap;
use spin::Once;

static TRAP_INIT: Once<()> = Once::new();
pub fn init(hartid: usize, _dtb: *const u8) {
    TRAP_INIT.call_once(|| {
        trap::init();
    });
    trap::init_hart(hartid);
}
