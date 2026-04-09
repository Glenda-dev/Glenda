use crate::boot;
use crate::error::Error;
use crate::mem::PhysAddr;

use super::asm;
use core::str;

const SSTATUS_VS_BIT: usize = 9;
const HSTATUS_SPV_BIT: usize = 7;
const HSTATUS_SPVP_BIT: usize = 8;
const HSTATUS_HU_BIT: usize = 9;
const HEDELEG_DEFAULT: usize = 0;
const HIDELEG_DEFAULT: usize = 0;
const HGATP_MODE_SV39X4: usize = 8;
pub const VCPU_REG_COUNT: usize = 32;

#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtExitReason {
    None = 0,
    HostTrap = 1,
    GuestPageFault = 2,
    VirtualInstruction = 3,
    ExternalInterrupt = 4,
    TimerInterrupt = 5,
    Unknown = 255,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VirtExit {
    pub reason: VirtExitReason,
    pub detail0: usize,
    pub detail1: usize,
    pub detail2: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VcpuRunState {
    pub regs: [usize; VCPU_REG_COUNT],
    pub pc: usize,
    pub hgatp: usize,
    pub pending_virq_bitmap: usize,
}

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

#[inline(always)]
fn write_hgatp(value: usize) {
    unsafe {
        core::arch::asm!("csrw hgatp, {}", in(reg) value);
    }
}

#[inline(always)]
fn read_hgatp() -> usize {
    let value: usize;
    unsafe {
        core::arch::asm!("csrr {}, hgatp", out(reg) value);
    }
    value
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
    hstatus |= 1 << HSTATUS_HU_BIT;
    write_hstatus(hstatus);
    write_hedeleg(HEDELEG_DEFAULT);
    write_hideleg(HIDELEG_DEFAULT);
    write_hgatp(0);

    log!("virt: rvh detected and initialized on cpu {}", super::cpu::cpu_id());
}

pub fn setup_vmspace(root: PhysAddr, vmid: usize) -> Result<usize, Error> {
    if !is_enabled() {
        return Err(Error::NotSupported);
    }
    if !root.is_aligned(4096) {
        return Err(Error::InvalidAddress);
    }
    let ppn = root.as_usize() >> 12;
    let hgatp = (HGATP_MODE_SV39X4 << 60) | ((vmid & 0x3fff) << 44) | (ppn & ((1usize << 44) - 1));
    Ok(hgatp)
}

pub fn run_vcpu(state: &mut VcpuRunState) -> Result<VirtExit, Error> {
    if !is_enabled() {
        return Err(Error::NotSupported);
    }
    if state.hgatp == 0 {
        return Err(Error::InvalidArgs);
    }

    // 当前实现先建立完整调用路径：加载 HGATP，返回可观测 exit。
    // 后续可在此替换为真实 vcpu enter/exit 汇编序列。
    let old_hgatp = read_hgatp();
    write_hgatp(state.hgatp);
    write_hgatp(old_hgatp);

    if state.pending_virq_bitmap != 0 {
        let pending = state.pending_virq_bitmap;
        state.pending_virq_bitmap = 0;
        return Ok(VirtExit {
            reason: VirtExitReason::ExternalInterrupt,
            detail0: pending,
            detail1: state.pc,
            detail2: 0,
        });
    }

    Ok(VirtExit {
        reason: VirtExitReason::HostTrap,
        detail0: state.pc,
        detail1: 0,
        detail2: 0,
    })
}
