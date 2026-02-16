use crate::boot::BOOT_LOADER_INFO;
use crate::boot::BootLoaderInfo;
use crate::boot::MemoryMapEntry;
use crate::hal;
use crate::mem::{PhysAddr, VirtAddr};
use crate::platform::MemoryType;

mod boot;

static mut MEM_MAP: [MemoryMapEntry; 64] =
    [MemoryMapEntry { base: PhysAddr::null(), length: 0, kind: MemoryType::Ram }; 64];
static mut MEM_MAP_COUNT: usize = 0;
static mut OPENSBI_DTB_ADDR: usize = 0;
static mut OPENSBI_HARTID: usize = 0;

unsafe extern "C" {
    static __kernel_pbase: u8;
    static __alloc_start: u8;
}

pub unsafe fn init_mem_map() -> &'static [MemoryMapEntry] {
    let dtb_pa = unsafe { OPENSBI_DTB_ADDR };
    if dtb_pa == 0 {
        return &[];
    }
    let fdt = unsafe { fdt::Fdt::from_ptr(dtb_pa as *const u8) }.expect("sbi: Failed to parse FDT");
    // 解析内存映射 (仅在第一次调用时)
    unsafe {
        if MEM_MAP_COUNT == 0 {
            let mut count = 0;
            for region in fdt.memory().regions() {
                if count < 64 {
                    let size = region.size.unwrap_or(0);
                    log!(
                        "sbi: Found memory region: {:#x} - {:#x} ({} MB)",
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
    let dtb_pa = unsafe { OPENSBI_DTB_ADDR };
    let hartid = unsafe { OPENSBI_HARTID };

    let fdt = unsafe { fdt::Fdt::from_ptr(dtb_pa as *const u8) }.expect("sbi: Failed to parse FDT");
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
                    "sbi: Found initrd in DTB: {:#x} - {:#x} ({} KB)",
                    start_val,
                    end_val,
                    size / 1024
                );
            }
        }
    }

    hal::cpu::set_cpuid(hartid);

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

pub fn set_boot_info(hartid: usize, dtb_pa: usize) {
    unsafe {
        OPENSBI_HARTID = hartid;
        OPENSBI_DTB_ADDR = dtb_pa;
    }
}

pub fn bootstrap() {
    hal::platform::bootstrap_cpus();
}
