use super::bootinfo::BOOTINFO_PAGES;
use super::bootinfo::BootInfo;
use super::layout::*;
use crate::cap::{CNode, CapPtr, Capability, Rights};
use crate::hal;
use crate::hal::irq::MAX_IRQS;
use crate::hal::mem::{KSTACK_PAGES, PGSIZE};
use crate::irq::IRQ;

use crate::mem::pmem;
use crate::mem::{MemoryRange, PageTable};
use crate::mem::{Perms, PhysAddr, VirtAddr};
use crate::mem::{TRAPFRAME_VA, UTCB_VA};
use crate::platform;
use crate::platform::PLATFORM_PAGES;
use crate::platform::PlatformInfo;
use crate::proc::roottask::bootinfo::BOOTINFO_MAGIC;

pub struct RootCaps {
    pub vspace: Capability,
    pub cspace: Capability,
    pub tcb: Capability,
    pub utcb: Capability,
    pub tf: Capability,
    pub kstack: Capability,
    pub bootinfo: Capability,
    pub kernel: Capability,
    pub untyped_cspace: Capability,
    pub mmio_cspace: Capability,
    pub irq_cspace: Capability,
    pub platform: Capability,
}

pub fn alloc_root_caps() -> RootCaps {
    RootCaps {
        vspace: pmem::alloc_vspace_cap().expect("Failed to alloc root VSpace"),
        cspace: pmem::alloc_cnode_cap().expect("Failed to alloc root CSpace"),
        tcb: pmem::alloc_tcb_cap().expect("Failed to alloc root TCB"),
        utcb: pmem::alloc_frame_cap(1).expect("Failed to alloc root UTCB"),
        tf: pmem::alloc_frame_cap(1).expect("Failed to alloc root TrapFrame"),
        kstack: pmem::alloc_frame_cap(KSTACK_PAGES).expect("Failed to alloc root Kernel Stack"),
        platform: pmem::alloc_frame_cap(PLATFORM_PAGES)
            .expect("Failed to alloc root Platform Info"),
        bootinfo: pmem::alloc_frame_cap(BOOTINFO_PAGES).expect("Failed to alloc root BootInfo"),
        kernel: Capability::create_kernel(Rights::ALL),
        untyped_cspace: pmem::alloc_cnode_cap().expect("Failed to alloc Untyped CNode"),
        mmio_cspace: pmem::alloc_cnode_cap().expect("Failed to alloc MMIO CNode"),
        irq_cspace: pmem::alloc_cnode_cap().expect("Failed to alloc IRQ CNode"),
    }
}

