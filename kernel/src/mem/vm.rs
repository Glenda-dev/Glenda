use crate::hal;
use crate::mem::PageTable;
use crate::mem::VirtAddr;
use crate::mem::addr::virt_to_phys;
use crate::sync::Once;

// TODO: HHDM support

// see linker.ld
unsafe extern "C" {
    static __text_start: u8;
    static __text_end: u8;
    static __rodata_start: u8;
    static __rodata_end: u8;
    static __data_start: u8;
    static __data_end: u8;
    static __bss_start: u8;
    static __bss_end: u8;
}

pub static KERNEL_PAGE_TABLE: Once<&mut PageTable> = Once::new();

pub fn init_kernel_vm() {
    let mut kpt = hal::mem::get_pt();
    hal::mem::kpt_setup(&mut kpt);
    log!("vm: Root page table built");
    KERNEL_PAGE_TABLE.call_once(|| kpt);
}

pub fn switch_to_kernel() {
    let cpuid = hal::cpu::cpu_id();
    let kpt = KERNEL_PAGE_TABLE.get().expect("Kernel page table not initialized");
    let kpt_va = VirtAddr::from(kpt as *const _ as usize);
    let kpt_pa = virt_to_phys(kpt_va);
    let reg = hal::mem::get_mmu_register(kpt_pa, 0);
    unsafe {
        hal::mem::activate_vspace(reg);
    }
    log!("vm: CPU {} switched to kernel page table", cpuid);
}

pub fn switch_off() {
    let cpuid = hal::cpu::cpu_id();
    unsafe {
        hal::mem::deactivate_vspace();
    }
    log!("vm: CPU {} switching off vm", cpuid);
}
