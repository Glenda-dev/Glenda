use crate::boot;
use crate::hal;
use crate::hal::mem::PGSIZE;
use crate::mem::addr::virt_to_phys;
use crate::mem::{PageTable, Perms, VirtAddr};
use crate::sync::SpinLock;

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

pub static KERNEL_PAGE_TABLE: SpinLock<PageTable> = SpinLock::new(PageTable::new());

pub fn init_kernel_vm() {
    let mut kpt = KERNEL_PAGE_TABLE.lock();
    let hhdm_offset = crate::boot::get_hhdm();
    log!("vm: Initializing kernel VM at {:#x}", kpt.paddr().as_usize());
    // 映射整个 RAM 区域
    let regions = crate::boot::get_mem_map();
    log!("vm: Mapping RAM regions ({} entries) with HHDM offset {:#x}", regions.len(), hhdm_offset);
    for region in regions {
        if region.kind == crate::platform::MemoryType::Ram {
            let pa = region.base;
            let size = region.length;

            let hhdm_va = VirtAddr::from(pa.as_usize() + hhdm_offset);
            kpt.map_with_alloc(hhdm_va, pa, size, Perms::READ | Perms::WRITE | Perms::VALID);
        }
    }

    // 细化内核段权限
    let text_start =
        VirtAddr::from(unsafe { &__text_start as *const u8 as usize }).align_down(PGSIZE);
    let text_end = VirtAddr::from(unsafe { &__text_end as *const u8 as usize }).align_up(PGSIZE);
    let rodata_start =
        VirtAddr::from(unsafe { &__rodata_start as *const u8 as usize }).align_down(PGSIZE);
    let rodata_end =
        VirtAddr::from(unsafe { &__rodata_end as *const u8 as usize }).align_up(PGSIZE);
    let data_start =
        VirtAddr::from(unsafe { &__data_start as *const u8 as usize }).align_down(PGSIZE);
    let bss_end = VirtAddr::from(unsafe { &__bss_end as *const u8 as usize }).align_up(PGSIZE);

    log!("vm: Mapping kernel sections...");

    // .text: Read + Execute
    kpt.map_with_alloc(
        text_start,
        virt_to_phys(text_start),
        (text_end - text_start).as_usize(),
        Perms::READ | Perms::EXECUTE | Perms::VALID,
    );

    // .rodata: Read-only
    kpt.map_with_alloc(
        rodata_start,
        virt_to_phys(rodata_start),
        (rodata_end - rodata_start).as_usize(),
        Perms::READ | Perms::VALID,
    );

    // .data & .bss: Read + Write
    kpt.map_with_alloc(
        data_start,
        virt_to_phys(data_start),
        (bss_end - data_start).as_usize(),
        Perms::READ | Perms::WRITE | Perms::VALID,
    );

    log!("vm: Mapping initrd...");
    let (initrd_start_va, initrd_size) = boot::get_initrd().expect("Failed to get initrd info");
    let initrd_start_pa = virt_to_phys(initrd_start_va);
    kpt.map_with_alloc(initrd_start_va, initrd_start_pa, initrd_size, Perms::READ | Perms::VALID);

    hal::mem::kpt_setup(&mut *kpt);
}

pub fn switch_to_kernel() {
    let cpuid = hal::cpu::cpu_id();
    let kpt_pa = KERNEL_PAGE_TABLE.lock().paddr();
    let reg = hal::mem::get_mmu_register(kpt_pa, 0);

    log!("vm: CPU {} switching to kernel page table with mmu {:#x}", cpuid, reg); // 在 vm.rs 的 switch_to_kernel 中添加

    unsafe {
        hal::mem::activate_vspace(reg);
    }
}

pub fn switch_off() {
    let cpuid = hal::cpu::cpu_id();
    unsafe {
        hal::mem::deactivate_vspace();
    }
    log!("vm: CPU {} switching off vm", cpuid);
}
