use crate::drivers::uart;
use spin::Once;

static UART_INIT: Once<()> = Once::new();

pub fn init(_hartid: usize, dtb: *const u8) {
    UART_INIT.call_once(|| {
        uart::initialize_from_dtb(dtb);
    });
}
