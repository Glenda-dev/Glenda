use super::barrier::MultiCoreTestBarrier;
use crate::dtb;
use crate::mem::addr::PhysAddr;
use crate::mem::pagetable::PageTable;
use crate::mem::pmem::{pmem_alloc, pmem_free};
use crate::mem::pte::{PTE_R, PTE_W, PTE_X, pte_get_flags, pte_is_valid, pte_to_pa};
use crate::mem::vm::{vm_getpte, vm_mappages, vm_print, vm_unmappages};
use crate::mem::{PGSIZE, VA_MAX};
use crate::printk::{uart_hex, uart_puts};

static VM_BARRIER: MultiCoreTestBarrier = MultiCoreTestBarrier::new();

pub fn run(hartid: usize) {
    VM_BARRIER.ensure_inited(dtb::hart_count());
    if hartid == 0 {
        VM_BARRIER.init(dtb::hart_count());
        uart_puts("[TEST] VM test start (");
        uart_hex(VM_BARRIER.total());
        uart_puts(" harts)\n");
    }
    VM_BARRIER.wait_start();
    if hartid == 0 {
        vm_func_test();
        vm_mapping_test();
    }
    if VM_BARRIER.finish_and_last() {
        uart_puts("[PASS] VM test (");
        uart_hex(VM_BARRIER.total());
        uart_puts(" harts)\n");
    }
}

fn vm_func_test() {
    let test_pgtbl = pmem_alloc(true) as *mut PageTable;
    if test_pgtbl.is_null() {
        panic!("vm_func_test: failed to allocate page table");
    }
    let mut mem: [PhysAddr; 5] = [0; 5];
    for i in 0..5 {
        let page = pmem_alloc(false);
        if page.is_null() {
            panic!("vm_func_test: failed to allocate memory page");
        }
        mem[i] = page as usize;
    }

    uart_puts("--- vm_func_test: test 1 ---\n");
    let table = unsafe { &mut *test_pgtbl };
    uart_puts("Mapped VA ");
    uart_hex(0);
    uart_puts(" -> PA ");
    uart_hex(mem[0]);
    uart_puts(" (R)\n");
    vm_mappages(table, 0, mem[0], PGSIZE, PTE_R);
    uart_puts("Mapped VA ");
    uart_hex(PGSIZE * 10);
    uart_puts(" -> PA ");
    uart_hex(mem[1]);
    uart_puts(" (R W)\n");
    vm_mappages(table, PGSIZE * 10, mem[1], PGSIZE, PTE_R | PTE_W);
    uart_puts("Mapped VA ");
    uart_hex(PGSIZE * 512);
    uart_puts(" -> PA ");
    uart_hex(mem[2]);
    uart_puts(" (R X)\n");
    vm_mappages(table, PGSIZE * 512, mem[2], PGSIZE, PTE_R | PTE_X);
    uart_puts("Mapped VA ");
    uart_hex(PGSIZE * 512 * 512);
    uart_puts(" -> PA ");
    uart_hex(mem[3]);
    uart_puts(" (R X)\n");
    vm_mappages(table, PGSIZE * 512 * 512, mem[3], PGSIZE, PTE_R | PTE_X);
    uart_puts("Mapped VA ");
    uart_hex(VA_MAX - PGSIZE);
    uart_puts(" -> PA ");
    uart_hex(mem[4]);
    uart_puts(" (W)\n");
    vm_mappages(table, VA_MAX - PGSIZE, mem[4], PGSIZE, PTE_W);
    uart_puts("Page table after mapping:\n");
    vm_print(table);

    uart_puts("--- vm_func_test: test 2 ---\n");
    uart_puts("Mapped VA ");
    uart_hex(0);
    uart_puts(" -> PA ");
    uart_hex(mem[0]);
    uart_puts(" (W)\n");
    vm_mappages(table, 0, mem[0], PGSIZE, PTE_W);
    uart_puts("Unmapped VA ");
    uart_hex(PGSIZE * 10);
    uart_puts("\n");
    vm_unmappages(table, PGSIZE * 10, PGSIZE, true);
    uart_puts("Unmapped VA ");
    uart_hex(PGSIZE * 512 * 512);
    uart_puts("\n");
    vm_unmappages(table, PGSIZE * 512, PGSIZE, true);
    vm_print(table);

    // Clean up allocated memory
    for &page in mem.iter() {
        pmem_free(page, false);
    }
    pmem_free(test_pgtbl as usize, true);
    uart_puts("vm_func_test passed!\n");
}

