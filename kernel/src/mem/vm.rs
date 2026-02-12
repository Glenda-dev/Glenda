use super::PGSIZE;
use super::PhysAddr;
use crate::hal;

use crate::mem::PageTable;
use crate::mem::Perms;
use crate::mem::VirtAddr;
use crate::platform;
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

pub static KERNEL_PAGE_TABLE: Once<PageTable> = Once::new();

pub fn init_kernel_vm() {
    let info = platform::get();
    let mut kpt = PageTable::new();
    let mut flags: Perms;

    // 1. 映射所有物理内存 (Identity Mapping)
    // 微内核需要访问所有物理内存来管理 Untyped 资源。
    // 在不使用 HHDM 的情况下，我们直接将所有 RAM 恒等映射。
    for mem in info.memory_regions.iter().filter(|r| r.region_type == platform::MemoryType::Ram) {
        let mem_start_pa = mem.start;
        let mem_start_va = hal::mem::phys_to_virt(mem_start_pa);
        let mem_size = mem.size;
        flags = Perms::READ | Perms::WRITE | Perms::ACCESSED | Perms::DIRTY | Perms::GLOBAL;
        log!(
            "vm: Map RAM [{:#x}, {:#x}) -> [{:#x}, {:#x}) {flags}",
            mem_start_pa.as_usize(),
            (mem_start_pa + mem_size).as_usize(),
            mem_start_va.as_usize(),
            (mem_start_va + mem_size).as_usize()
        );
        kpt.map_with_alloc(mem_start_va, mem_start_pa, mem_size, flags);
    }

    // 2. 重映射内核段以加强权限控制 (覆盖上面的 RW 映射)
    let text_start = PhysAddr::from(unsafe { &__text_start as *const u8 as usize });
    let text_end = PhysAddr::from(unsafe { &__text_end as *const u8 as usize });
    let text_pa = text_start.align_down(PGSIZE);
    let text_va = hal::mem::phys_to_virt(text_pa);
    let text_size = (text_end - text_start).as_usize();
    flags = Perms::READ | Perms::EXECUTE | Perms::ACCESSED | Perms::GLOBAL;
    log!(
        "vm: Map .text [{:#x}, {:#x}) -> [{:#x}, {:#x}) {}",
        text_start.as_usize(),
        text_end.as_usize(),
        text_va.as_usize(),
        (text_va + text_size).as_usize(),
        flags
    );
    kpt.map_with_alloc(text_va, text_pa, text_size, flags);

    let rodata_start = PhysAddr::from(unsafe { &__rodata_start as *const u8 as usize });
    let rodata_end = PhysAddr::from(unsafe { &__rodata_end as *const u8 as usize });
    let rodata_pa = rodata_start.align_down(PGSIZE);
    let rodata_va = hal::mem::phys_to_virt(rodata_pa);
    let rodata_size = (rodata_end - rodata_start).as_usize();
    flags = Perms::READ | Perms::ACCESSED | Perms::GLOBAL;
    log!(
        "vm: Map .rodata [{:#x}, {:#x}) -> [{:#x}, {:#x}) {}",
        rodata_start.as_usize(),
        rodata_end.as_usize(),
        rodata_va.as_usize(),
        (rodata_va + rodata_size).as_usize(),
        flags
    );
    kpt.map_with_alloc(rodata_va, rodata_pa, rodata_size, flags);

    // .data 和 .bss 已经是 RW 了，不需要额外重映射，但为了逻辑完整也可以做

    // 映射initrd
    let initrd = platform::get().initrd;
    let initrd_start = initrd.start;
    let initrd_end = initrd.start + initrd.size;
    let initrd_size = initrd.size;
    let initrd_pa = initrd_start.align_down(PGSIZE);
    let initrd_va = hal::mem::phys_to_virt(initrd_pa);
    log!(
        "vm: Map initrd [{:#x}, {:#x}) -> [{:#x}, {:#x}) {}",
        initrd_start.as_usize(),
        initrd_end.as_usize(),
        initrd_va.as_usize(),
        (initrd_va + initrd_size).as_usize(),
        flags
    );
    kpt.map_with_alloc(initrd_va, initrd_pa, initrd_size, flags);

    hal::mem::kpt_setup(&mut kpt);

    log!("vm: Root page table built");
    KERNEL_PAGE_TABLE.call_once(|| kpt);
}

pub fn switch_to_kernel() {
    let cpuid = hal::cpu::cpu_id();
    let kpt = KERNEL_PAGE_TABLE.get().expect("Kernel page table not initialized");
    let kpt_va = VirtAddr::from(kpt as *const _ as usize);
    let kpt_pa = hal::mem::virt_to_phys(kpt_va);
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
