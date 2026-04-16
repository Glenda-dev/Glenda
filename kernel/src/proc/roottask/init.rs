use super::bootinfo::BOOTINFO_PAGES;
use super::bootinfo::BootInfo;
use super::bootinfo::PlatformType;
use super::layout::*;
use super::layout::{STACK_BASE, TRAPFRAME_VA, UTCB_VA};
use crate::boot;
use crate::cap::{CNode, CapPtr, Capability, Rights};
use crate::error::Error;
use crate::hal;
use crate::hal::mem::PGSIZE;
use crate::mem::PageTable;
use crate::mem::addr::virt_to_phys;
use crate::mem::pmem;
use crate::mem::{Perms, PhysAddr, VirtAddr};

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
    pub console: Capability,
    pub irq_control: Capability,
}

pub fn alloc_root_caps() -> Result<RootCaps, Error> {
    Ok(RootCaps {
        vspace: pmem::alloc_vspace_cap().ok_or(Error::OutOfMemory)?,
        cspace: {
            pmem::alloc_cnode_cap().ok_or(Error::OutOfMemory)?
        },
        tcb: {
            pmem::alloc_tcb_cap().ok_or(Error::OutOfMemory)?
        },
        utcb: {
            pmem::alloc_page_cap(1).ok_or(Error::OutOfMemory)?
        },
        tf: {
            pmem::alloc_page_cap(1).ok_or(Error::OutOfMemory)?
        },
        kstack: {
            pmem::alloc_page_cap(1).ok_or(Error::OutOfMemory)?
        },
        bootinfo: {
            pmem::alloc_page_cap(1).ok_or(Error::OutOfMemory)?
        },
        kernel: Capability::create_kernel(Rights::ALL),
        untyped_cspace: {
            pmem::alloc_cnode_cap().ok_or(Error::OutOfMemory)?
        },
        console: Capability::create_console(Rights::ALL),
        irq_control: Capability::create_irqhandler(0, Rights::ALL),
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
    if let Some((start, size)) = boot::get_initrd() {
        vspace.map_with_alloc(
            VirtAddr::from(INITRD_VA),
            virt_to_phys(start).align_down(PGSIZE),
            size,
            Perms::USER | Perms::READ,
        );
    }

    // 映射用户栈
    // 栈向下生长：[STACK_BASE - STACK_SIZE, STACK_BASE)
    // 映射时从高地址开始向下分配
    let stack_base = STACK_BASE;
    let stack_pages = STACK_PAGES;
    for i in 1..=stack_pages {
        let frame_pa = pmem::alloc_page().ok_or(Error::OutOfMemory)?;
        let va = VirtAddr::from(stack_base - i * PGSIZE);
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
    if let Some((start, size)) = boot::get_initrd() {
        bootinfo.initrd_paddr = crate::mem::addr::virt_to_phys(start).as_usize();
        bootinfo.initrd_size = size;
    }

    // ACPI优先
    if let Some(rsdp) = boot::get_rsdp() {
        bootinfo.platform_type = PlatformType::ACPI;
        bootinfo.addr = virt_to_phys(rsdp).as_usize();
        bootinfo.size = 0x1000;
    } else if let Some((dtb, size)) = boot::get_dtb() {
        bootinfo.platform_type = PlatformType::DTB;
        bootinfo.addr = virt_to_phys(dtb).as_usize();
        bootinfo.size = size;
    }

    if let Some(cmdline) = boot::get_cmdline() {
        let bytes = cmdline.as_bytes();
        let len = bytes.len().min(bootinfo.cmdline.len());
        bootinfo.cmdline[..len].copy_from_slice(&bytes[..len]);
    }

    bootinfo.virt_enabled = usize::from(hal::platform::is_virtualization_enabled());

    bootinfo.version = crate::version::get_version();
    bootinfo.build = crate::version::get_build_time_bytes();
    bootinfo.git_hash = crate::version::get_git_hash_bytes();
    Ok(())
}

pub fn init_cspace(
    cspace: &mut CNode,
    caps: &RootCaps,
    bootinfo: &mut BootInfo,
) -> Result<(), Error> {
    log!("proc: Setting up Root Task CSpace at {:p}", cspace);
    cspace.insert(CSPACE_CAP, &caps.cspace)?;
    cspace.insert(VSPACE_CAP, &caps.vspace)?;
    cspace.insert(TCB_CAP, &caps.tcb)?;
    cspace.insert(KERNEL_CAP, &caps.kernel)?;
    cspace.insert(BOOTINFO_CAP, &caps.bootinfo)?;
    cspace.insert(UNTYPED_CAP, &caps.untyped_cspace)?;
    cspace.insert(CONSOLE_CAP, &caps.console)?;
    cspace.insert(IRQ_CAP, &caps.irq_control)?;

    // === 1. MMIO Caps (Deprecated old logic) ===
    // 内核现在不再在启动阶段探测 MMIO 内存并填充 CNode。
    // 应用程序应使用 KERNEL_CAP 的 GET_MMIO 方法动态获取 Frame。

    // === 2. Untyped RAM Caps (Stored in Untyped CNode at slot 3) ===
    let (untyped_regions, count) = pmem::get_untyped();
    let untyped_cnode = unsafe { caps.untyped_cspace.obj_ptr().as_mut::<CNode>() };

    let mut slot = 1;
    for i in 0..count {
        let region = untyped_regions[i];
        let cap = Capability::create_untyped(&region, Rights::ALL);
        untyped_cnode.insert(CapPtr::from(slot), &cap)?;
        slot += 1;

        if bootinfo.untyped_count < bootinfo.untyped_list.len() {
            bootinfo.untyped_list[bootinfo.untyped_count] = region.start.as_usize();
            bootinfo.untyped_count += 1;
        }
    }
    bootinfo.cpus = boot::get_cpu_count();
    Ok(())
}
