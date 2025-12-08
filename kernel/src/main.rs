#![no_std]
#![no_main]

mod dtb;
mod hart;
mod init;
mod irq;
mod logo;
mod mem;
mod printk;
mod proc;
mod sbi;
mod syscall;

#[cfg(feature = "tests")]
mod tests;

use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};
use init::init;
use logo::LOGO;
use printk::{ANSI_BLUE, ANSI_RED, ANSI_RESET};
use riscv::asm::wfi;

include!("../../target/proc_payload.rs");

/*
 为了便捷，M-mode 固件与 M->S 的降权交给 OpenSBI，程序只负责 S-mode 下的内核
 (虽然大概率以后要从头写出来 M-mode 到 S-mode 的切换)

 寄存器约定[1]:
   - $a0 存放当前核的 hartid
   - $a1 存放设备树指针

 [1]: https://www.kernel.org/doc/Documentation/riscv/boot.rst

*/
#[unsafe(no_mangle)]
pub extern "C" fn glenda_main(hartid: usize, dtb: *const u8) -> ! {
    // 解析设备树
    let dtb_result = dtb::init(dtb);

    // 初始化串口驱动
    let uart_cfg = dtb::uart_config().unwrap_or(drivers::uart::DEFAULT_QEMU_VIRT);
    drivers::uart::init(uart_cfg);

    static START_BANNER_PRINTED: AtomicBool = AtomicBool::new(false);

    if START_BANNER_PRINTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        match dtb_result {
            Ok(_) => {
                printk!("Device tree blob at {:p}", dtb);
                printk!(
                    "UART in use: base=0x{:x}, thr=0x{:x}, lsr=0x{:x}",
                    uart_cfg.base,
                    uart_cfg.thr_offset,
                    uart_cfg.lsr_offset
                );
                printk!("{} harts detected", dtb::hart_count());
            }
            Err(err) => {
                printk!("Device tree parsing failed: {:?}", err);
                printk!("Falling back to QEMU-virt default UART @ 0x10000000");
            }
        }
        printk!("{}", LOGO);
        printk!("{}Glenda microkernel booting{}", ANSI_BLUE, ANSI_RESET);
    }

    init(hartid, dtb);
    #[cfg(feature = "tests")]
    {
        tests::test(hartid);
        crate::irq::enable_s();
    }

    if hartid == 0 {
        if HAS_PROC_PAYLOAD && !PROC_PAYLOAD.is_empty() {
            printk!("Creating init process from payload...");
            proc::process::create(PROC_PAYLOAD);
        } else {
            printk!("Creating init process from fallback...");
            // wfi()
            proc::process::create(&[0x6f, 0x00, 0x00, 0x00]);
        }
        printk!("Starting scheduler on hart 0...");
        proc::scheduler::scheduler();
    }

    printk!("{}Hart {} entering main loop{}", ANSI_BLUE, hartid, ANSI_RESET);
    loop {
        wfi();
    }
}

#[inline(always)]
fn fp() -> usize {
    let ptr: usize;
    unsafe {
        core::arch::asm!("mv {}, s0", out(reg) ptr);
    }
    ptr
}

fn backtrace() {
    printk!("--- GLENDA BACKTRACE START ---");
    let mut current_fp = fp();
    let mut depth = 0;
    while current_fp != 0 && depth < 20 {
        // 0(fp) -> saved fp
        // 8(fp) -> saved ra
        unsafe {
            let ra_ptr = (current_fp as *const usize).sub(1);
            let prev_fp_ptr = (current_fp as *const usize).sub(2);

            // TODO: embed more info
            if ra_ptr as usize >= 0x80000000 && prev_fp_ptr as usize >= 0x80000000 {
                 let ra = *ra_ptr;
                 let prev_fp = *prev_fp_ptr;
                 printk!("{:>2}: fp={:#x} ra={:#x}", depth, current_fp, ra);
                 current_fp = prev_fp;
            } else {
                printk!("Invalid fp/ra ptr at {:#x}", current_fp);
                break;
            }
        }
        depth += 1;
    }
    printk!("--- GLENDA BACKTRACE END ---");
}

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    printk!("{}PANIC{}: {}", ANSI_RED, ANSI_RESET, info);
    backtrace();
    loop {
        wfi();
    }
}
