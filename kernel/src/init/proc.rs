use crate::proc;
use spin::Once;

static PROC_INIT: Once<()> = Once::new();

pub fn init() {
    PROC_INIT.call_once(|| {
        proc::init();
    });
}
