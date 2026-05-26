mod pte;

pub use pte::Pte;

use crate::error::Error;
use crate::hal::aarch64::asm;
use crate::hal::drivers;
use crate::mem::addr::{phys_to_virt, virt_to_phys};
use crate::mem::{PageTable, Perms, PhysAddr, VPN, VirtAddr};

pub const PGSIZE: usize = 4096;
pub const VA_MAX: usize = 1 << 48;
pub const USER_VA: usize = 0x400000;
pub const TRAMPOLINE_VA: usize = 0x0000_7fff_ffff_f000;
pub const STACK_BASE: usize = TRAMPOLINE_VA;
pub const THREAD_AREA_BASE: usize = 0x3F_0000_0000;
pub const UTCB_VA: usize = THREAD_AREA_BASE;
pub const TRAPFRAME_VA: usize = THREAD_AREA_BASE + PGSIZE;
pub const HEAP_VA: usize = 0x2000_0000;
pub const BOOTINFO_VA: usize = 0x4000_0000;
pub const INITRD_VA: usize = 0x5000_0000;
pub const PT_LEVELS: usize = 4;
pub const PGNUM: usize = 512;
pub const PT_INDEX_BITS: usize = 9;
pub const PTEFLAGS_MASK: usize = 0xfff0_0000_0000_0fff;
pub const MAX_ASID: usize = 1 << 16;
pub const ASID_MASK: usize = MAX_ASID - 1;
pub const KSTACK_PAGES: usize = 4;

unsafe extern "C" {
    static __trampoline: u8;
    static __alloc_start: u8;
}

pub unsafe fn activate_vspace(val: usize) {
    unsafe { asm::write_ttbr0(val) };
    asm::isb();
    unsafe { asm::tlbi_vmalle1is() };
    asm::dsb_sy();
    asm::isb();
}

pub unsafe fn deactivate_vspace() {
    unsafe { asm::write_ttbr0_zero() };
}

pub fn get_mmu_register(root_paddr: PhysAddr, asid: usize) -> usize {
    (root_paddr.as_usize() & !0xfff) | ((asid & ASID_MASK) << 48)
}

pub fn flush_tlb(vaddr: Option<VirtAddr>, _size: usize, _asid: Option<usize>) {
    unsafe {
        if let Some(va) = vaddr {
            asm::tlbi_vae1is(va.as_usize() >> 12);
        } else {
            asm::tlbi_vmalle1is();
        }
    }
    asm::dsb_sy();
    asm::isb();
}

pub fn get_vpn_index(va: VirtAddr, level: usize) -> VPN {
    let shift = 12 + level * PT_INDEX_BITS;
    VPN::from((va.as_usize() >> shift) & 0x1ff)
}

pub const fn page_size_for_level(level: usize) -> usize {
    1usize << (12 + level * PT_INDEX_BITS)
}

pub fn kpt_setup(kpt: &mut PageTable) {
    // MAIR_EL1: Attr0 = Normal, Attr1 = Device
    let mair: u64 = 0xFF << 0 | 0x04 << 8;

    // TCR_EL1:
    // T0SZ = 16 (48-bit), TG0 = 4KB, SH0 = Inner, ORGN0/IRGN0 = WBWA
    // Using same for T1SZ
    let tcr: u64 = 16 << 0 |  // T0SZ
                   16 << 16 | // T1SZ
                   0 << 14 |  // TG0 = 4KB
                   2 << 30 |  // TG1 = 4KB (actually 10 is 4KB for TG1)
                   3 << 12 |  // SH0 = Inner
                   3 << 28 |  // SH1 = Inner
                   1 << 10 |  // ORGN0 = WBWA
                   1 << 8 |   // IRGN0 = WBWA
                   1 << 26 |  // ORGN1 = WBWA
                   1 << 24 |  // IRGN1 = WBWA
                   (0b101u64 << 32); // IPS = 48-bit PA

    // Fix TG1: 10 is 4KB
    let tcr = (tcr & !(3 << 30)) | (2 << 30);

    unsafe {
        asm::write_mair(mair);
        asm::write_tcr(tcr);
    }
    asm::isb();

    pt_setup(kpt).expect("failed to setup kpt trampoline");

    if let Some(uart) = drivers::UART.get() {
        uart.map_mmio(kpt);
    }
    if let Some(intc) = drivers::INTC.get() {
        intc.map_mmio(kpt);
    }
    if let Some(fb) = drivers::FB.get() {
        fb.map_mmio(kpt);
    }
}

pub fn pt_setup(pt: &mut PageTable) -> Result<(), Error> {
    let tramp_va = VirtAddr::from(unsafe { &__trampoline as *const u8 as usize });
    let tramp_pa = virt_to_phys(tramp_va);
    pt.map_with_alloc(
        VirtAddr::from(TRAMPOLINE_VA),
        tramp_pa,
        PGSIZE,
        Perms::READ | Perms::EXECUTE | Perms::VALID,
    );
    Ok(())
}

pub fn kernel_end_addr() -> PhysAddr {
    virt_to_phys(VirtAddr::from(unsafe { &__alloc_start as *const u8 as usize }))
}

pub fn get_pt() -> &'static mut PageTable {
    unsafe {
        phys_to_virt(PhysAddr::from(
            virt_to_phys(VirtAddr::from(kernel_end_addr().as_usize())).as_usize(),
        ))
        .as_mut::<PageTable>()
    }
}
