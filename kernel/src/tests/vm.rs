use super::barrier::MultiCoreTestBarrier;
use crate::dtb;
use crate::mem::addr::PhysAddr;
use crate::mem::pmem::{pmem_alloc, pmem_free};
use crate::mem::pte::{PTE_R, PTE_W, PTE_X, pte_get_flags, pte_is_valid, pte_to_pa};
use crate::mem::vm::{PageTable, vm_getpte, vm_mappages, vm_print, vm_unmappages};
use crate::mem::{PGSIZE, VA_MAX};
use crate::printk;
use crate::printk::{ANSI_GREEN, ANSI_RESET, ANSI_YELLOW};

static VM_BARRIER: MultiCoreTestBarrier = MultiCoreTestBarrier::new();

pub fn run(hartid: usize) {
    VM_BARRIER.ensure_inited(dtb::hart_count());
    if hartid == 0 {
        VM_BARRIER.init(dtb::hart_count());
        printk!("{}[TEST]{} VM test start ({} harts)", ANSI_YELLOW, ANSI_RESET, VM_BARRIER.total());
    }
    VM_BARRIER.wait_start();
    if hartid == 0 {
        vm_func_test();
        vm_mapping_test();
    }
    if VM_BARRIER.finish_and_last() {
        printk!("{}[PASS]{} VM test ({} harts)", ANSI_GREEN, ANSI_RESET, VM_BARRIER.total());
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

    printk!("--- vm_func_test: test 1 ---");
    let table = unsafe { &*test_pgtbl };
    printk!("Mapped VA 0x0 -> PA {:#x} (R)", mem[0]);
    vm_mappages(table, 0, PGSIZE, mem[0], PTE_R);
    printk!("Mapped VA 0x{:#x} -> PA {:#x} (R W)", PGSIZE * 10, mem[1]);
    vm_mappages(table, PGSIZE * 10, PGSIZE, mem[1], PTE_R | PTE_W);
    printk!("Mapped VA 0x{:#x} -> PA {:#x} (R X)", PGSIZE * 512, mem[2]);
    vm_mappages(table, PGSIZE * 512, PGSIZE, mem[2], PTE_R | PTE_X);
    printk!("Mapped VA 0x{:#x} -> PA {:#x} (R X)", PGSIZE * 512 * 512, mem[3]);
    vm_mappages(table, PGSIZE * 512 * 512, PGSIZE, mem[3], PTE_R | PTE_X);
    printk!("Mapped VA 0x{:#x} -> PA {:#x} (W)", VA_MAX - PGSIZE, mem[4]);
    vm_mappages(table, VA_MAX - PGSIZE, PGSIZE, mem[4], PTE_W);
    printk!("Page table after mapping:");
    vm_print(table);

    printk!("--- vm_func_test: test 2 ---");
    printk!("Mapped VA 0x0 -> PA {:#x} (W)", mem[0]);
    vm_mappages(table, 0, PGSIZE, mem[0], PTE_W);
    printk!("Unmapped VA 0x{:#x}", PGSIZE * 10);
    vm_unmappages(table, PGSIZE * 10, PGSIZE, true);
    printk!("Unmapped VA 0x{:#x}", PGSIZE * 512 * 512);
    vm_unmappages(table, PGSIZE * 512, PGSIZE, true);
    vm_print(table);

    // Clean up allocated memory
    for &page in mem.iter() {
        pmem_free(page, false);
    }
    pmem_free(test_pgtbl as usize, true);
    printk!("{}vm_func_test passed!{}", ANSI_GREEN, ANSI_RESET);
}

fn vm_mapping_test() {
    printk!("--- vm_mapping_test ---");

    // 1. 初始化测试页表
    // pmem_alloc 已经将内存清零
    let pgtbl = pmem_alloc(true) as *mut PageTable;
    assert!(!pgtbl.is_null(), "vm_mapping_test: pgtbl alloc failed");
    let table = unsafe { &*pgtbl };

    // 2. 准备测试条件
    let va_1: usize = 0x100000;
    let va_2: usize = 0x8000;
    let pa_1 = pmem_alloc(false) as usize;
    let pa_2 = pmem_alloc(false) as usize;
    assert!(pa_1 != 0, "vm_mapping_test: pa_1 alloc failed");
    assert!(pa_2 != 0, "vm_mapping_test: pa_2 alloc failed");

    // 3. 建立映射
    printk!("Mapping VA {:#x} -> PA {:#x} (R W)", va_1, pa_1);
    vm_mappages(table, va_1, PGSIZE, pa_1, PTE_R | PTE_W);
    printk!("Mapping VA {:#x} -> PA {:#x} (R W X)", va_2, pa_2);
    vm_mappages(table, va_2, PGSIZE, pa_2, PTE_R | PTE_W | PTE_X);

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
    printk!("Unmapping VA {:#x}", va_1);
    vm_unmappages(table, va_1, PGSIZE, true);
    printk!("Unmapping VA {:#x}", va_2);
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

    printk!("{}vm_mapping_test passed!{}", ANSI_GREEN, ANSI_RESET);
}
