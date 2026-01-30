/// 内核对象类型
/// 仅用于标识 Capability 的类型，不再携带数据
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum CapType {
    Empty = 0,
    Untyped = 1,
    TCB = 2,
    Endpoint = 3,
    Reply = 4,
    Frame = 5,
    PageTable = 6,
    CNode = 7,
    IrqHandler = 8,
    Console = 9,
    MMIO = 10,
    VSpace = 11,
}

impl CapType {
    #[inline(always)]
    pub const fn from(value: usize) -> Self {
        match value {
            1 => CapType::Untyped,
            2 => CapType::TCB,
            3 => CapType::Endpoint,
            4 => CapType::Reply,
            5 => CapType::Frame,
            6 => CapType::PageTable,
            7 => CapType::CNode,
            8 => CapType::IrqHandler,
            9 => CapType::Console,
            10 => CapType::MMIO,
            11 => CapType::VSpace,
            _ => CapType::Empty,
        }
    }
}

impl CapType {
    /// 判断 Cap 是否指向有效的内核对象
    pub fn is_valid(&self) -> bool {
        !matches!(self, CapType::Empty)
    }

    /// 判断是否为可调度的对象 (TCB)
    pub fn is_schedulable(&self) -> bool {
        matches!(self, CapType::TCB)
    }

    /// 判断是否为 IPC 端点
    pub fn is_ipc_endpoint(&self) -> bool {
        matches!(self, CapType::Endpoint)
    }

    /// 判断是否为物理内存帧
    pub fn is_frame(&self) -> bool {
        matches!(self, CapType::Frame)
    }

    /// 判断是否为页表对象
    pub fn is_pagetable(&self) -> bool {
        matches!(self, CapType::PageTable)
    }

    /// 判断是否为 CNode 对象
    pub fn is_cnode(&self) -> bool {
        matches!(self, CapType::CNode)
    }

    /// 判断是否为未类型化内存
    pub fn is_untyped(&self) -> bool {
        matches!(self, CapType::Untyped)
    }

    /// 判断是否为终端
    pub fn is_console(&self) -> bool {
        matches!(self, CapType::Console)
    }

    /// 判断是否为中断处理
    pub fn is_irq_handler(&self) -> bool {
        matches!(self, CapType::IrqHandler)
    }

    // 判断是否为Mmio
    pub fn is_mmio(&self) -> bool {
        matches!(self, CapType::MMIO)
    }

    /// 判断是否为虚拟地址空间
    pub fn is_vspace(&self) -> bool {
        matches!(self, CapType::VSpace)
    }
}