pub fn init_vspace(
    vspace: &mut PageTable,
    tf_paddr: PhysAddr,
    utcb_paddr: PhysAddr,
    bootinfo_paddr: PhysAddr,
) {
    log!("proc: Setting up Root Task VSpace at {:#x}", vspace as *const _ as usize);
    // 2. 映射 TrapFrame (Trampoline 下方)
    // TrapFrame 仅由 S 态的 user_vector/user_return 访问
    vspace.map_with_alloc(
        VirtAddr::from(TRAPFRAME_VA),
        tf_paddr,
        PGSIZE,
        Perms::READ | Perms::WRITE,
    );

    // 映射 UTCB 到固定位置
    vspace.map_with_alloc(
        VirtAddr::from(UTCB_VA),
        utcb_paddr,
        PGSIZE,
        Perms::USER | Perms::READ | Perms::WRITE,
    );

    // 映射 BootInfo 到固定位置
    vspace.map_with_alloc(
        VirtAddr::from(BOOTINFO_VA),
        bootinfo_paddr,
        BOOTINFO_PAGES * PGSIZE,
        Perms::USER | Perms::READ, // 只读
    );

    // 映射 Initrd 到固定位置
    let range = platform::get().initrd;
    vspace.map_with_alloc(
        VirtAddr::from(INITRD_VA),
        range.start.align_down(PGSIZE),
        range.size,
        Perms::USER | Perms::READ,
    );

    // 映射用户栈
    let stack_va_start = STACK_VA;
    let stack_pages = STACK_PAGES;
    for i in 1..=stack_pages {
        let frame_pa = pmem::alloc_page().expect("Failed to alloc user stack");
        let va = VirtAddr::from(stack_va_start - i * PGSIZE);
        vspace.map_with_alloc(va, frame_pa, PGSIZE, Perms::USER | Perms::READ | Perms::WRITE);
    }

    // 映射用户堆 (256KB)
    // HEAP_VA = 0x2000_0000 (Defined in libglenda-rs/src/runtime.rs)
    let heap_va_start = HEAP_VA;
    let heap_pages = HEAP_PAGES;
    for i in 0..heap_pages {
        let frame_pa = pmem::alloc_page().expect("Failed to alloc user heap");
        let va = VirtAddr::from(heap_va_start + i * PGSIZE);
        vspace.map_with_alloc(va, frame_pa, PGSIZE, Perms::USER | Perms::READ | Perms::WRITE);
    }

    // 设置 Trampoline 映射
    hal::mem::pt_setup(vspace).expect("Failed to setup VSpace for root task");
}
pub fn init_bootinfo(bootinfo: &mut BootInfo) {
    bootinfo.magic = BOOTINFO_MAGIC;
    // 设置 Initrd 信息
    let initrd = platform::get().initrd;
    let initrd_offset = initrd.start.as_usize() % PGSIZE;
    bootinfo.initrd_start = INITRD_VA + initrd_offset;
    bootinfo.initrd_size = initrd.size;
}
pub fn init_platform(platform: &mut PlatformInfo) {
    let info = platform::get();
    *platform = info.clone();
}
pub fn init_cspace(cspace: &mut CNode, caps: &RootCaps, bootinfo: &mut BootInfo) {
    let info = platform::get();
    cspace.insert(CSPACE_CAP, &caps.cspace);
    cspace.insert(VSPACE_CAP, &caps.vspace);
    cspace.insert(TCB_CAP, &caps.tcb);
    cspace.insert(KERNEL_CAP, &caps.kernel);
    cspace.insert(PLATFORM_CAP, &caps.platform);
    cspace.insert(UNTYPED_CAP, &caps.untyped_cspace);
    cspace.insert(MMIO_CAP, &caps.mmio_cspace);
    cspace.insert(IRQ_CAP, &caps.irq_cspace);

    // === 1. MMIO Caps (Stored in MMIO CNode at slot 7) ===
    let mut slot = 1;
    let mmio_cnode = caps.mmio_cspace.obj_ptr().as_mut::<CNode>();

    for i in 0..info.memory_regions.len() {
        let region = &info.memory_regions[i];
        if region.region_type == platform::MemoryType::Mmio {
            let cap = Capability::create_mmio(
                &MemoryRange { start: region.start, size: region.size },
                Rights::ALL,
            );
            // 插入到 MMIO 子 CNode
            mmio_cnode.insert(CapPtr::from(slot), &cap);

            if bootinfo.mmio_count < bootinfo.mmio_list.len() {
                bootinfo.mmio_list[bootinfo.mmio_count] =
                    MemoryRange { start: region.start, size: region.size };
                bootinfo.mmio_count += 1;
            }
            slot += 1;
        }
    }

    // === 2. Untyped RAM Caps (Stored in Untyped CNode at slot 3) ===
    let (untyped_regions, count) = pmem::get_untyped();
    let untyped_cnode = caps.untyped_cspace.obj_ptr().as_mut::<CNode>();

    let mut slot = 1;
    for i in 0..count {
        let region = untyped_regions[i];
        let cap = Capability::create_untyped(&region, Rights::ALL);
        untyped_cnode.insert(CapPtr::from(slot), &cap);
        slot += 1;

        if bootinfo.untyped_count < bootinfo.untyped_list.len() {
            bootinfo.untyped_list[bootinfo.untyped_count] = region;
            bootinfo.untyped_count += 1;
        }
    }

    // === 3. IRQ Caps (Stored in Root CNode L1 directly, starting slot 8) ===
    // === 2. Untyped RAM Caps (Stored in Untyped CNode at slot 3) ===
    let mut slot = 1;
    let irq_cnode = caps.irq_cspace.obj_ptr().as_mut::<CNode>();
    for irq in 0..MAX_IRQS {
        let irq_obj = IRQ::new(irq);
        let cap = Capability::create_irqhandler(&irq_obj, Rights::ALL);
        // 插入到 IRQ 子 CNode
        irq_cnode.insert(CapPtr::from(slot), &cap);
        slot += 1;
    }
    bootinfo.irq_count = MAX_IRQS;
}
