use super::payload;
use super::scheduler;
use super::{TCB, ThreadState};
use crate::boot::{BootInfo, MAX_UNTYPED_REGIONS, UntypedDesc};
use crate::cap::CNode;
use crate::cap::Capability;
use crate::cap::rights;
use crate::dtb;
use crate::mem::pmem;
use crate::mem::pte::perms;
use crate::mem::{BOOTINFO_VA, TRAMPOLINE_VA, TRAPFRAME_VA, UTCB_VA};
use crate::mem::{KernelStack, PGSIZE, PageTable, PhysAddr, PteFlags, VirtAddr};
use crate::printk;
use crate::trap::vector;

pub const VSPACE_SLOT: usize = 1;
pub const CSPACE_SLOT: usize = 0;
pub const TCB_SLOT: usize = 2;
pub const UTCB_SLOT: usize = 3;
pub const BOOTINFO_SLOT: usize = 4;

pub const MEM_SLOT_START: usize = 5;

/// 初始化进程子系统并创建 Root Task
pub fn init() {
    let root_task = payload::get_root_task().expect("proc: Root task not found");
    // 1. 加载 Root Task 的 ELF 文件 (获取入口点和段信息)
    let (entry_point, stack_top) = root_task.info();

    // 2. 手动分配 Root Task 的核心对象
    let root_vspace_cap = pmem::alloc_pagetable_cap(2).expect("Failed to alloc root VSpace");
    let root_cspace_cap = pmem::alloc_cnode_cap(12).expect("Failed to alloc root CSpace");
    let root_tcb_cap = pmem::alloc_tcb_cap().expect("Failed to alloc root TCB");
    let root_utcb_cap = pmem::alloc_frame_cap().expect("Failed to alloc root UTCB");
    let root_bootinfo_cap = pmem::alloc_frame_cap().expect("Failed to alloc root BootInfo");

    // 3. 初始化 TCB (提前到这里是为了获取 TrapFrame 的物理地址)
    let tcb = root_tcb_cap.obj_ptr().as_mut::<TCB>();
    // 分配内核栈
    tcb.kstack = Some(KernelStack::alloc().expect("Failed to alloc kernel stack for Root Task"));
    let tf_paddr = tcb.get_trapframe_va().expect("Failed to get TF VA").to_pa();

    // 4. 构建 Root VSpace (页表)
    // 必须映射内核空间和 Root Task 自身的代码/数据段
    let vspace = PageTable::from_addr(root_vspace_cap.obj_ptr().to_pa());

    // 初始化用户空间布局 (Trampoline, TrapFrame, UTCB Tables)
    init_vspace(vspace, tf_paddr);

    root_task.map(vspace);
    let utcb_base = VirtAddr::from(UTCB_VA);
    // 映射 UTCB 到固定位置
    vspace.map_with_alloc(
        utcb_base,
        root_utcb_cap.obj_ptr().to_pa(),
        PGSIZE,
        PteFlags::from(perms::READ | perms::WRITE),
    );

    // 映射 BootInfo 到固定位置
    vspace.map_with_alloc(
        VirtAddr::from(BOOTINFO_VA),
        root_bootinfo_cap.obj_ptr().to_pa(),
        PGSIZE,
        PteFlags::from(perms::READ), // 只读
    );

    // 初始化 BootInfo
    let bootinfo = root_bootinfo_cap.obj_ptr().as_mut::<BootInfo>();
    *bootinfo = BootInfo::new();

    // 填充 DTB 信息
    if let Some((dtb_paddr, dtb_size)) = dtb::dtb_info() {
        bootinfo.dtb_paddr = dtb_paddr;
        bootinfo.dtb_size = dtb_size;
    }

    // 填充启动参数
    if let Some(args) = dtb::bootargs() {
        let bytes = args.as_bytes();
        let len = core::cmp::min(bytes.len(), bootinfo.cmdline.len() - 1);
        bootinfo.cmdline[..len].copy_from_slice(&bytes[..len]);
        bootinfo.cmdline[len] = 0;
    }

    // 5. 构建 Root CSpace (CNode)
    // 这是 Root Task 权力的来源。我们需要把所有剩余的物理内存
    // 转化为 Untyped Capability 并放入这个 CNode。
    let mut cspace = CNode::from_addr(root_cspace_cap.obj_ptr().to_pa(), 12);
    populate_root_cnode(&mut cspace, bootinfo);

    // 6. 配置 TCB (绑定资源)
    tcb.configure(
        &root_cspace_cap,
        &root_vspace_cap,
        Some(&root_utcb_cap),
        utcb_base,
        None, // Root Task 暂时没有 Fault Handler，或者指向内核默认处理
    );

    // 7. 设置初始寄存器
    tcb.set_registers(entry_point, stack_top);

    // 设置 BootInfo 指针到 a1
    if let Some(tf) = tcb.get_trapframe() {
        tf.a1 = BOOTINFO_VA;
    }

    // 8. 激活线程
    tcb.state = ThreadState::Ready;
    scheduler::add_thread(tcb);

    // 9. 在 Root CNode 中注册 VSpace 和 CSpace 的 Capability
    // cspace=[cspace,vspace,tcb,...]
    cspace.insert(CSPACE_SLOT, &root_cspace_cap);
    cspace.insert(VSPACE_SLOT, &root_vspace_cap);
    cspace.insert(TCB_SLOT, &root_tcb_cap);
    cspace.insert(UTCB_SLOT, &root_utcb_cap);
    cspace.insert(BOOTINFO_SLOT, &root_bootinfo_cap);

    printk!("Root Task created. Entry: {:#x}, SP: {:#x}\n", entry_point, stack_top);
}

