use crate::printk;
use crate::printk::{ANSI_GREEN, ANSI_RESET, ANSI_YELLOW};
use crate::proc;

pub fn run(hartid: usize) {
    if hartid != 0 {
        return;
    }
    printk!("{}[TEST]{} Starting user tests on hart {}\n", ANSI_YELLOW, ANSI_RESET, hartid);
    launch_test_payload();
    printk!("{}[PASS]{} User tests\n", ANSI_GREEN, ANSI_RESET);
}

fn launch_test_payload() {
    printk!("Launching user test payload\n");
    proc::process::init_test();
    printk!("Starting scheduler on hart 0...\n");
    proc::scheduler::scheduler();
}
