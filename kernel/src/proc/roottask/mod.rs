pub mod bootinfo;
pub mod init;
pub mod initrd;
pub mod layout;

pub use bootinfo::BootInfo;
pub use layout::STACK_BASE;

use super::scheduler;
use super::{TCB, ThreadState};
use crate::cap::CNode;

use crate::mem::PageTable;
use init::*;
use layout::*;

use initrd::ProcPayload;

use crate::error::Error;

/// 初始化进程子系统
pub fn init() {
    initrd::init();
}

/// 创建 Root Task
pub fn spawn(name: &str) {
    if let Some(task) = initrd::find(name) {
        spawn_payload(task).expect("Failed to spawn root task payload");
    } else {
        panic!("proc: Root task '{}' not found", name);
    }
}

pub fn spawn_first() {
    if let Some(task) = initrd::get_by_index(0) {
        let len = task.metadata.name.iter().position(|&c| c == 0).unwrap_or(32);
        let name = core::str::from_utf8(&task.metadata.name[..len]).unwrap_or("unknown");
        log!("proc: Spawning default root task '{}'", name);
        spawn_payload(task).expect("Failed to spawn default root task payload");
    } else {
        panic!("proc: No root task found in initrd");
    }
}

fn spawn_payload(root_task: ProcPayload) -> Result<(), Error> {
    let (entry_point, stack_top) = root_task.info();

    // 1. Allocate Capabilities
    let caps = alloc_root_caps()?;

    // 2. Setup TCB basic fields
    let tcb = unsafe { caps.tcb.obj_ptr().as_mut::<TCB>() };

    // 3. Setup VSpace
    let pt_pa = caps.vspace.paddr();
    let vspace = PageTable::from_addr(pt_pa);
    init_vspace(vspace, caps.tf.paddr(), caps.utcb.paddr(), caps.bootinfo.paddr())?;
    root_task.map(vspace);

    // 4. Setup bootinfo
    let bootinfo = unsafe { caps.bootinfo.obj_ptr().as_mut::<BootInfo>() };
    init_bootinfo(bootinfo)?;

    // 5. Setup CSpace
    let cspace = unsafe { caps.cspace.obj_ptr().as_mut::<CNode>() };
    init_cspace(cspace, &caps, bootinfo)?;
    // 7. Configure TCB resources
    TCB::register(tcb);
    tcb.configure(&caps.cspace, &caps.vspace, &caps.utcb, &caps.tf, &caps.kstack);
    tcb.set_priority(ROOT_TASK_PRIORITY);
    tcb.set_entrypoint(entry_point, stack_top, 0);
    tcb.set_address(UTCB_VA, TRAPFRAME_VA);
    tcb.state = ThreadState::Ready;
    scheduler::add_thread(tcb);
    log!("proc: Root Task created. Entry: {:#x}, SP: {:#x}", entry_point, stack_top);

    //cspace.debug_print();
    //vspace.debug_print();
    Ok(())
}
/*
用户地址空间布局：
trampoline  (1 page) 映射在最高地址
ustack      (N pages)
------------
utcb                  0x70000000
trapframe
------------
Initrd      (N pages) 0x50000000
————————————
heap        (M pages) 0x20000000
-------------
code + data (N pages)
empty space (1 page) 最低的4096字节 不分配物理页，同时不可访问
*/
