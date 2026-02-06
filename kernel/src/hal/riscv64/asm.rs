use core::arch::asm;

#[inline(always)]
pub fn read_scause() -> usize {
    let scause: usize;
    unsafe {
        asm!("csrr {}, scause", out(reg) scause);
    }
    scause
}

#[inline(always)]
pub fn read_sepc() -> usize {
    let sepc: usize;
    unsafe {
        asm!("csrr {}, sepc", out(reg) sepc);
    }
    sepc
}

#[inline(always)]
pub unsafe fn write_sepc(value: usize) {
    unsafe {
        asm!("csrw sepc, {}", in(reg) value);
    }
}

#[inline(always)]
pub fn read_stval() -> usize {
    let stval: usize;
    unsafe {
        asm!("csrr {}, stval", out(reg) stval);
    }
    stval
}

#[inline(always)]
pub fn read_sstatus() -> usize {
    let sstatus: usize;
    unsafe {
        asm!("csrr {}, sstatus", out(reg) sstatus);
    }
    sstatus
}

#[inline(always)]
pub unsafe fn write_stvec(value: usize) {
    unsafe {
        asm!("csrw stvec, {}", in(reg) value);
    }
}

#[inline(always)]
pub unsafe fn write_sstatus(value: usize) {
    unsafe {
        asm!("csrw sstatus, {}", in(reg) value);
    }
}

#[inline(always)]
pub unsafe fn sstatus_clear(info: usize) {
    unsafe {
        // 使用 csrc (Register operand) 代替 csrci (Immediate operand)
        asm!("csrc sstatus, {}", in(reg) 1<<info);
    }
}

#[inline(always)]
pub unsafe fn sstatus_set(info: usize) {
    unsafe {
        asm!("csrs sstatus, {}", in(reg) 1<<info);
    }
}

#[inline(always)]
pub fn sstatus_query(info: usize) -> bool {
    let sstatus = read_sstatus();
    (sstatus & (1 << info)) != 0
}

#[inline(always)]
pub fn read_satp() -> usize {
    let satp: usize;
    unsafe {
        asm!("csrr {}, satp", out(reg) satp);
    }
    satp
}

#[inline(always)]
pub fn read_tp() -> usize {
    let mut tp: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) id);
    }
    tp
}

#[inline(always)]
pub unsafe fn write_sscratch(value: usize) {
    unsafe {
        asm!("csrw sscratch, {}", in(reg) value);
    }
}

#[inline(always)]
pub unsafe fn read_a0() -> usize {
    let a0: usize;
    unsafe {
        asm!("mv {}, a0", out(reg) a0);
    }
    a0
}

#[inline(always)]
pub unsafe fn read_a1() -> usize {
    let a1: usize;
    unsafe {
        asm!("mv {}, a1", out(reg) a1);
    }
    a1
}

#[inline(always)]
pub unsafe fn write_satp(value: usize) {
    unsafe {
        asm!("csrw satp, {}", in(reg) value);
    }
}

#[inline(always)]
pub unsafe fn sfence_vma(vaddr: usize) {
    unsafe {
        asm!("sfence.vma {}, zero", in(reg) vaddr);
    }
}

#[inline(always)]
pub unsafe fn sfence_vma_all() {
    unsafe {
        asm!("sfence.vma zero, zero");
    }
}

#[inline(always)]
pub unsafe fn wfi() {
    unsafe {
        asm!("wfi");
    }
}

#[inline(always)]
pub unsafe fn sie_set(intr: usize) {
    unsafe {
        asm!("csrs sie, {}", in(reg) 1<<intr);
    }
}

#[inline(always)]
pub unsafe fn sie_clear(intr: usize) {
    unsafe {
        asm!("csrc sie, {}", in(reg) 1<<intr);
    }
}

#[inline(always)]
pub unsafe fn sip_set(intr: usize) {
    unsafe {
        asm!("csrs sip, {}", in(reg) 1<<intr);
    }
}

#[inline(always)]
pub unsafe fn sip_clear(intr: usize) {
    unsafe {
        asm!("csrc sip, {}", in(reg) 1<<intr);
    }
}

#[inline(always)]
pub unsafe fn rdcycle() -> usize {
    let cycle: usize;
    unsafe {
        asm!("rdcycle {}", out(reg) cycle);
    }
    cycle
}

#[inline(always)]
pub fn rdtime() -> usize {
    let time: usize;
    unsafe {
        asm!("rdtime {}", out(reg) time);
    }
    time
}
