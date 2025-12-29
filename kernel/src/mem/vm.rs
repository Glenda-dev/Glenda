use super::PGSIZE;
use super::pte::perms;
use super::{PageTable, PhysAddr, PteFlags, VirtAddr};
use crate::dtb;
use crate::printk;
use crate::printk::uart;
use crate::trap::vector;
use riscv::asm::sfence_vma_all;
use riscv::register::satp;
use spin::Once;

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

pub fn init_kernel_vm(hartid: usize) {
    let mut kpt = PageTable::new();

    // 1. 映射所有物理内存 (Identity Mapping)
    // 微内核需要访问所有物理内存来管理 Untyped 资源。
    // 在不使用 HHDM 的情况下，我们直接将所有 RAM 恒等映射。
    let mem = dtb::memory_range().expect("Memory range not found in DTB");
    let mem_start_pa = mem.start;
    let mem_start_va = mem_start_pa.to_va();
    let mem_size = mem.size;
    printk!(
        "vm: Map RAM [{:#x}, {:#x}) -> [{:#x}, {:#x}) RW\n",
        mem_start_pa.as_usize(),
        (mem_start_pa + mem_size).as_usize(),
        mem_start_va.as_usize(),
        (mem_start_va + mem_size).as_usize()
    );
    kpt.map_with_alloc(
        mem_start_va,
        mem_start_pa,
        mem_size,
        PteFlags::from(perms::READ | perms::WRITE | perms::ACCESSED | perms::DIRTY | perms::GLOBAL),
    );

    // 2. 重映射内核段以加强权限控制 (覆盖上面的 RW 映射)
    let text_start = PhysAddr::from(unsafe { &__text_start as *const u8 as usize });
    let text_end = PhysAddr::from(unsafe { &__text_end as *const u8 as usize });
    let text_pa = text_start.align_down(PGSIZE);
    let text_va = text_pa.to_va();
    let text_size = (text_end - text_start).as_usize();
    printk!(
        "vm: Map .text [{:#x}, {:#x}) -> [{:#x}, {:#x}) RX\n",
        text_start.as_usize(),
        text_end.as_usize(),
        text_va.as_usize(),
        (text_va + text_size).as_usize()
    );
    kpt.map_with_alloc(
        text_va,
        text_pa,
        text_size,
        PteFlags::from(perms::READ | perms::EXECUTE | perms::ACCESSED | perms::GLOBAL),
    );

    let rodata_start = PhysAddr::from(unsafe { &__rodata_start as *const u8 as usize });
    let rodata_end = PhysAddr::from(unsafe { &__rodata_end as *const u8 as usize });
    let rodata_pa = rodata_start.align_down(PGSIZE);
    let rodata_va = rodata_pa.to_va();
    let rodata_size = (rodata_end - rodata_start).as_usize();
    printk!(
        "vm: Map .rodata [{:#x}, {:#x}) -> [{:#x}, {:#x}) R\n",
        rodata_start.as_usize(),
        rodata_end.as_usize(),
        rodata_va.as_usize(),
        (rodata_va + rodata_size).as_usize()
    );
    kpt.map_with_alloc(
        rodata_va,
        rodata_pa,
        rodata_size,
        PteFlags::from(perms::READ | perms::ACCESSED | perms::GLOBAL),
    );

    // .data 和 .bss 已经是 RW 了，不需要额外重映射，但为了逻辑完整也可以做

    // 3. 映射 Trampoline (高地址)
    let tramp_pa = PhysAddr::from(vector::user_vector as usize).align_down(PGSIZE);
    let tramp_va = VirtAddr::from(VirtAddr::max().as_usize() - PGSIZE);
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
        PteFlags::from(perms::READ | perms::EXECUTE | perms::ACCESSED | perms::GLOBAL),
    );

    // 4. 映射 MMIO (UART, PLIC)
    let uart_base = PhysAddr::from(dtb::uart_config().unwrap_or(uart::DEFAULT_QEMU_VIRT).base);
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
        PteFlags::from(perms::READ | perms::WRITE | perms::ACCESSED | perms::DIRTY | perms::GLOBAL),
    );

    if let Some(plic_range) = dtb::plic() {
        let plic_pa = plic_range.start;
        let plic_va = plic_pa.to_va();
        let plic_size = plic_range.size;
        printk!(
            "vm: Map PLIC [{:#x}, {:#x}) -> [{:#x}, {:#x}) RW\n",
            plic_pa.as_usize(),
            (plic_pa + plic_range.size).as_usize(),
            plic_va.as_usize(),
            (plic_va + plic_size).as_usize()
        );
        // 映射整个 PLIC 区域 (简化处理，映射 4MB)
        kpt.map_with_alloc(
            plic_va,
            plic_pa,
            plic_size,
            PteFlags::from(
                perms::READ | perms::WRITE | perms::ACCESSED | perms::DIRTY | perms::GLOBAL,
            ),
        );
    }

    // 映射initrd
    if let Some(initrd) = dtb::initrd_range() {
        let initrd_start = initrd.start;
        let initrd_end = initrd.start + initrd.size;
        let initrd_size = initrd.size;
        let initrd_pa = initrd_start.align_down(PGSIZE);
        let initrd_va = initrd_pa.to_va();
        printk!(
            "vm: Map initrd [{:#x}, {:#x}) -> [{:#x}, {:#x}) R\n",
            initrd_start.as_usize(),
            initrd_end.as_usize(),
            initrd_va.as_usize(),
            (initrd_va + initrd_size).as_usize()
        );
        kpt.map_with_alloc(
            initrd_va,
            initrd_pa,
            initrd_size,
            PteFlags::from(perms::READ | perms::ACCESSED | perms::GLOBAL),
        );
    }

    printk!("vm: Root page table built by hart {}\n", hartid);
    KERNEL_PAGE_TABLE.call_once(|| kpt);
}

pub fn switch_to_kernel(hartid: usize) {
    let root_ppn = {
        let kpt = KERNEL_PAGE_TABLE.get().expect("Kernel page table not initialized");
        (&*kpt as *const PageTable as usize) >> 12
    };
    // set SATP to the new page table in Sv39 mode (ASID=0)
    unsafe {
        satp::set(satp::Mode::Sv39, 0, root_ppn);
        sfence_vma_all();
    }
    printk!("vm: Hart {} switched to kernel page table\n", hartid);
}

pub fn switch_off(hartid: usize) {
    unsafe {
        satp::set(satp::Mode::Bare, 0, 0);
        sfence_vma_all();
    }
    printk!("vm: Hart {} switching off vm\n", hartid);
}
