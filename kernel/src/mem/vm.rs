use super::PGSIZE;
use super::addr;
use super::addr::{align_down, align_up, vpn};
use super::pte;
use super::pte::{PTE_A, PTE_D, PTE_R, PTE_V, PTE_W, PTE_X, pa_to_pte, pte_to_pa};
use super::{PageTable, PhysAddr, PhysFrame, VirtAddr};
use crate::dtb;
use crate::irq::vector;
use crate::printk;
use crate::printk::uart;
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
    va_start: usize,
    pa_start: usize,
    size: usize,
    flags: usize,
) {
    let start = align_down(va_start);
    let end = align_up(va_start + size);

    let mut va = start;
    let mut pa = align_down(pa_start);

    while va < end {
        // 手动遍历页表，如果中间层级缺失则分配
        let mut table = kpt as *mut PageTable;
        for level in (1..3).rev() {
            let idx = vpn(VirtAddr::from(va))[level];
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
            let next_va = addr::phys_to_virt(PhysAddr::from(next_pa));
            table = unsafe { &mut *(next_va as *mut PageTable) };
        }

        // 设置最后一级 PTE
        let idx = vpn(VirtAddr::from(va))[0];

        let pte_ptr = unsafe { &mut (*table).entries[idx] };
        // 允许重映射，因为 init_kernel_vm 会先映射整个 RAM 再细化内核段权限
        *pte_ptr = pa_to_pte(PhysAddr::from(pa), flags | PTE_V);

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
    printk!("VM: Identity Map RAM [{:#x}, {:#x})\n", mem.start, mem.start + mem.size);
    unsafe {
        boot_map(&mut kpt, mem.start, mem.start, mem.size, PTE_R | PTE_W | PTE_A | PTE_D);
    }

    // 2. 重映射内核段以加强权限控制 (覆盖上面的 RW 映射)
    let text_start = unsafe { &__text_start as *const u8 as usize };
    let text_end = unsafe { &__text_end as *const u8 as usize };
    printk!("VM: Remap .text RX\n");
    unsafe {
        boot_map(&mut kpt, text_start, text_start, text_end - text_start, PTE_R | PTE_X | PTE_A);
    }

    let rodata_start = unsafe { &__rodata_start as *const u8 as usize };
    let rodata_end = unsafe { &__rodata_end as *const u8 as usize };
    printk!("VM: Remap .rodata R\n");
    unsafe {
        boot_map(&mut kpt, rodata_start, rodata_start, rodata_end - rodata_start, PTE_R | PTE_A);
    }

    // .data 和 .bss 已经是 RW 了，不需要额外重映射，但为了逻辑完整也可以做

    // 3. 映射 Trampoline (高地址)
    let tramp_pa = align_down(vector::trampoline as usize);
    let tramp_va = super::VA_MAX - super::PGSIZE;
    printk!("VM: Map TRAMPOLINE\n");
    unsafe {
        boot_map(&mut kpt, tramp_va, tramp_pa, PGSIZE, PTE_R | PTE_X | PTE_A);
    }

    // 4. 映射 MMIO (UART, PLIC)
    let uart_base = dtb::uart_config().unwrap_or(uart::DEFAULT_QEMU_VIRT).base;
    printk!("VM: Map UART\n");
    unsafe {
        boot_map(&mut kpt, uart_base, uart_base, PGSIZE, PTE_R | PTE_W | PTE_A | PTE_D);
    }

    if let Some(plic_base) = dtb::plic_base() {
        printk!("VM: Map PLIC\n");
        // 映射整个 PLIC 区域 (简化处理，映射 4MB)
        unsafe {
            boot_map(&mut kpt, plic_base, plic_base, 0x3000, PTE_R | PTE_W | PTE_A | PTE_D);
        }
    }

    printk!("VM: Root page table built by hart {}\n", hartid);
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
    printk!("VM: Hart {} switched to kernel page table\n", hartid);
}

pub fn switch_off(hartid: usize) {
    unsafe {
        satp::set(satp::Mode::Bare, 0, 0);
        sfence_vma_all();
    }
    printk!("VM: Hart {} switching off VM\n", hartid);
}
