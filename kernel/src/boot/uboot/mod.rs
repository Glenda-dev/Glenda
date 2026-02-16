pub mod arch;

use crate::boot::BOOT_LOADER_INFO;
use crate::boot::{BootLoaderInfo, MemoryMapEntry};
use crate::hal;
use crate::mem::{PhysAddr, VirtAddr};
use crate::platform::MemoryType;

#[unsafe(no_mangle)]
static mut UBOOT_DTB_ADDR: usize = 0;

unsafe extern "C" {
    static __kernel_pbase: u8;
    static __alloc_start: u8;
}

pub const MAX_MEM_ENTRIES: usize = 256;
static mut MEM_MAP: [MemoryMapEntry; MAX_MEM_ENTRIES] =
    [MemoryMapEntry { base: PhysAddr::null(), length: 0, kind: MemoryType::Reserved };
        MAX_MEM_ENTRIES];
static mut MEM_MAP_COUNT: usize = 0;

pub unsafe fn init_mem_map() -> &'static [MemoryMapEntry] {
    let dtb_pa = unsafe { UBOOT_DTB_ADDR };
    if dtb_pa == 0 {
        return &[];
    }
    let fdt =
        unsafe { fdt::Fdt::from_ptr(dtb_pa as *const u8) }.expect("uboot: Failed to parse FDT");
    // 解析内存映射 (仅在第一次调用时)
    unsafe {
        if MEM_MAP_COUNT == 0 {
            let mut count = 0;
            for region in fdt.memory().regions() {
                if count < 64 {
                    let size = region.size.unwrap_or(0);
                    log!(
                        "dtb: Found memory region: {:#x} - {:#x} ({} MB)",
                        region.starting_address as usize,
                        region.starting_address as usize + size,
                        size / 1024 / 1024
                    );
                    MEM_MAP[count] = MemoryMapEntry {
                        base: PhysAddr::from(region.starting_address as usize),
                        length: size,
                        kind: MemoryType::Ram,
                    };
                    count += 1;
                }
            }
            MEM_MAP_COUNT = count;
        }
    }
    unsafe { &MEM_MAP[..MEM_MAP_COUNT] }
}

pub fn init() {
    let dtb_pa = unsafe { UBOOT_DTB_ADDR };
    if dtb_pa == 0 {
        return;
    }

    // 解析设备树
    let fdt =
        unsafe { fdt::Fdt::from_ptr(dtb_pa as *const u8).expect("uboot: Failed to parse FDT") };

    // 确定 CPU 数量
    let cpu_count = fdt.cpus().count();

    // 确定 initrd 和命令行
    let mut initrd_addr = None;
    let cmdline = fdt.chosen().bootargs();

    if let Some(chosen_node) = fdt.find_node("/chosen") {
        if let (Some(start), Some(end)) =
            (chosen_node.property("linux,initrd-start"), chosen_node.property("linux,initrd-end"))
        {
            let parse_be = |data: &[u8]| -> usize {
                if data.len() == 8 {
                    u64::from_be_bytes(data.try_into().unwrap()) as usize
                } else if data.len() == 4 {
                    u32::from_be_bytes(data.try_into().unwrap()) as usize
                } else {
                    0
                }
            };

            let start_val = parse_be(start.value);
            let end_val = parse_be(end.value);

            if start_val != 0 && end_val > start_val {
                let size = end_val - start_val;
                // Currently identity mapping
                initrd_addr = Some((VirtAddr::from(start_val), size));
                log!(
                    "uboot: Found initrd in DTB: {:#x} - {:#x} ({} bytes)",
                    start_val,
                    end_val,
                    size
                );
            }
        }
    }

    let pbase = &raw const __kernel_pbase as usize;
    let pend = &raw const __alloc_start as usize;
    let kernel_size = pend - pbase;

    BOOT_LOADER_INFO.call_once(|| BootLoaderInfo {
        dtb_addr: Some(VirtAddr::from(dtb_pa)),
        rsdp_addr: None,
        hhdm_offset: 0,
        memory_map: unsafe { &MEM_MAP[..MEM_MAP_COUNT] },
        kernel_address: (PhysAddr::from(pbase), VirtAddr::from(pbase)),
        kernel_size,
        initrd_addr,
        cmdline,
        cpu_count,
    });
}

pub fn bootstrap() {
    hal::platform::bootstrap_cpus();
}

pub unsafe fn bootstrap_kernel(hartid: usize, dtb_pa: usize) -> ! {
    // 0. 清零 BSS (主核负责)
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

    // 0. 设置当前核 ID 到 tp 寄存器
    hal::cpu::set_cpuid(hartid);

    // 1. 保存 DTB 物理地址
    unsafe {
        UBOOT_DTB_ADDR = dtb_pa;
    }

    // 2. 通过 HAL 构造初始页表并开启 MMU
    log!("uboot: Setting up boot page tables...");

    let regions = unsafe { init_mem_map() };
    let satp = unsafe { hal::mem::setup_boot_pagetable(regions) };

    log!("uboot: Enabling MMU...");
    unsafe {
        hal::mem::activate_vspace(satp);
    }

    // 3. 跳转到内核入口
    log!("uboot: Jumping to kernel main...");
    crate::glenda_boot();
}

#[unsafe(no_mangle)]
pub unsafe fn uboot_secondary_bootstrap(hartid: usize) -> ! {
    // 复用主核建立的 BOOT_PAGE_TABLE
    let root_pa = PhysAddr::from(&raw const hal::mem::BOOT_PAGE_TABLE as usize);
    let satp = hal::mem::get_mmu_register(root_pa, 0);
    unsafe {
        hal::mem::activate_vspace(satp);
    }
    crate::glenda_secondary(hartid);
}

pub fn get_dtb_addr() -> Option<VirtAddr> {
    unsafe { if UBOOT_DTB_ADDR != 0 { Some(VirtAddr::from(UBOOT_DTB_ADDR)) } else { None } }
}
