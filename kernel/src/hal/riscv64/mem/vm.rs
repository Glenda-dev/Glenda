use super::PGSIZE;
use super::virt_to_phys;
use crate::hal::drivers;
use crate::mem::PageTable;
use crate::mem::TRAMPOLINE_VA;
use crate::mem::{Perms, VirtAddr};

unsafe extern "C" {
    static __trampoline: u8;
}

pub fn setup_trampoline(kpt: &mut PageTable) {
    // 3. 映射 Trampoline (高地址)
    // __trampoline 符号在链接脚本中定义，位于内核虚拟地址空间
    // 需要转换为物理地址才能在页表中建立映射
    let tramp_va_sym = VirtAddr::from(unsafe { &__trampoline as *const u8 as usize });
    let tramp_pa = virt_to_phys(tramp_va_sym);

    assert!(tramp_pa.is_aligned(PGSIZE));
    let tramp_va = VirtAddr::from(TRAMPOLINE_VA);
    let flags = Perms::READ | Perms::EXECUTE | Perms::ACCESSED | Perms::GLOBAL;
    log!(
        "hal: Map TRAMPOLINE [{:#x}, {:#x}) -> [{:#x}, {:#x}) {}",
        tramp_pa.as_usize(),
        (tramp_pa + PGSIZE).as_usize(),
        tramp_va.as_usize(),
        (tramp_va + PGSIZE).as_usize(),
        flags
    );
    kpt.map_with_alloc(tramp_va, tramp_pa, PGSIZE, flags);
}

pub fn setup_mmio(kpt: &mut PageTable) {
    log!("hal: Setting up MMIO, kpt at {:p}", kpt);
    // 4. 映射 MMIO (通过已配置的驱动)
    if let Some(uart) = drivers::UART.get() {
        uart.map_mmio(kpt);
    }
    if let Some(intc) = drivers::INTC.get() {
        intc.map_mmio(kpt);
    }
}
