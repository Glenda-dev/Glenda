use crate::printk;
use core::arch::asm;

#[inline(always)]
fn fp() -> usize {
    let ptr: usize;
    unsafe {
        asm!("mv {}, s0", out(reg) ptr);
    }
    ptr
}

pub fn backtrace() {
    printk!("\n--- GLENDA BACKTRACE START ---\n");
    let mut current_fp = fp();
    let mut depth = 0;
    while current_fp != 0 && depth < 20 {
        // [修复] 检查 fp 对齐，防止读取垃圾值导致的非对齐访问 Panic
        if current_fp % 8 != 0 {
            printk!("Invalid unaligned fp: {:#x}\n", current_fp);
            break;
        }

        // 0(fp) -> saved fp
        // 8(fp) -> saved ra
        unsafe {
            let ra_ptr = (current_fp as *const usize).sub(1);
            let prev_fp_ptr = (current_fp as *const usize).sub(2);

            // TODO: embed more info
            if ra_ptr as usize >= 0x80000000 && prev_fp_ptr as usize >= 0x80000000 {
                let ra = *ra_ptr;
                let prev_fp = *prev_fp_ptr;
                printk!("{:>2}: fp={:#x} ra={:#x}\n", depth, current_fp, ra);
                current_fp = prev_fp;
            } else {
                printk!("Invalid fp/ra ptr at {:#x}\n", current_fp);
                break;
            }
        }
        depth += 1;
    }
    printk!("--- GLENDA BACKTRACE END ---\n");
}
