use super::asid;
use crate::cap::{Badge, CNode, CapPtr, CapType, Capability, Slot};
use crate::hal;
use crate::hal::mem::{KSTACK_PAGES, PGSIZE};
use crate::hal::proc::ProcContext;
use crate::hal::trap::TrapFrame;
use crate::hal::trap::{trap_user_handler, trap_user_return};
use crate::ipc::{MsgArgs, UTCB};
use crate::mem::PageTable;
use crate::mem::VirtAddr;
use crate::sync::spinlock::SpinLock;
use core::fmt::Display;
use core::sync::atomic::AtomicUsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadState {
    Inactive,
    Suspended,
    Ready,
    Running,
    BlockedSend,
    BlockedRecv,
    BlockedCall,
}

#[repr(C)]
#[derive(Debug)]
pub struct TCB {
    /// 锁，保护 TCB 内部状态
    pub lock: SpinLock<()>,

    /// 引用计数
    pub ref_count: AtomicUsize,

    // --- Core Execution State ---
    pub context: ProcContext,   // 架构相关寄存器 (IP, SP, etc.)
    pub base_priority: u8,      // 基础优先级
    pub priority: u8,           // 当前优先级 (可能因优先级继承而提升)
    pub timeslice: usize,       // 剩余时间片
    pub timeslice_limit: usize, // 时间片限制 (Refill 值)
    pub state: ThreadState,     // 当前状态
    pub affinity: usize,        // CPU 亲和性
    pub cpu_id: usize,          // 当前存在的CPU调度队列（用于remove）

    // --- Kernel Stack ---
    pub kstack: Option<Capability>, // 内核栈的物理帧 (以 Capability 形式存储)

    // TrapFrame
    pub trapframe: Option<Capability>, // 用户态上下文所在的物理帧 (以 Capability 形式存储)

    // --- Resource Containers (Capabilities) ---
    pub cspace_root: Option<Capability>, // Root CNode (CSpace)
    pub vspace_root: Option<Capability>, // Root PageTable (VSpace)

    // --- IPC State ---
    pub fault_handler: Option<Capability>, // 异常处理 Endpoint
    pub bound_vcpu: Option<Capability>,    // 绑定的 VCPU（用于 guest trap 回传）

    // IPC 等待队列：当此线程处于 BlockedRecv 状态时，
    // 试图向此线程发送消息的其他线程会挂入此队列
    pub send_queue_head: Option<*mut TCB>,
    pub send_queue_tail: Option<*mut TCB>,

    // Intrusive list node (for Ready Queue or other's Send Queue)
    pub prev: Option<*mut TCB>,
    pub next: Option<*mut TCB>,

    // 正在与之通信的目标线程 (用于 Send/Recv 握手)
    pub ipc_partner: Option<*mut TCB>,

    // 当前线程正在处理的入站 IPC Endpoint（用于检测递归自调用）
    pub ipc_active_ep: Option<usize>,

    // 当前请求对应的调用方线程（若由 Call 触发），用于等待环检测
    pub ipc_caller: Option<*mut TCB>,

    // IPC state when blocked
    pub ipc_badge: Badge,
    pub ipc_cap: Option<Capability>,

    // --- UTCB (User Thread Control Block) ---
    pub utcb_frame: Option<Capability>, // UTCB 所在的物理帧 (以 Capability 形式存储)
    pub utcb_va: usize,                 // UTCB VA
    pub trapframe_va: usize,            // TrapFrame VA

    // Priveleged Thread Indicator
    pub privileged: bool, // 是否为内核线程

    // 是否为原生线程
    pub native: bool,

    // 用户态 upcall 跳转已注入（由 TCB::DeliverUpcall 设置）。
    // 在 syscall fault 返回路径消费一次后清除，避免被默认的 advance_pc/ret 覆盖。
    pub upcall_delivery_armed: bool,

    // Global thread list for debugging
    pub global_prev: Option<*mut TCB>,
    pub global_next: Option<*mut TCB>,
}

// 编译期约束：TCB 必须控制在单页内，避免线程元数据越界侵占。
const _: [(); PGSIZE - core::mem::size_of::<TCB>()] = [(); PGSIZE - core::mem::size_of::<TCB>()];

pub static mut ALL_THREADS: Option<*mut TCB> = None;