/// 填充 Root CNode
/// 将所有空闲物理内存作为 Untyped Capability 授予 Root Task
fn populate_root_cnode(cnode: &mut CNode, bootinfo: &mut BootInfo) {
    let free_region = pmem::get_untyped();
    let preserved_region = pmem::get_preserved_untyped();
    let mut slot = MEM_SLOT_START;

    // 记录 Untyped 区域的起始槽位
    bootinfo.untyped.start = slot;

    // 目前 pmem::get_untyped 返回单个区域，但 BootInfo 支持列表
    // 我们将其作为一个条目添加
    let size = (free_region.end - free_region.start).as_usize();
    let preserved_size = (preserved_region.end - preserved_region.start).as_usize();
    // 简单起见，我们假设这是一个 2^N 大小的块，或者我们只给出一个大块
    // 实际上 Untyped 应该是 2^N 对齐的。
    // 这里我们简化处理，直接创建一个覆盖该区域的 Untyped Cap
    // 注意：Capability::create_untyped 需要 size_bits 吗？
    // 查看 pmem.rs: Capability::create_untyped(paddr, size, rights)
    // 它是 size (bytes)。

    let cap = Capability::create_untyped(free_region.start, size, rights::ALL);
    cnode.insert(slot, &cap);

    bootinfo.untyped_list[0] = UntypedDesc {
        paddr: preserved_region.start,
        size_bits: (preserved_size.ilog2() as u8), // 近似
        is_device: true,
        padding: [0; 6],
    };
    bootinfo.untyped_count += 1;

    // 填充 BootInfo
    bootinfo.untyped_list[1] = UntypedDesc {
        paddr: free_region.start,
        size_bits: (size.ilog2() as u8), // 近似
        is_device: false,
        padding: [0; 6],
    };
    bootinfo.untyped_count += 1;

    slot += 1;

    bootinfo.untyped.end = slot;

    // 插入 IRQ Handler Capabilities
    // 假设系统支持 64 个中断 (与 IRQ_TABLE 大小一致)
    bootinfo.irq.start = slot;
    for irq in 0..64 {
        let cap = Capability::new(crate::cap::CapType::IrqHandler { irq }, rights::ALL);
        cnode.insert(slot, &cap);
        slot += 1;
    }
    bootinfo.irq.end = slot;

    // 记录空闲槽位
    bootinfo.empty.start = slot;
    bootinfo.empty.end = 1 << 12; // CNode size bits = 12
}

/*
用户地址空间布局：
trampoline  (1 page) 映射在最高地址
trapframe   (1 page)
UTCB        (1 page)
BootInfo    (1 page)
ustack      (N pages)
heap        (M pages)
code + data (1 page)
empty space (1 page) 最低的4096字节 不分配物理页，同时不可访问
*/

fn init_vspace(vspace: &mut PageTable, tf_paddr: PhysAddr) {
    // 1. 映射 Trampoline (最高地址)
    // 物理地址是 vector::user_vector 的地址 (需对齐)
    let tramp_pa = PhysAddr::from(vector::user_vector as usize).align_down(PGSIZE);
    vspace.map_with_alloc(
        VirtAddr::from(TRAMPOLINE_VA),
        tramp_pa,
        PGSIZE,
        PteFlags::from(perms::READ | perms::EXECUTE),
    );

    // 2. 映射 TrapFrame (Trampoline 下方)
    vspace.map_with_alloc(
        VirtAddr::from(TRAPFRAME_VA),
        tf_paddr,
        PGSIZE,
        PteFlags::from(perms::READ | perms::WRITE),
    );
}
