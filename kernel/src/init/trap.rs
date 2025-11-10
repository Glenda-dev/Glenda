use crate::trap::{inittraps, inittraps_hart};
use spin::{Mutex, Once};

static TRAP_INIT: Once<()> = Once::new();
pub fn init(hartid: usize, _dtb: *const u8) {
    TRAP_INIT.call_once(|| {
        inittraps();
    });
    inittraps_hart(hartid);
}