impl TCB {
    pub const fn new() -> Self {
        Self {
            lock: SpinLock::new(()),
            ref_count: AtomicUsize::new(0),
            context: ProcContext::new(),
            base_priority: 0,
            priority: 0,
            timeslice: 0,
            timeslice_limit: 0,
            state: ThreadState::Inactive,
            affinity: usize::MAX,
            cpu_id: 0,
            kstack: None,
            trapframe: None,
            cspace_root: None,
            vspace_root: None,
            fault_handler: None,
            bound_vcpu: None,
            send_queue_head: None,
            send_queue_tail: None,
            prev: None,
            next: None,
            ipc_partner: None,
            ipc_active_ep: None,
            ipc_caller: None,
            ipc_badge: Badge::null(),
            ipc_cap: None,
            utcb_frame: None,
            utcb_va: 0,
            trapframe_va: 0,
            privileged: false,
            native: true,
            upcall_delivery_armed: false,
            global_prev: None,
            global_next: None,
        }
    }

    pub fn register(tcb: *mut TCB) {
        unsafe {
            if let Some(head) = ALL_THREADS {
                (*head).global_prev = Some(tcb);
                (*tcb).global_next = Some(head);
            }
            ALL_THREADS = Some(tcb);
        }
    }

    pub fn get_kstack_top(&self) -> VirtAddr {
        self.kstack.as_ref().expect("Kernel stack not configured").obj_ptr()
            + (KSTACK_PAGES * PGSIZE)
    }

    pub fn get_tf(&mut self) -> &mut TrapFrame {
        let tf_cap = self.trapframe.as_ref().expect("TrapFrame not configured");
        unsafe { tf_cap.obj_ptr().as_mut::<TrapFrame>() }
    }

    pub fn get_tf_ref(&self) -> &TrapFrame {
        let tf_cap = self.trapframe.as_ref().expect("TrapFrame not configured");
        unsafe { tf_cap.obj_ptr().as_ref::<TrapFrame>() }
    }

    pub fn get_tf_va(&self) -> VirtAddr {
        let tf_cap = self.trapframe.as_ref().expect("TrapFrame not configured");
        tf_cap.obj_ptr()
    }

    pub fn get_pt(&self) -> &PageTable {
        let vspace_cap = self.vspace_root.as_ref().expect("VSpace root not configured");
        unsafe { vspace_cap.obj_ptr().as_ref::<PageTable>() }
    }

    pub fn get_cspace(&self) -> &CNode {
        let cspace_cap = self.cspace_root.as_ref().expect("CSpace root not configured");
        unsafe { cspace_cap.obj_ptr().as_ref::<CNode>() }
    }

    pub fn mmu_register(&mut self) -> usize {
        let (paddr, mut id) =
            self.vspace_root.as_ref().expect("VSpace root not configured").vspace_info();
        if !asid::check(id) {
            id = asid::alloc();
            // 持久化 ASID 更新到 Capability 中
            self.vspace_root.as_mut().unwrap().set_asid(id);
        }
        hal::mem::get_mmu_register(paddr, id.id as usize)
    }

    /// 配置线程的核心资源
    /// 这是 Capability 系统分发 VSpace 和 CSpace 的关键接口
    pub fn configure(
        &mut self,
        cspace: &Capability,
        vspace: &Capability,
        utcb_frame: &Capability,
        trapframe: &Capability,
        kstack: &Capability,
    ) {
        self.cspace_root = Some(cspace.clone());
        self.vspace_root = Some(vspace.clone());
        self.utcb_frame = Some(utcb_frame.clone());
        self.trapframe = Some(trapframe.clone());
        self.kstack = Some(kstack.clone());
    }

    pub fn set_priority(&mut self, prio: u8) {
        let _guard = self.lock.lock();
        self.base_priority = prio;
        if prio > self.priority {
            self.priority = prio;
        }
    }

    pub fn set_entrypoint(&mut self, entry_point: usize, stack_top: usize, thread_pointer: usize) {
        // 1. 获取内核栈顶和 mmu
        let kstack_top = self.get_kstack_top().as_usize();
        let mmu = self.mmu_register();

        // 2. 获取 TrapFrame
        let tf = self.get_tf();

        // 3. 设置用户态初始状态
        tf.configure(entry_point, stack_top, thread_pointer);
        tf.configure_kernel(
            mmu,
            hal::cpu::cpu_id(),
            kstack_top,
            trap_user_handler as *const () as usize,
        );

        // 4. 设置内核上下文，使其在被调度时跳转到 trap_user_return
        let ra = trap_user_return as *const () as usize;
        self.context.configure(ra, kstack_top);
    }

    pub fn set_address(&mut self, utcb_va: usize, trapframe_va: usize) {
        self.utcb_va = utcb_va;
        self.trapframe_va = trapframe_va;
    }

