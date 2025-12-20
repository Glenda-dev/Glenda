use super::PGSIZE;
use super::pte;
use super::pte::{PTE_A, PTE_D, PTE_R, PTE_V, PTE_W, PTE_X, pa_to_pte, pte_to_pa};
use super::{PageTable, PhysAddr, PhysFrame, VirtAddr};
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

/// 启动阶段的映射辅助函数
///
/// 由于 PageTable::map 不再负责自动分配中间页表，我们需要在 Boot 阶段
/// 手动处理页表页的分配。此时使用的是启动分配器。
unsafe fn boot_map(
    kpt: &mut PageTable,
    va_start: VirtAddr,
    pa_start: PhysAddr,
    size: usize,
    flags: usize,
) {
    let start = va_start.align_down(PGSIZE);
    let end = (va_start + size).align_up(PGSIZE);

    let mut va = start;
    let mut pa = pa_start.align_down(PGSIZE);
    while va < end {
        // 手动遍历页表，如果中间层级缺失则分配
        let mut table = kpt as *mut PageTable;
        for level in (1..3).rev() {
            let idx = va.vpn()[level].as_usize();
            let entry = unsafe { &mut (*table).entries[idx] };

            if !pte::is_valid(*entry) {
                // 分配新的页表页
                let frame = PhysFrame::alloc().expect("Boot OOM: Failed to allocate page table");
                // pmem::allocate 已经清零了内存，这里不需要再次 zero()
                // 必须 leak，否则 frame 在作用域结束时会被释放，导致页表损坏
                let frame_pa = frame.leak();

                // 建立中间层级映射 (V=1, 无 R/W/X)
                *entry = pa_to_pte(frame_pa, PTE_V);
            }

            // 进入下一级
            let next_pa = pte_to_pa(*entry);
            // 在恒等映射模式下，物理地址即为内核虚拟地址
            let next_va = next_pa.to_va();
            table = next_va.as_mut::<PageTable>();
        }

        // 设置最后一级 PTE
        let idx = va.vpn()[0].as_usize();

        let pte_ptr = unsafe { &mut (*table).entries[idx] };
        // 允许重映射，因为 init_kernel_vm 会先映射整个 RAM 再细化内核段权限
        *pte_ptr = pa_to_pte(pa, flags | PTE_V);
        va += PGSIZE;
        pa += PGSIZE;
    }
}

pub fn init_kernel_vm(hartid: usize) {
    let mut kpt = PageTable::new();

    // 1. 映射所有物理内存 (Identity Mapping)
    // 微内核需要访问所有物理内存来管理 Untyped 资源。
    // 在不使用 HHDM 的情况下，我们直接将所有 RAM 恒等映射。
    let mem = dtb::memory_range().expect("Memory range not found in DTB");
    printk!("vm: Identity Map RAM [{:#x}, {:#x})\n", mem.start, mem.start + mem.size);
    let mem_start = PhysAddr::from(mem.start);
    unsafe {
        boot_map(&mut kpt, mem_start.to_va(), mem_start, mem.size, PTE_R | PTE_W | PTE_A | PTE_D);
    }

    // 2. 重映射内核段以加强权限控制 (覆盖上面的 RW 映射)
    let text_start = PhysAddr::from(unsafe { &__text_start as *const u8 as usize });
    let text_end = PhysAddr::from(unsafe { &__text_end as *const u8 as usize });
    printk!("vm: Remap .text RX\n");
    unsafe {
        boot_map(
            &mut kpt,
            text_start.to_va(),
            text_start,
            (text_end - text_start).as_usize(),
            PTE_R | PTE_X | PTE_A,
        );
    }

    let rodata_start = PhysAddr::from(unsafe { &__rodata_start as *const u8 as usize });
    let rodata_end = PhysAddr::from(unsafe { &__rodata_end as *const u8 as usize });
    printk!("vm: Remap .rodata R\n");
    unsafe {
        boot_map(
            &mut kpt,
            rodata_start.to_va(),
            rodata_start,
            (rodata_end - rodata_start).as_usize(),
            PTE_R | PTE_A,
        );
    }

    // .data 和 .bss 已经是 RW 了，不需要额外重映射，但为了逻辑完整也可以做

    // 3. 映射 Trampoline (高地址)
    let tramp_pa = PhysAddr::from(vector::trampoline as usize).align_down(PGSIZE);
    let tramp_va = VirtAddr::from(super::VA_MAX - super::PGSIZE);
    printk!("vm: Map TRAMPOLINE\n");
    unsafe {
        boot_map(&mut kpt, tramp_va, tramp_pa, PGSIZE, PTE_R | PTE_X | PTE_A);
    }

    // 4. 映射 MMIO (UART, PLIC)
    let uart_base = PhysAddr::from(dtb::uart_config().unwrap_or(uart::DEFAULT_QEMU_VIRT).base);
    printk!("vm: Map UART\n");
    unsafe {
        boot_map(&mut kpt, uart_base.to_va(), uart_base, PGSIZE, PTE_R | PTE_W | PTE_A | PTE_D);
    }

    if let Some(plic_base) = dtb::plic_base() {
        let plic_pa = PhysAddr::from(plic_base);
        printk!("vm: Map PLIC\n");
        // 映射整个 PLIC 区域 (简化处理，映射 4MB)
        unsafe {
            boot_map(&mut kpt, plic_pa.to_va(), plic_pa, 0x3000, PTE_R | PTE_W | PTE_A | PTE_D);
        }
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
