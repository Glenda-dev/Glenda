use super::super::dtb;
use super::PGSIZE;
use super::{PageTable, PteFlags, PtePerms};
use crate::mem::TRAMPOLINE_VA;
use crate::mem::{PhysAddr, VirtAddr};
use crate::printk;

unsafe extern "C" {
    static __trampoline: u8;
}

pub fn setup_trampoline(kpt: &mut PageTable) {
    // 3. 映射 Trampoline (高地址)
    let tramp_pa = PhysAddr::from(unsafe { &__trampoline as *const u8 as usize });
    assert!(tramp_pa.is_aligned(PGSIZE));
    let tramp_va = VirtAddr::from(TRAMPOLINE_VA);
    printk!(
        "vm: Map TRAMPOLINE [{:#x}, {:#x}) -> [{:#x}, {:#x}) RX\n",
        tramp_pa.as_usize(),
        (tramp_pa + PGSIZE).as_usize(),
        tramp_va.as_usize(),
        (tramp_va + PGSIZE).as_usize()
    );
    kpt.map_with_alloc(
        tramp_va,
        tramp_pa,
        PGSIZE,
        PteFlags::from(PtePerms::READ | PtePerms::EXECUTE | PtePerms::ACCESSED | PtePerms::GLOBAL),
    );
}

pub fn setup_mmio(kpt: &mut PageTable) {
    // 4. 映射 MMIO (UART, PLIC)
    match dtb::uart_config() {
        Some(uart) => {
            let uart_base = PhysAddr::from(uart.base);
            let uart_pa = uart_base.align_down(PGSIZE);
            let uart_va = uart_pa.to_va();
            printk!(
                "vm: Map UART [{:#x}, {:#x}) -> [{:#x}, {:#x}) RW\n",
                uart_base.as_usize(),
                (uart_base + PGSIZE).as_usize(),
                uart_va.as_usize(),
                (uart_va + PGSIZE).as_usize()
            );
            kpt.map_with_alloc(
                uart_va,
                uart_pa,
                PGSIZE,
                PteFlags::from(
                    PtePerms::READ
                        | PtePerms::WRITE
                        | PtePerms::ACCESSED
                        | PtePerms::DIRTY
                        | PtePerms::GLOBAL,
                ),
            );
        }
        None => {
            printk!("vm: No UART found in DTB, skipping MMIO mapping\n");
        }
    }
    match dtb::plic() {
        None => {
            printk!("vm: No PLIC found in DTB, skipping MMIO mapping\n");
            return;
        }
        Some(plic_range) => {
            let plic_pa = plic_range.start;
            let plic_va = plic_pa.to_va();
            let plic_size = plic_range.size;
            printk!(
                "vm: Map PLIC [{:#x}, {:#x}) -> [{:#x}, {:#x}) RW\n",
                plic_pa.as_usize(),
                (plic_pa + plic_size).as_usize(),
                plic_va.as_usize(),
                (plic_va + plic_size).as_usize()
            );
            // 映射整个 PLIC 区域
            kpt.map_with_alloc(
                plic_va,
                plic_pa,
                plic_size,
                PteFlags::from(
                    PtePerms::READ
                        | PtePerms::WRITE
                        | PtePerms::ACCESSED
                        | PtePerms::DIRTY
                        | PtePerms::GLOBAL,
                ),
            );
        }
    }
}