    pub fn set_fault_handler(&mut self, ep: Capability) {
        self.fault_handler = Some(ep);
    }

    pub fn set_bound_vcpu(&mut self, vcpu: Capability) {
        self.bound_vcpu = Some(vcpu);
    }

    pub fn set_affinity(&mut self, cpuid: usize) {
        self.affinity = cpuid;
    }

    pub fn set_registers(&mut self, regs: &MsgArgs) {
        let tf = self.get_tf();
        tf.set_registers(regs);
    }

    pub fn fork_from(&mut self, parent: &TCB) -> Result<(), crate::error::Error> {
        let parent_tf = *parent.get_tf_ref();
        let kstack_top = self.get_kstack_top().as_usize();
        let mmu = self.mmu_register();
        let parent_epc = parent_tf.get_epc();

        let tf = self.get_tf();
        *tf = parent_tf;
        tf.set_return_value(0);
        tf.set_epc(parent_epc.wrapping_add(4));

        tf.configure_kernel(
            mmu,
            hal::cpu::cpu_id(),
            kstack_top,
            trap_user_handler as *const () as usize,
        );

        let ra = trap_user_return as *const () as usize;
        self.context.configure(ra, kstack_top);

        Ok(())
    }

    pub fn resume(&mut self) -> bool {
        if self.state == ThreadState::Suspended || self.state == ThreadState::Inactive {
            self.state = ThreadState::Ready;
            true
        } else {
            false
        }
    }

    pub fn suspend(&mut self) {
        self.state = ThreadState::Suspended;
    }

    pub fn get_utcb(&self) -> Option<&mut UTCB> {
        if let Some(utcb_cap) = &self.utcb_frame {
            let vaddr = utcb_cap.obj_ptr();
            Some(unsafe { vaddr.as_mut::<UTCB>() })
        } else {
            None
        }
    }

    pub fn get_cspace_mut(&self) -> Option<&mut CNode> {
        let root_cap = self.cspace_root.as_ref()?;
        if root_cap.cap_type() == CapType::CNode {
            unsafe { Some(root_cap.obj_ptr().as_mut::<CNode>()) }
        } else {
            None
        }
    }

    pub fn cap_lookup(&self, cptr: CapPtr) -> Option<Capability> {
        // 1. 获取 Root CNode
        let root_cap = self.cspace_root.as_ref()?;
        if root_cap.cap_type() == CapType::CNode {
            let cnode = unsafe { root_cap.obj_ptr().as_mut::<CNode>() };
            // 2. 在 CNode 中查找
            cnode.lookup(cptr)
        } else {
            None
        }
    }

    pub fn lookup_slot(&self, cptr: CapPtr) -> Option<*mut Slot> {
        let root_cap = self.cspace_root.as_ref()?;
        if root_cap.cap_type() == CapType::CNode {
            let cnode = unsafe { root_cap.obj_ptr().as_mut::<CNode>() };
            unsafe { cnode.lookup_slot_ptr(cptr) }
        } else {
            None
        }
    }

    pub fn debug_print(&self) {
        // use crate::printk;
        printk!("TCB @ {:#x}:\n", self as *const _ as usize);
        printk!("  state:          {:?}\n", self.state);
        printk!("  priority:       {}\n", self.priority);
        printk!("  affinity:       {}\n", self.affinity);
        printk!("  timeslice:      {}\n", self.timeslice);
        printk!("  context:        {:?}\n", self.context);

        if let Some(cap) = &self.cspace_root {
            printk!("  cspace_root:    {}\n", cap);
        }
        if let Some(cap) = &self.vspace_root {
            printk!("  vspace_root:    {}\n", cap);
        }
        if let Some(cap) = &self.utcb_frame {
            printk!("  utcb_frame:     {}\n", cap);
        }
        if let Some(cap) = &self.trapframe {
            printk!("  trapframe:      {}\n", cap);
        }
        if let Some(cap) = &self.kstack {
            printk!("  kstack:         {}\n", cap);
        }

        if let Some(handler) = &self.fault_handler {
            printk!("  fault_handler:  {}\n", handler);
        }

        if let Some(partner) = self.ipc_partner {
            printk!("  ipc_partner:    {:#x}\n", partner as usize);
        }

        printk!("  privileged:     {}\n", self.privileged);
        printk!("  native:         {}\n", self.native);
    }
}

unsafe impl Send for TCB {}
unsafe impl Sync for TCB {}

impl Display for TCB {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "TCB {{ state: {:?}, priority: {}, affinity: {} }}",
            self.state, self.priority, self.affinity
        )
    }
}
