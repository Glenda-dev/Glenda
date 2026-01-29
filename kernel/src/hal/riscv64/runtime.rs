use crate::printk_unsynced;
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
    printk_unsynced!("--- GLENDA BACKTRACE START ---\n");
    let mut current_fp = fp();
    let mut depth = 0;
    while current_fp != 0 && depth < 20 {
        // [修复] 检查 fp 对齐，防止读取垃圾值导致的非对齐访问 Panic
        if current_fp % 8 != 0 {
            printk_unsynced!("Invalid unaligned fp: {:#x}\n", current_fp);
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
                printk_unsynced!("{:>2}: fp={:#x} ra={:#x}\n", depth, current_fp, ra);

                // [新增] 严格校验下一个帧指针，防止缺页导致的递归Panic
                if prev_fp != 0 {
                    // 1. 栈回溯时地址应递增 (Caller Stack Frame is higher)
                    if prev_fp <= current_fp {
                        printk_unsynced!(
                            "Stack corruption: Loop or wrong direction (prev_fp {:#x} <= fp)\n",
                            prev_fp
                        );
                        break;
                    }
                    // 2. 检查栈帧跨度是否过大 (例如 > 1MB 可能是读到了垃圾值)
                    if prev_fp - current_fp > 0x10_0000 {
                        printk_unsynced!(
                            "Stack corruption: Frame too large (delta {:#x})\n",
                            prev_fp - current_fp
                        );
                        break;
                    }
                }

                current_fp = prev_fp;
            } else {
                printk_unsynced!("Invalid fp/ra ptr at {:#x}\n", current_fp);
                break;
            }
        }
        depth += 1;
    }
    printk_unsynced!("--- GLENDA BACKTRACE END ---\n");
}
