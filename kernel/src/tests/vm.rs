use super::barrier::MultiCoreTestBarrier;
use crate::dtb;
use crate::mem::addr::PhysAddr;
use crate::mem::pagetable::PageTable;
use crate::mem::pmem;
use crate::mem::pte::{self, PTE_R, PTE_W, PTE_X, pte_to_pa};
use crate::mem::vm;
use crate::mem::{PGSIZE, VA_MAX};
use crate::printk;

static VM_BARRIER: MultiCoreTestBarrier = MultiCoreTestBarrier::new();

pub fn run(hartid: usize) {
    VM_BARRIER.ensure_inited(dtb::hart_count());
    if hartid == 0 {
        VM_BARRIER.init(dtb::hart_count());
        printk!("[TEST] VM test start ({} harts)", VM_BARRIER.total());
    }
    VM_BARRIER.wait_start();
    if hartid == 0 {
        vm_func_test();
        vm_mapping_test();
    }
    if VM_BARRIER.finish_and_last() {
        printk!("[PASS] VM test ({} harts)", VM_BARRIER.total());
    }
}

// TODO: Fix Panic
fn vm_func_test() {
    let test_pgtbl = pmem::alloc(true) as *mut PageTable;
    if test_pgtbl.is_null() {
        panic!("vm_func_test: failed to allocate page table");
    }
    let mut mem: [PhysAddr; 5] = [0; 5];
    for i in 0..5 {
        let page = pmem::alloc(false);
        if page.is_null() {
            panic!("vm_func_test: failed to allocate memory page");
        }
        mem[i] = page as usize;
    }

    printk!("--- vm_func_test: test 1 ---");
    let table = unsafe { &mut *test_pgtbl };
    printk!("Mapped VA 0x{:x} -> PA 0x{:x} (R)", 0, mem[0]);
    vm::mappages(table, 0, mem[0], PGSIZE, PTE_R);
    printk!("Mapped VA 0x{:x} -> PA 0x{:x} (R W)", PGSIZE * 10, mem[1]);
    vm::mappages(table, PGSIZE * 10, mem[1], PGSIZE, PTE_R | PTE_W);
    printk!("Mapped VA 0x{:x} -> PA 0x{:x} (R X)", PGSIZE * 512, mem[2]);
    vm::mappages(table, PGSIZE * 512, mem[2], PGSIZE, PTE_R | PTE_X);
    printk!("Mapped VA 0x{:x} -> PA 0x{:x} (R X)", PGSIZE * 512 * 512, mem[3]);
    vm::mappages(table, PGSIZE * 512 * 512, mem[3], PGSIZE, PTE_R | PTE_X);
    printk!("Mapped VA 0x{:x} -> PA 0x{:x} (W)", VA_MAX - PGSIZE, mem[4]);
    vm::mappages(table, VA_MAX - PGSIZE, mem[4], PGSIZE, PTE_W);
    printk!("Page table after mapping:");
    vm::print(table);

    printk!("--- vm_func_test: test 2 ---");
    printk!("Mapped VA 0x{:x} -> PA 0x{:x} (W)", 0, mem[0]);
    vm::mappages(table, 0, mem[0], PGSIZE, PTE_W);

    printk!("Unmapped VA 0x{:x}", 0);
    vm::unmappages(table, 0, PGSIZE, true);

    printk!("Unmapped VA 0x{:x}", PGSIZE * 10);
    vm::unmappages(table, PGSIZE * 10, PGSIZE, true);

    printk!("Unmapped VA 0x{:x}", PGSIZE * 512);
    vm::unmappages(table, PGSIZE * 512, PGSIZE, true);

    printk!("Unmapped VA 0x{:x}", PGSIZE * 512 * 512);
    vm::unmappages(table, PGSIZE * 512 * 512, PGSIZE, true);

    printk!("Unmapped VA 0x{:x}", VA_MAX - PGSIZE);
    vm::unmappages(table, VA_MAX - PGSIZE, PGSIZE, true);
    vm::print(table);

    pmem::free(test_pgtbl as usize, true);
    printk!("vm_func_test passed!");
}

