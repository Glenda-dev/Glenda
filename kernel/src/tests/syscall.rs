use crate::printk;
use crate::printk::{ANSI_RESET, ANSI_YELLOW};
use crate::proc::process::launch_payload;

// 由 build.rs 生成，这个文件可能为空
// FIXME: 由于 xtask 构建流程的问题，暂时无法使用 OUT_DIR
include!("../../../target/proc_payload.rs");

// 故提供一个 Fallback example
static USER_INIT_CODE: [u8; 20] = [
    0x93, 0x08, 0x10, 0x00, 0x73, 0x00, 0x00, 0x00, 0x93, 0x08, 0x10, 0x00, 0x73, 0x00, 0x00, 0x00,
    0x6f, 0x00, 0x00, 0x00,
];

pub fn run(hartid: usize) {
    if hartid != 0 {
        return;
    }
    printk!("{}[TEST]{} Starting syscall tests on hart {}", ANSI_YELLOW, ANSI_RESET, hartid);
    launch_test_proc();
    //printk!("{}[PASS]{} Syscall tests", ANSI_GREEN, ANSI_RESET);
}

fn launch_test_proc() -> ! {
    if HAS_PROC_PAYLOAD && !PROC_PAYLOAD.is_empty() {
        printk!("Launching external test payload");
        launch_payload(&PROC_PAYLOAD);
    } else {
        printk!("Launching internal test payload");
        launch_payload(&USER_INIT_CODE);
    }
}
