use core::panic;

use super::PGSIZE;
use super::addr::{align_down, align_up};
use super::pmem::{self, kernel_region_info, user_region_info};
use super::pte::{PTE_A, PTE_D, PTE_R, PTE_W, PTE_X, Pte};
use super::{PageTable, PhysAddr, VirtAddr};
use crate::drivers;
use crate::dtb;
use crate::irq::vector;
use crate::printk;
use crate::printk::{ANSI_RESET, ANSI_YELLOW};
use riscv::asm::sfence_vma_all;
use riscv::register::satp;
use spin::{Mutex, Once};

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

static KERNEL_PAGE_TABLE: Mutex<PageTable> = Mutex::new(PageTable::new());

// 256MB region for kernel stacks, below VA_MAX
pub const KSTACK_REGION_SIZE: usize = 0x1000_0000; 
pub const KSTACK_VA_BASE: usize = super::VA_MAX - KSTACK_REGION_SIZE;

// Increase kernel stack to 4 pages (16KB)
pub const KSTACK_SIZE: usize = super::PGSIZE * 4;

pub fn map_kstack0() {
    let _top = alloc_kstack(0);
    printk!("VM: KSTACK(0) allocated at VA={:p}\n", kstack_base(0) as *const u8);
}

#[inline(always)]
pub fn kstack_base(procid: usize) -> VirtAddr {
    KSTACK_VA_BASE + procid * KSTACK_SIZE
}

#[inline(always)]
pub fn kstack_top(procid: usize) -> VirtAddr {
    kstack_base(procid) + KSTACK_SIZE
}

pub fn alloc_kstack(pid: usize) -> VirtAddr {
    let base = kstack_base(pid);
    let mut kpt = KERNEL_PAGE_TABLE.lock();
    for i in 0..(KSTACK_SIZE / super::PGSIZE) {
        let va = base + i * super::PGSIZE;
        let pa = pmem::alloc(true) as PhysAddr;
        mappages(&mut kpt, va, pa, super::PGSIZE, PTE_R | PTE_W | PTE_A | PTE_D);
    }
    sfence_vma_all();
    base + KSTACK_SIZE
}

pub fn free_kstack(pid: usize) {
    let base = kstack_base(pid);
    let mut kpt = KERNEL_PAGE_TABLE.lock();
    for i in 0..(KSTACK_SIZE / super::PGSIZE) {
        let va = base + i * super::PGSIZE;
        unmappages(&mut kpt, va, super::PGSIZE, true);
    }
    sfence_vma_all();
}

pub fn getpte(table: &PageTable, va: VirtAddr) -> *mut Pte {
    match table.lookup(va) {
        Some(p) => p,
        None => panic!("vm_getpte: failed for VA {:#x}\n", va),
    }
}

pub fn mappages(table: &mut PageTable, va: VirtAddr, pa: PhysAddr, size: usize, perm: usize) {
    if !table.map(va, pa, size, perm) {
        panic!("vm_mappages: failed map VA {:#x} -> PA {:#x}\n", va, pa);
    }
}

pub fn unmappages(table: &mut PageTable, va: VirtAddr, size: usize, free: bool) {
    if !table.unmap(va, size, free) {
        // table.print();
        panic!("vm_unmappages: failed unmap VA {:#x}", va);
    }
}

pub fn map_kernel_pages(va: VirtAddr, pa: PhysAddr, size: usize, perm: usize) {
    let mut kpt = KERNEL_PAGE_TABLE.lock();
    mappages(&mut kpt, va, pa, size, perm);
    sfence_vma_all();
}

#[cfg(debug_assertions)]
pub fn print(table: &PageTable) {
    table.print();
}

