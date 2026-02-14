use super::bootinfo::BOOTINFO_PAGES;
use super::bootinfo::BootInfo;
use super::layout::*;
use crate::cap::{CNode, CapPtr, Capability, Rights};
use crate::error::Error;
use crate::hal;
use crate::hal::irq::MAX_IRQS;
use crate::hal::mem::{KSTACK_PAGES, PGSIZE};
use crate::irq::IRQ;
use crate::log;
use crate::mem::PageTable;
use crate::mem::pmem;
use crate::mem::{Perms, PhysAddr, VirtAddr};
use crate::mem::{TRAPFRAME_VA, UTCB_VA};

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
    pub mmio: Capability,
    pub irq_cspace: Capability,
}

pub fn alloc_root_caps() -> Result<RootCaps, Error> {
    Ok(RootCaps {
        vspace: pmem::alloc_vspace_cap().ok_or(Error::OutOfMemory)?,
        cspace: pmem::alloc_cnode_cap().ok_or(Error::OutOfMemory)?,
        tcb: pmem::alloc_tcb_cap().ok_or(Error::OutOfMemory)?,
        utcb: pmem::alloc_frame_cap(1).ok_or(Error::OutOfMemory)?,
        tf: pmem::alloc_frame_cap(1).ok_or(Error::OutOfMemory)?,
        kstack: pmem::alloc_frame_cap(KSTACK_PAGES).ok_or(Error::OutOfMemory)?,
        bootinfo: pmem::alloc_frame_cap(BOOTINFO_PAGES).ok_or(Error::OutOfMemory)?,
        kernel: Capability::create_kernel(Rights::ALL),
        untyped_cspace: pmem::alloc_cnode_cap().ok_or(Error::OutOfMemory)?,
        mmio: Capability::create_mmio(Rights::ALL),
        irq_cspace: pmem::alloc_cnode_cap().ok_or(Error::OutOfMemory)?,
    })
}

pub fn init_vspace(
    vspace: &mut PageTable,
    tf_paddr: PhysAddr,
    utcb_paddr: PhysAddr,
    bootinfo_paddr: PhysAddr,
) -> Result<(), Error> {
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
    if let Some((start, size)) = crate::boot::get_initrd() {
        vspace.map_with_alloc(
            VirtAddr::from(INITRD_VA),
            start.align_down(PGSIZE),
            size,
            Perms::USER | Perms::READ,
        );
    }

    // 映射用户栈
    let stack_va_start = STACK_VA;
    let stack_pages = STACK_PAGES;
    for i in 1..=stack_pages {
        let frame_pa = pmem::alloc_page().ok_or(Error::OutOfMemory)?;
        let va = VirtAddr::from(stack_va_start - i * PGSIZE);
        vspace.map_with_alloc(va, frame_pa, PGSIZE, Perms::USER | Perms::READ | Perms::WRITE);
    }

    // 映射用户堆 (256KB)
    // HEAP_VA = 0x2000_0000 (Defined in libglenda-rs/src/runtime.rs)
    let heap_va_start = HEAP_VA;
    let heap_pages = HEAP_PAGES;
    for i in 0..heap_pages {
        let frame_pa = pmem::alloc_page().ok_or(Error::OutOfMemory)?;
        let va = VirtAddr::from(heap_va_start + i * PGSIZE);
        vspace.map_with_alloc(va, frame_pa, PGSIZE, Perms::USER | Perms::READ | Perms::WRITE);
    }

    // 设置 Trampoline 映射
    hal::mem::pt_setup(vspace)?;
    Ok(())
}
pub fn init_bootinfo(bootinfo: &mut BootInfo) -> Result<(), Error> {
    // 设置 Initrd 信息
    if let Some((start, size)) = crate::boot::get_initrd() {
        let initrd_offset = start.as_usize() % PGSIZE;
        bootinfo.initrd_offset = initrd_offset;
        bootinfo.initrd_size = size;
    }

    // ACPI优先
    if let Some(rsdp) = crate::boot::get_rsdp() {
        bootinfo.platform_type = super::bootinfo::PlatformType::ACPI;
        bootinfo.addr = rsdp.as_usize();
        bootinfo.size = 0x1000;
    } else if let Some(dtb) = crate::boot::get_dtb() {
        bootinfo.platform_type = super::bootinfo::PlatformType::DTB;
        bootinfo.addr = dtb.as_usize();
        bootinfo.size = 0x10000;
    }

    if let Some(cmdline) = crate::boot::get_cmdline() {
        let bytes = cmdline.as_bytes();
        let len = bytes.len().min(bootinfo.cmdline.len());
        bootinfo.cmdline[..len].copy_from_slice(&bytes[..len]);
    }

    bootinfo.version = crate::version::get_version();
    bootinfo.build = crate::version::get_build_time_bytes();
    bootinfo.git_hash = crate::version::get_git_hash_bytes();
    Ok(())
}

pub fn init_cspace(
    cspace: &mut CNode,
    caps: &RootCaps,
    _bootinfo: &mut BootInfo,
) -> Result<(), Error> {
    log!("proc: Setting up Root Task CSpace at {:p}", cspace);
    cspace.insert(CSPACE_CAP, &caps.cspace)?;
    cspace.insert(VSPACE_CAP, &caps.vspace)?;
    cspace.insert(TCB_CAP, &caps.tcb)?;
    cspace.insert(KERNEL_CAP, &caps.kernel)?;
    cspace.insert(BOOTINFO_CAP, &caps.bootinfo)?;
    cspace.insert(UNTYPED_CAP, &caps.untyped_cspace)?;
    cspace.insert(MMIO_CAP, &caps.mmio)?;
    cspace.insert(IRQ_CAP, &caps.irq_cspace)?;

    // === 1. MMIO Caps (Deprecated old logic) ===
    // 内核现在不再在启动阶段探测 MMIO 内存并填充 CNode。
    // 应用程序应使用 MMIO_CAP (Mmio 类型) 动态获取 Frame。

    // === 2. Untyped RAM Caps (Stored in Untyped CNode at slot 3) ===
    let (untyped_regions, count) = pmem::get_untyped();
    let untyped_cnode = unsafe { caps.untyped_cspace.obj_ptr().as_mut::<CNode>() };

    let mut slot = 1;
    for i in 0..count {
        let region = untyped_regions[i];
        let cap = Capability::create_untyped(&region, Rights::ALL);
        untyped_cnode.insert(CapPtr::from(slot), &cap)?;
        slot += 1;

        if _bootinfo.untyped_count < _bootinfo.untyped_list.len() {
            _bootinfo.untyped_list[_bootinfo.untyped_count] = region;
            _bootinfo.untyped_count += 1;
        }
    }

    // === 3. IRQ Caps (Stored in Root CNode L1 directly, starting slot 8) ===
    // === 2. Untyped RAM Caps (Stored in Untyped CNode at slot 3) ===
    let mut slot = 1;
    let irq_cnode = unsafe { caps.irq_cspace.obj_ptr().as_mut::<CNode>() };
    for irq in 1..MAX_IRQS {
        let irq_obj = IRQ::new(irq);
        let cap = Capability::create_irqhandler(&irq_obj, Rights::ALL);
        // 插入到 IRQ 子 CNode
        irq_cnode.insert(CapPtr::from(slot), &cap)?;
        slot += 1;
    }
    Ok(())
}
