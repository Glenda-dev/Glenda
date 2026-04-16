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

    let initrd_info = boot::get_initrd().expect("Failed to get initrd info");

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
    log!("vm: Updating kernel section permissions...");

    // .text: Read + Execute
    kpt.update(
        text_start,
        (text_end - text_start).as_usize(),
        Perms::READ | Perms::EXECUTE | Perms::VALID,
    )
    .expect("vm: failed to update .text permissions");

    // .rodata: Read-only
    kpt.update(
        rodata_start,
        (rodata_end - rodata_start).as_usize(),
        Perms::READ | Perms::VALID,
    )
    .expect("vm: failed to update .rodata permissions");

    // .data & .bss: Read + Write
    kpt.update(
        data_start,
        (bss_end - data_start).as_usize(),
        Perms::READ | Perms::WRITE | Perms::VALID,
    )
    .expect("vm: failed to update .data/.bss permissions");

    log!("vm: Mapping initrd backing...");
    let (initrd_start_va, initrd_size) = initrd_info;
    let initrd_start = initrd_start_va.align_down(PGSIZE);
    let initrd_end = (initrd_start_va + initrd_size).align_up(PGSIZE);
    let initrd_map_size = (initrd_end - initrd_start).as_usize();
    kpt.map_with_alloc(
        initrd_start,
        virt_to_phys(initrd_start),
        initrd_map_size,
        Perms::READ | Perms::WRITE | Perms::VALID,
    );

    log!("vm: Updating initrd permissions...");
    kpt.update(initrd_start, initrd_map_size, Perms::READ | Perms::VALID)
        .expect("vm: failed to update initrd permissions");

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
