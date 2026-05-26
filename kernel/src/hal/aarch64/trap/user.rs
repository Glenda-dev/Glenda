use super::super::asm;
use super::super::cpu;
use super::super::mem::{PGSIZE, TRAMPOLINE_VA};
use super::vector::{__user_vector_table_base, kernel_vector, user_return};
use crate::mem::VirtAddr;
use crate::proc::scheduler;
use crate::trap::handler::trap_kernel_handler;
use core::mem::transmute;

const ATTRIDX_MASK: usize = 0x7;
const ATTRIDX_SHIFT: usize = 2;
const PXN_BIT: usize = 1 << 53;
const UXN_BIT: usize = 1 << 54;

fn log_user_pte_state(label: &str, pt: &mut crate::mem::PageTable, va: usize) {
    match pt.walk_with_level(VirtAddr::from(va)) {
        Some((pte_ptr, level)) => {
            let pte = unsafe { *pte_ptr };
            let raw = pte.as_usize();
            let pxn = (raw & PXN_BIT) != 0;
            let uxn = (raw & UXN_BIT) != 0;
            let attr_idx = (raw >> ATTRIDX_SHIFT) & ATTRIDX_MASK;
            debug!(
                "aarch64: {} va={:#x} level={} raw_pte={:#x} pa={:#x} flags={} attr_idx={} pxn={} uxn={} exec={}",
                label,
                va,
                level,
                raw,
                pte.pa().as_usize(),
                pte.get_flags(),
                attr_idx,
                pxn,
                uxn,
                !pxn || !uxn,
            );
        }
        None => {
            debug!("aarch64: {} va={:#x} missing in current user page table", label, va);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn trap_user_handler() {
    debug!("aarch64: trap_user_handler entered");
    unsafe { asm::write_vbar(kernel_vector as *const () as usize) };
    let tcb = unsafe { &mut *scheduler::current().expect("No current process in scheduler") };
    let ctx = tcb.get_tf();
    trap_kernel_handler(ctx);
    debug!("aarch64: trap_user_handler returning to user space");
    trap_user_return();
}

pub fn trap_user_return() {
    let tcb = unsafe { &mut *scheduler::current().expect("No current process in scheduler") };
    let kstack_top = tcb.get_kstack_top().as_usize();
    let user_mmu = tcb.mmu_register();
    let user_tf_va = tcb.trapframe_va;
    assert!(user_tf_va != 0, "TCB's trapframe_va should be set before returning to user mode");
    let ctx = tcb.get_tf();
    debug!(
        "aarch64: trap_user_return epc={:#x} sp={:#x} tf_va={:#x} user_mmu={:#x}",
        ctx.get_epc(),
        ctx.get_sp(),
        user_tf_va,
        user_mmu
    );

    let tramp_base_va = TRAMPOLINE_VA;
    let user_vec_table_off =
        (unsafe { &__user_vector_table_base as *const u8 as usize }) & (PGSIZE - 1);
    assert_eq!(
        user_vec_table_off & 0x7ff,
        0,
        "AArch64 user vector table must be 2KB aligned within trampoline page"
    );
    let user_vec_table_addr = tramp_base_va + user_vec_table_off;

    unsafe {
        // Keep asynchronous exceptions on the kernel vector until eret.
        asm::write_daif(asm::read_daif() | 0x3c0usize);
        asm::write_vbar(user_vec_table_addr);
    }

    let kernel_mmu = asm::read_ttbr0();

    ctx.configure_kernel(
        kernel_mmu,
        cpu::cpu_id(),
        kstack_top,
        trap_user_handler as *const () as usize,
    );

    unsafe { asm::write_tpidr_el1(user_tf_va) };

    let user_ret_off = (user_return as *const () as usize) & (PGSIZE - 1);
    let user_ret_addr = tramp_base_va + user_ret_off;
    let user_return_fn: extern "C" fn(usize, usize) -> ! = unsafe { transmute(user_ret_addr) };
    let user_vec_par = asm::at_s1e1r(user_vec_table_addr);
    let user_ret_par_el1 = asm::at_s1e1r(user_ret_addr);
    let user_ret_par_el0 = asm::at_s1e0r(user_ret_addr);
    let pt = tcb.get_pt_mut();
    let first_words = unsafe {
        [
            (user_ret_addr as *const u32).read_volatile(),
            (user_ret_addr.wrapping_add(4) as *const u32).read_volatile(),
            (user_ret_addr.wrapping_add(8) as *const u32).read_volatile(),
            (user_ret_addr.wrapping_add(12) as *const u32).read_volatile(),
        ]
    };
    debug!(
        "aarch64: return prep tf_va={:#x} user_mmu={:#x} kernel_mmu={:#x} vbar={:#x} mair={:#x} tcr={:#x} sctlr={:#x} tpidr_el1={:#x}",
        user_tf_va,
        user_mmu,
        kernel_mmu,
        asm::read_vbar(),
        asm::read_mair(),
        asm::read_tcr(),
        asm::read_sctlr(),
        asm::read_tpidr_el1(),
    );
    log_user_pte_state("trampoline page", pt, tramp_base_va);
    log_user_pte_state("user vector table", pt, user_vec_table_addr);
    log_user_pte_state("user return", pt, user_ret_addr);
    debug!(
        "aarch64: trampoline probe vec={:#x} par_s1e1r={:#x} ret={:#x} par_s1e1r={:#x} par_s1e0r={:#x} words=[{:#010x}, {:#010x}, {:#010x}, {:#010x}]",
        user_vec_table_addr,
        user_vec_par,
        user_ret_addr,
        user_ret_par_el1,
        user_ret_par_el0,
        first_words[0],
        first_words[1],
        first_words[2],
        first_words[3],
    );
    debug!(
        "aarch64: Jumping to user return function at {:#x} with tf_va={:#x} user_mmu={:#x}",
        user_ret_addr, user_tf_va, user_mmu
    );
    user_return_fn(user_tf_va, user_mmu)
}
