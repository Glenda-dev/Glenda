use super::super::dtb;
use super::PGSIZE;
use super::phys_to_virt;
use crate::mem::PageTable;
use crate::mem::TRAMPOLINE_VA;
use crate::mem::{Perms, PhysAddr, VirtAddr};

unsafe extern "C" {
    static __trampoline: u8;
}

pub fn setup_trampoline(kpt: &mut PageTable) {
    // 3. 映射 Trampoline (高地址)
    let tramp_pa = PhysAddr::from(unsafe { &__trampoline as *const u8 as usize });
    assert!(tramp_pa.is_aligned(PGSIZE));
    let tramp_va = VirtAddr::from(TRAMPOLINE_VA);
    let flags = Perms::READ | Perms::EXECUTE | Perms::ACCESSED | Perms::GLOBAL;
    log!(
        "vm: Map TRAMPOLINE [{:#x}, {:#x}) -> [{:#x}, {:#x}) {}\n",
        tramp_pa.as_usize(),
        (tramp_pa + PGSIZE).as_usize(),
        tramp_va.as_usize(),
        (tramp_va + PGSIZE).as_usize(),
        flags
    );
    kpt.map_with_alloc(tramp_va, tramp_pa, PGSIZE, flags);
}

pub fn setup_mmio(kpt: &mut PageTable) {
    // 4. 映射 MMIO (UART, PLIC)
    match dtb::uart_config() {
        Some(uart) => {
            let uart_base = PhysAddr::from(uart.base);
            let uart_pa = uart_base.align_down(PGSIZE);
            let uart_va = phys_to_virt(uart_pa);
            let flags = Perms::READ | Perms::WRITE | Perms::ACCESSED | Perms::DIRTY | Perms::GLOBAL;
            log!(
                "vm: Map UART [{:#x}, {:#x}) -> [{:#x}, {:#x}) {}\n",
                uart_base.as_usize(),
                (uart_base + PGSIZE).as_usize(),
                uart_va.as_usize(),
                (uart_va + PGSIZE).as_usize(),
                flags
            );
            kpt.map_with_alloc(uart_va, uart_pa, PGSIZE, flags);
        }
        None => {
            log!("vm: No UART found in DTB, skipping MMIO mapping");
        }
    }
    match dtb::plic() {
        None => {
            log!("vm: No PLIC found in DTB, skipping MMIO mapping");
            return;
        }
        Some(plic_range) => {
            let plic_pa = plic_range.start;
            let plic_va = phys_to_virt(plic_pa);
            let plic_size = plic_range.size;
            let flags = Perms::READ | Perms::WRITE | Perms::ACCESSED | Perms::DIRTY | Perms::GLOBAL;
            log!(
                "vm: Map PLIC [{:#x}, {:#x}) -> [{:#x}, {:#x}) {}\n",
                plic_pa.as_usize(),
                (plic_pa + plic_size).as_usize(),
                plic_va.as_usize(),
                (plic_va + plic_size).as_usize(),
                flags
            );
            // 映射整个 PLIC 区域
            kpt.map_with_alloc(plic_va, plic_pa, plic_size, flags);
        }
    }
}