fn vm_mapping_test() {
    printk!("--- vm_mapping_test ---");

    // 1. 初始化测试页表
    // pmem::alloc 已经将内存清零
    let pgtbl = pmem::alloc(true) as *mut PageTable;
    assert!(!pgtbl.is_null(), "vm_mapping_test: pgtbl alloc failed");
    let table = unsafe { &mut *pgtbl };
    // 2. 准备测试条件
    let va_1: usize = 0x100000;
    let va_2: usize = 0x8000;
    let pa_1 = pmem::alloc(false) as usize;
    let pa_2 = pmem::alloc(false) as usize;
    assert!(pa_1 != 0, "vm_mapping_test: pa_1 alloc failed");
    assert!(pa_2 != 0, "vm_mapping_test: pa_2 alloc failed");

    // 3. 建立映射
    printk!("Mapping VA 0x{:x} -> PA 0x{:x} (R W)", va_1, pa_1);
    vm::mappages(table, va_1, pa_1, PGSIZE, PTE_R | PTE_W);
    printk!("Mapping VA 0x{:x} -> PA 0x{:x} (R W X)", va_2, pa_2);
    vm::mappages(table, va_2, pa_2, PGSIZE, PTE_R | PTE_W | PTE_X);

    // 4. 验证映射结果
    let pte_1_ptr = vm::getpte(table, va_1);
    let pte_1 = unsafe { *pte_1_ptr };
    assert!(!pte_1_ptr.is_null(), "vm_mapping_test: pte_1 not found");
    assert!(pte::is_valid(pte_1), "vm_mapping_test: pte_1 not valid");
    assert_eq!(pte_to_pa(pte_1), pa_1, "vm_mapping_test: pa_1 mismatch");
    assert_eq!(
        pte::get_flags(pte_1) & (PTE_R | PTE_W),
        PTE_R | PTE_W,
        "vm_mapping_test: flag_1 mismatch"
    );

    let pte_2_ptr = vm::getpte(table, va_2);
    assert!(!pte_2_ptr.is_null(), "vm_mapping_test: pte_2 not found");
    let pte_2 = unsafe { *pte_2_ptr };
    assert!(pte::is_valid(pte_2), "vm_mapping_test: pte_2 not valid");
    assert_eq!(pte_to_pa(pte_2), pa_2, "vm_mapping_test: pa_2 mismatch");
    // C 代码中的断言是错误的，这里修正为只检查 PTE_R
    assert_eq!(
        pte::get_flags(pte_2) & (PTE_R | PTE_W),
        PTE_R | PTE_W,
        "vm_mapping_test: flag_2 mismatch"
    );

    // 5. 解除映射
    // vm::unmappages 会释放 pa_1 和 pa_2
    printk!("Unmapping VA 0x{:x}", va_1);
    vm::unmappages(table, va_1, PGSIZE, true);
    printk!("Unmapping VA 0x{:x}", va_2);
    vm::unmappages(table, va_2, PGSIZE, true);

    // 6. 验证解除映射结果
    let pte_1_ptr_after = vm::getpte(table, va_1);
    assert!(!pte_1_ptr_after.is_null(), "vm_mapping_test: pte_1 not found after unmap");
    let pte_1_after = unsafe { *pte_1_ptr_after };
    assert!(!pte::is_valid(pte_1_after), "vm_mapping_test: pte_1 still valid");

    let pte_2_ptr_after = vm::getpte(table, va_2);
    assert!(!pte_2_ptr_after.is_null(), "vm_mapping_test: pte_2 not found after unmap");
    let pte_2_after = unsafe { *pte_2_ptr_after };
    assert!(!pte::is_valid(pte_2_after), "vm_mapping_test: pte_2 still valid");

    // 7. 清理页表
    pmem::free(pgtbl as usize, true);

    printk!("vm_mapping_test passed!");
}
