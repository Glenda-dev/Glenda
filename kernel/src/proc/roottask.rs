use super::payload;
use super::scheduler;
use super::{TCB, ThreadState};
use crate::cap::CNode;
use crate::cap::Capability;
use crate::cap::rights;
use crate::ipc::UTCB_VA;
use crate::mem::pmem;
use crate::mem::pte::perms;
use crate::mem::{PGSIZE, PageTable, PteFlags};
use crate::printk;

pub const VSPACE_SLOT: usize = 1;
pub const CSPACE_SLOT: usize = 0;
pub const TCB_SLOT: usize = 2;
pub const UTCB_SLOT: usize = 3;

pub const MEM_SLOT: usize = 4;
pub const MMIO_SLOT: usize = 5;
pub const IRQ_SLOT: usize = 6;
pub const FAULT_SLOT: usize = 7;

/// 初始化进程子系统并创建 Root Task
pub fn init() {
    let root_task = payload::get_root_task().expect("proc: Root task not found");
    // 1. 加载 Root Task 的 ELF 文件 (获取入口点和段信息)
    let (entry_point, stack_top) = root_task.info();

    // 2. 手动分配 Root Task 的核心对象
    let root_vspace_cap = pmem::alloc_pagetable_cap().expect("Failed to alloc root VSpace");
    let root_cspace_cap = pmem::alloc_cnode_cap(12).expect("Failed to alloc root CSpace");
    let root_tcb_cap = pmem::alloc_tcb_cap().expect("Failed to alloc root TCB");
    let root_utcb_cap = pmem::alloc_frame_cap().expect("Failed to alloc root UTCB");

    // 3. 构建 Root VSpace (页表)
    // 必须映射内核空间和 Root Task 自身的代码/数据段
    let vspace = PageTable::from_addr(root_vspace_cap.obj_ptr().to_pa());
    vspace.map_kernel();
    root_task.map_segments(vspace);
    let utcb_base = UTCB_VA;
    // 映射 UTCB 到固定位置
    vspace
        .map(
            utcb_base,
            root_utcb_cap.obj_ptr().to_pa(),
            PGSIZE,
            PteFlags::from(perms::READ | perms::WRITE),
        )
        .expect("Failed to map UTCB");

    // 4. 构建 Root CSpace (CNode)
    // 这是 Root Task 权力的来源。我们需要把所有剩余的物理内存
    // 转化为 Untyped Capability 并放入这个 CNode。
    let mut cspace = CNode::from_addr(root_cspace_cap.obj_ptr().to_pa(), 12);
    populate_root_cnode(&mut cspace);

    // 5. 初始化 TCB
    // 这里我们将物理帧转换为内核对象引用
    let tcb = root_tcb_cap.obj_ptr().as_mut::<TCB>();
    // TCB 已经在 alloc_tcb_cap 中初始化

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

    // 8. 激活线程
    tcb.state = ThreadState::Ready;
    scheduler::add_thread(tcb);

    // 9. 在 Root CNode 中注册 VSpace 和 CSpace 的 Capability
    // cspace=[cspace,vspace,tcb,...]
    cspace.insert(CSPACE_SLOT, &root_cspace_cap);
    cspace.insert(VSPACE_SLOT, &root_vspace_cap);
    cspace.insert(TCB_SLOT, &root_tcb_cap);
    cspace.insert(UTCB_SLOT, &root_utcb_cap);

    printk!("Root Task created. Entry: {:#x}, SP: {:#x}\n", entry_point, stack_top);
}

/// 填充 Root CNode
/// 将所有空闲物理内存作为 Untyped Capability 授予 Root Task
fn populate_root_cnode(cnode: &mut CNode) {
    let free_regions = pmem::get_untyped();

    let cap = Capability::create_untyped(
        free_regions.start,
        (free_regions.end - free_regions.start).as_usize(),
        rights::ALL,
    );
    cnode.insert(MEM_SLOT, &cap);

    // TODO: 还需要插入设备内存 (MMIO) 和 IRQ Capability
    // ...
    unimplemented!()
}
