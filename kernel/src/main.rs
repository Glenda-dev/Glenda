#![no_std]
#![no_main]

mod dtb;
mod init;
mod logo;
mod mem;
mod printk;
mod trap;

#[cfg(feature = "tests")]
mod tests;

use core::panic::PanicInfo;
use init::init;
use logo::LOGO;
use printk::{ANSI_BLUE, ANSI_RED, ANSI_RESET};
use riscv::asm::wfi;

#[cfg(feature = "tests")]
use tests::test;

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
    let uart_cfg = dtb::uart_config().unwrap_or(driver_uart::DEFAULT_QEMU_VIRT);
    driver_uart::init(uart_cfg);

    // 启动信息
    if hartid == 0 {
        match dtb_result {
            Ok(_) => {
                printk!("Device tree blob at {:p}", dtb);
                printk!(
                    "UART in use: base=0x{:x}, thr=0x{:x}, lsr=0x{:x}",
                    uart_cfg.base(),
                    uart_cfg.thr_offset(),
                    uart_cfg.lsr_offset()
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
        test(hartid);
    }
    printk!("{}Hart {} entering main loop{}", ANSI_BLUE, hartid, ANSI_RESET);
    loop {
        wfi();
    }
}

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    printk!("{}PANIC{}: {}", ANSI_RED, ANSI_RESET, info);
    loop {
        wfi();
    }
}
