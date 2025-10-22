use crate::mem::pmem::pmem_alloc;
use crate::mem::pte::{PTE_A, PTE_D, PTE_R, PTE_U, PTE_W, PTE_X};
use crate::mem::vm::vm_map_kernel_pages;
use crate::printk;

const PAGE_SIZE: usize = crate::mem::PGSIZE;

unsafe extern "C" {
    fn enter_user(entry: usize, user_sp: usize) -> !;
}

// 由 build.rs 生成，这个文件可能为空
include!(concat!(env!("OUT_DIR"), "/user_payload.rs"));

// 故提供一个 Fallback example
static USER_INIT_CODE: [u8; 20] = [
    0x93, 0x08, 0x10, 0x00,
    0x73, 0x00, 0x00, 0x00,
    0x93, 0x08, 0x10, 0x00,
    0x73, 0x00, 0x00, 0x00,
    0x6f, 0x00, 0x00, 0x00,
];

pub fn launch_first_user() -> ! {
    let code_pa = pmem_alloc(false) as usize;
    let stack_pa = pmem_alloc(false) as usize;

    let (src_ptr, src_len) = if HAS_USER_PAYLOAD && !USER_PAYLOAD.is_empty() {
        (USER_PAYLOAD.as_ptr(), USER_PAYLOAD.len())
    } else {
        (USER_INIT_CODE.as_ptr(), USER_INIT_CODE.len())
    };
    let copy_len = core::cmp::min(src_len, PAGE_SIZE);
    unsafe { core::ptr::copy_nonoverlapping(src_ptr, code_pa as *mut u8, copy_len) };

    // Code: U|R|X
    vm_map_kernel_pages(code_pa, PAGE_SIZE, code_pa, PTE_U | PTE_R | PTE_X | PTE_A);
    // Stack: U|R|W
    vm_map_kernel_pages(stack_pa, PAGE_SIZE, stack_pa, PTE_U | PTE_R | PTE_W | PTE_A | PTE_D);

    let entry = code_pa;
    let user_sp = stack_pa + PAGE_SIZE;
    printk!("USER: launching first user at {:p}, sp={:p}", entry as *const u8, user_sp as *const u8);
    unsafe { enter_user(entry, user_sp) }
}