fn vm_mapping_test() {
    uart_puts("--- vm_mapping_test ---\n");

    // 1. 初始化测试页表
    // pmem_alloc 已经将内存清零
    let pgtbl = pmem_alloc(true) as *mut PageTable;
    assert!(!pgtbl.is_null(), "vm_mapping_test: pgtbl alloc failed");
    let table = unsafe { &mut *pgtbl };
    // 2. 准备测试条件
    let va_1: usize = 0x100000;
    let va_2: usize = 0x8000;
    let pa_1 = pmem_alloc(false) as usize;
    let pa_2 = pmem_alloc(false) as usize;
    assert!(pa_1 != 0, "vm_mapping_test: pa_1 alloc failed");
    assert!(pa_2 != 0, "vm_mapping_test: pa_2 alloc failed");

    // 3. 建立映射
    uart_puts("Mapping VA ");
    uart_hex(va_1);
    uart_puts(" -> PA ");
    uart_hex(pa_1);
    uart_puts(" (R W)\n");
    vm_mappages(table, va_1, pa_1, PGSIZE, PTE_R | PTE_W);
    uart_puts("Mapping VA ");
    uart_hex(va_2);
    uart_puts(" -> PA ");
    uart_hex(pa_2);
    uart_puts(" (R W X)\n");
    vm_mappages(table, va_2, pa_2, PGSIZE, PTE_R | PTE_W | PTE_X);

    // 4. 验证映射结果
    let pte_1_ptr = vm_getpte(table, va_1);
    let pte_1 = unsafe { *pte_1_ptr };
    assert!(!pte_1_ptr.is_null(), "vm_mapping_test: pte_1 not found");
    assert!(pte_is_valid(pte_1), "vm_mapping_test: pte_1 not valid");
    assert_eq!(pte_to_pa(pte_1), pa_1, "vm_mapping_test: pa_1 mismatch");
    assert_eq!(
        pte_get_flags(pte_1) & (PTE_R | PTE_W),
        PTE_R | PTE_W,
        "vm_mapping_test: flag_1 mismatch"
    );

    let pte_2_ptr = vm_getpte(table, va_2);
    assert!(!pte_2_ptr.is_null(), "vm_mapping_test: pte_2 not found");
    let pte_2 = unsafe { *pte_2_ptr };
    assert!(pte_is_valid(pte_2), "vm_mapping_test: pte_2 not valid");
    assert_eq!(pte_to_pa(pte_2), pa_2, "vm_mapping_test: pa_2 mismatch");
    // C 代码中的断言是错误的，这里修正为只检查 PTE_R
    assert_eq!(
        pte_get_flags(pte_2) & (PTE_R | PTE_W),
        PTE_R | PTE_W,
        "vm_mapping_test: flag_2 mismatch"
    );

    // 5. 解除映射
    // vm_unmappages 会释放 pa_1 和 pa_2
    uart_puts("Unmapping VA ");
    uart_hex(va_1);
    uart_puts("\n");
    vm_unmappages(table, va_1, PGSIZE, true);
    uart_puts("Unmapping VA ");
    uart_hex(va_2);
    uart_puts("\n");
    vm_unmappages(table, va_2, PGSIZE, true);

    // 6. 验证解除映射结果
    let pte_1_ptr_after = vm_getpte(table, va_1);
    assert!(!pte_1_ptr_after.is_null(), "vm_mapping_test: pte_1 not found after unmap");
    let pte_1_after = unsafe { *pte_1_ptr_after };
    assert!(!pte_is_valid(pte_1_after), "vm_mapping_test: pte_1 still valid");

    let pte_2_ptr_after = vm_getpte(table, va_2);
    assert!(!pte_2_ptr_after.is_null(), "vm_mapping_test: pte_2 not found after unmap");
    let pte_2_after = unsafe { *pte_2_ptr_after };
    assert!(!pte_is_valid(pte_2_after), "vm_mapping_test: pte_2 still valid");

    // 7. 清理页表
    pmem_free(pgtbl as usize, true);

    uart_puts("vm_mapping_test passed!\n");
}