pub fn init_kernel_vm(hartid: usize) {
    static BUILD_ONCE: Once<()> = Once::new();
    BUILD_ONCE.call_once(|| {
        let kpt = &mut KERNEL_PAGE_TABLE.lock();
        // 权限映射, PTE_A/D 理论上硬件会帮忙做，但不确定 QEMU Virt 的具体行为，所以还是加上
        let text_start_addr = unsafe { &__text_start as *const u8 as usize };
        let text_end_addr = unsafe { &__text_end as *const u8 as usize };
        printk!(
            "VM: Map .text [{:p}, {:p})\n",
            text_start_addr as *const u8,
            text_end_addr as *const u8
        );
        mappages(
            kpt,
            text_start_addr,
            text_start_addr,
            text_end_addr - text_start_addr,
            PTE_R | PTE_X | PTE_A,
        );

        let rodata_start_addr = unsafe { &__rodata_start as *const u8 as usize };
        let rodata_end_addr = unsafe { &__rodata_end as *const u8 as usize };
        printk!(
            "VM: Map .rodata [{:p}, {:p})\n",
            rodata_start_addr as *const u8,
            rodata_end_addr as *const u8
        );
        mappages(
            kpt,
            rodata_start_addr,
            rodata_start_addr,
            rodata_end_addr - rodata_start_addr,
            PTE_R | PTE_A,
        );

        let data_start_addr = unsafe { &__data_start as *const u8 as usize };
        let data_end_addr = unsafe { &__data_end as *const u8 as usize };
        printk!(
            "VM: Map .data [{:p}, {:p})\n",
            data_start_addr as *const u8,
            data_end_addr as *const u8
        );
        mappages(
            kpt,
            data_start_addr,
            data_start_addr,
            data_end_addr - data_start_addr,
            PTE_R | PTE_W | PTE_A | PTE_D,
        );

        let bss_start_addr = unsafe { &__bss_start as *const u8 as usize };
        let bss_end_addr = unsafe { &__bss_end as *const u8 as usize };
        printk!(
            "VM: Map .bss [{:p}, {:p})\n",
            bss_start_addr as *const u8,
            bss_end_addr as *const u8
        );
        mappages(
            kpt,
            bss_start_addr,
            bss_start_addr,
            bss_end_addr - bss_start_addr,
            PTE_R | PTE_W | PTE_A | PTE_D,
        );

        // TRAMPOLINE 映射
        let tramp_pa = align_down(vector::trampoline as usize);
        let tramp_va = super::VA_MAX - super::PGSIZE;
        printk!(
            "VM: Map TRAMPOLINE VA={:p} -> PA={:p}\n",
            tramp_va as *const u8,
            tramp_pa as *const u8
        );
        mappages(kpt, tramp_va, tramp_pa, PGSIZE, PTE_R | PTE_X | PTE_A);

        // MMIO 映射
        let uart_base = dtb::uart_config().unwrap_or(drivers::uart::DEFAULT_QEMU_VIRT).base;
        let uart_size = PGSIZE;
        printk!("VM: Map UART @ {:p}\n", uart_base as *const u8);
        mappages(kpt, uart_base, uart_base, uart_size, PTE_R | PTE_W | PTE_A | PTE_D);

        // VirtIO Block Device
        let virtio_base = 0x10001000;
        let virtio_size = PGSIZE;
        printk!("VM: Map VirtIO @ {:p}", virtio_base as *const u8);
        mappages(kpt, virtio_base, virtio_base, virtio_size, PTE_R | PTE_W | PTE_A | PTE_D);

        // PLIC 映射
        let plic_base = match dtb::plic_base() {
            Some(b) => b,
            None => {
                printk!(
                    "{}[WARN] PLIC not found in DTB; skipping PLIC mapping{}\n",
                    ANSI_YELLOW,
                    ANSI_RESET
                );
                printk!(
                    "{}[WARN] External interrupts may fault under VM{}\n",
                    ANSI_YELLOW,
                    ANSI_RESET
                );
                return;
            }
        };
        let plic_low_start = plic_base;
        let plic_low_end = plic_base + 0x3000;
        printk!(
            "VM: Map PLIC low [{:p}, {:p})\n",
            plic_low_start as *const u8,
            plic_low_end as *const u8
        );
        mappages(
            kpt,
            align_down(plic_low_start),
            align_down(plic_low_start),
            align_up(plic_low_end) - align_down(plic_low_start),
            PTE_R | PTE_W | PTE_A | PTE_D,
        );

        let plic_ctx_start = plic_base + 0x200000;
        let harts = crate::dtb::hart_count();
        let max_ctx_index = if harts > 0 { (harts - 1) * 2 + 1 } else { 1 };
        let plic_ctx_end = plic_ctx_start + (max_ctx_index + 1) * 0x1000;
        printk!(
            "VM: Map PLIC ctx [{:p}, {:p})\n",
            plic_ctx_start as *const u8,
            plic_ctx_end as *const u8
        );
        mappages(
            kpt,
            align_down(plic_ctx_start),
            align_down(plic_ctx_start),
            align_up(plic_ctx_end) - align_down(plic_ctx_start),
            PTE_R | PTE_W | PTE_A | PTE_D,
        );

        // 内核的物理页分配池
        let kernel_info = kernel_region_info();
        let map_start = align_down(kernel_info.begin);
        let map_end = align_up(kernel_info.end);
        if map_start < map_end {
            printk!(
                "VM: Map kernel pool [{:p}, {:p})\n",
                map_start as *const u8,
                map_end as *const u8
            );
            mappages(kpt, map_start, map_start, map_end - map_start, PTE_R | PTE_W | PTE_A | PTE_D);
        }
        // FIXME: 不应该这么做，目前仅为过测试
        let user = user_region_info();
        let user_start = align_down(user.begin);
        let user_end = align_up(user.end);
        if user_start < user_end {
            printk!(
                "VM: Map user pool [{:p}, {:p})\n",
                user_start as *const u8,
                user_end as *const u8
            );
            mappages(
                kpt,
                user_start,
                user_start,
                user_end - user_start,
                PTE_R | PTE_W | PTE_A | PTE_D,
            );
        }
        printk!("VM: Root page table built by hart {}\n", hartid);
    });
    map_kstack0();
}

pub fn switch_to_kernel(hartid: usize) {
    let root_ppn = {
        let kpt = KERNEL_PAGE_TABLE.lock();
        (&*kpt as *const PageTable as usize) >> 12
    };
    // set SATP to the new page table in Sv39 mode (ASID=0)
    unsafe {
        satp::set(satp::Mode::Sv39, 0, root_ppn);
        // flush all TLB entries
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
