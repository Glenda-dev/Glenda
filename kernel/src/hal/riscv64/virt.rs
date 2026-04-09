use crate::boot;

use super::asm;
use core::str;

const SSTATUS_VS_BIT: usize = 9;
const HSTATUS_SPV_BIT: usize = 7;
const HSTATUS_SPVP_BIT: usize = 8;
const HEDELEG_DEFAULT: usize = 0;
const HIDELEG_DEFAULT: usize = 0;

#[inline(always)]
fn read_hstatus() -> usize {
    let value: usize;
    unsafe {
        core::arch::asm!("csrr {}, hstatus", out(reg) value);
    }
    value
}

#[inline(always)]
fn write_hstatus(value: usize) {
    unsafe {
        core::arch::asm!("csrw hstatus, {}", in(reg) value);
    }
}

#[inline(always)]
fn write_hedeleg(value: usize) {
    unsafe {
        core::arch::asm!("csrw hedeleg, {}", in(reg) value);
    }
}

#[inline(always)]
fn write_hideleg(value: usize) {
    unsafe {
        core::arch::asm!("csrw hideleg, {}", in(reg) value);
    }
}

pub fn is_enabled() -> bool {
    let mut enabled = false;
    if let Some((dtb, _)) = boot::get_dtb() {
        if let Ok(fdt) = unsafe { fdt::Fdt::from_ptr(dtb.as_usize() as *const u8) } {
            for node in fdt.all_nodes() {
                let Some(device_type_prop) = node.property("device_type") else {
                    continue;
                };
                let Ok(device_type) = str::from_utf8(device_type_prop.value) else {
                    continue;
                };
                if device_type.trim_end_matches('\0') != "cpu" {
                    continue;
                }

                let Some(isa_prop) = node.property("riscv,isa") else {
                    continue;
                };
                let Ok(isa) = str::from_utf8(isa_prop.value) else {
                    continue;
                };
                if isa.trim_end_matches('\0').contains('h') {
                    enabled = true;
                    break;
                }
            }
        }
    }
    enabled
}

pub fn init_cpu() {
    if !is_enabled() {
        return;
    }

    let mut sstatus = asm::read_sstatus();
    sstatus &= !(1 << SSTATUS_VS_BIT);
    unsafe {
        asm::write_sstatus(sstatus);
    }

    let mut hstatus = read_hstatus();
    hstatus &= !(1 << HSTATUS_SPV_BIT);
    hstatus &= !(1 << HSTATUS_SPVP_BIT);
    write_hstatus(hstatus);
    write_hedeleg(HEDELEG_DEFAULT);
    write_hideleg(HIDELEG_DEFAULT);

    log!("virt: rvh detected and initialized on cpu {}", super::cpu::cpu_id());
}
