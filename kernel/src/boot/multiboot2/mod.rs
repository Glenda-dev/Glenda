pub mod arch;

use super::BOOT_LOADER_INFO;
use crate::boot::{BootLoaderInfo, MemoryMapEntry};
use crate::hal;
use crate::mem::{PhysAddr, VirtAddr};
use crate::platform::MemoryType;

#[unsafe(no_mangle)]
static mut MULTIBOOT2_INFO_ADDR: usize = 0;

unsafe extern "C" {
    static __kernel_pbase: u8;
    static __alloc_start: u8;
}

pub const MAX_MEM_ENTRIES: usize = 256;
static mut MEM_MAP: [MemoryMapEntry; MAX_MEM_ENTRIES] =
    [MemoryMapEntry { base: PhysAddr::null(), length: 0, kind: MemoryType::Reserved };
        MAX_MEM_ENTRIES];
static mut MEM_MAP_COUNT: usize = 0;

pub unsafe fn init() {
    let info_pa = unsafe { MULTIBOOT2_INFO_ADDR };
    if info_pa == 0 {
        return;
    }

    let boot_info = unsafe { multiboot2::BootInformation::load(info_pa as *const _) }
        .expect("multiboot2: Failed to load boot info");

    // Parse memory map
    let mut count = 0;
    if let Some(mmap_tag) = boot_info.memory_map_tag() {
        for entry in mmap_tag.memory_areas() {
            if count >= MAX_MEM_ENTRIES {
                break;
            }
            unsafe {
                MEM_MAP[count] = MemoryMapEntry {
                    base: PhysAddr::from(entry.start_address() as usize),
                    length: entry.size() as usize,
                    kind: if u32::from(entry.typ()) == 1 {
                        MemoryType::Ram
                    } else {
                        MemoryType::Reserved
                    },
                };
                count += 1;
            }
        }
    }
    unsafe { MEM_MAP_COUNT = count };

    // Kernel address and sizes
    let pbase = &raw const __kernel_pbase as usize;
    let pend = &raw const __alloc_start as usize;
    let kernel_size = pend - pbase;

    // Command line
    let cmdline = boot_info.command_line_tag().and_then(|t| {
        t.cmdline().ok().map(|s| unsafe { core::mem::transmute::<&str, &'static str>(s) })
    });

    // Initrd
    let mut initrd_addr = None;
    if let Some(module) = boot_info.module_tags().next() {
        let start = module.start_address() as usize;
        let end = module.end_address() as usize;
        let size = end - start;
        initrd_addr = Some((VirtAddr::from(start), size));
    }

    // RSDP (ACPI)
    let rsdp_addr = boot_info
        .rsdp_v2_tag()
        .and_then(|t| t.signature().ok().map(|s| VirtAddr::from(s.as_ptr() as usize)))
        .or_else(|| {
            boot_info
                .rsdp_v1_tag()
                .and_then(|t| t.signature().ok().map(|s| VirtAddr::from(s.as_ptr() as usize)))
        });

    // DTB (Device Tree)
    let dtb_addr = None;
    /*
    let dtb_addr = boot_info.dtb_tag().map(|t| {
        let dtb_data = t.device_tree();
        (VirtAddr::from(dtb_data.as_ptr() as usize), dtb_data.len())
    });
    */

    BOOT_LOADER_INFO.call_once(|| BootLoaderInfo {
        dtb_addr,
        rsdp_addr,
        hhdm_offset: 0,
        memory_map: unsafe { &MEM_MAP[..MEM_MAP_COUNT] },
        kernel_address: (PhysAddr::from(pbase), VirtAddr::from(pbase)),
        kernel_size,
        initrd_addr,
        cmdline,
        cpu_count: 1, // TODO: MP info from multiboot2?
    });
}

pub fn bootstrap() {
    hal::platform::bootstrap_cpus();
}

pub unsafe fn bootstrap_kernel(magic: usize, info_pa: usize) -> ! {
    if magic != 0x36d76289 {
        // We can't use printk! yet because BSS is not cleared and init is not called
        // for now just loop if magic is wrong
        loop {}
    }

    // 0. Clean BSS
    unsafe {
        unsafe extern "C" {
            static mut __bss_start: u8;
            static mut __bss_end: u8;
        }
        let start = &raw mut __bss_start;
        let end = &raw mut __bss_end;
        let len = (end as usize).saturating_sub(start as usize);
        if len > 0 {
            core::ptr::write_bytes(start, 0, len);
        }
    }

    // 1. Save info address
    unsafe {
        MULTIBOOT2_INFO_ADDR = info_pa;
    }

    // 0. Set CPU ID (Assume 0 for now)
    hal::cpu::set_cpuid(0);

    // 2. Setup page tables
    unsafe { init() };

    let regions = unsafe { &MEM_MAP[..MEM_MAP_COUNT] };
    let satp = unsafe { hal::mem::setup_boot_pagetable(regions) };

    unsafe {
        hal::mem::activate_vspace(satp);
    }

    crate::glenda_boot();
}

#[unsafe(no_mangle)]
pub unsafe fn multiboot2_secondary_bootstrap(hartid: usize) -> ! {
    // 复用主核建立的 BOOT_PAGE_TABLE
    let root_pa = PhysAddr::from(&raw const hal::mem::BOOT_PAGE_TABLE as usize);
    let satp = hal::mem::get_mmu_register(root_pa, 0);
    unsafe {
        hal::mem::activate_vspace(satp);
    }
    crate::glenda_secondary(hartid);
}
