use super::asm;

#[inline(always)]
fn fp() -> usize {
    asm::read_fp()
}

pub fn backtrace() {
    printk_unsynced!("--- GLENDA BACKTRACE START (aarch64) ---\n");
    let mut current_fp = fp();
    let mut depth = 0;
    while current_fp != 0 && depth < 20 {
        if current_fp % 16 != 0 {
            break;
        }
        unsafe {
            let prev_fp = *(current_fp as *const usize);
            let ra = *((current_fp + 8) as *const usize);
            printk_unsynced!("{:>2}: fp={:#x} ra={:#x}\n", depth, current_fp, ra);
            if prev_fp != 0 && prev_fp <= current_fp {
                break;
            }
            current_fp = prev_fp;
        }
        depth += 1;
    }
    printk_unsynced!("--- GLENDA BACKTRACE END ---\n");
}
